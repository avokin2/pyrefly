/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! A project-wide routing table for Django reverse relations.
//!
//! Django adds an accessor to the model a relation field points *at*:
//! `Choice.question = ForeignKey(Question)` gives `Question.choice_set`. The
//! target class body says nothing about who references it, and the target
//! module does not import the source, so synthesizing `choice_set` requires
//! knowing which other modules declare a relation to `Question`.
//!
//! This module answers exactly that question, and nothing more. It is a
//! pruning filter, not a source of truth: the accessor names and types still
//! come from each source module's `KeyDjangoRelations` answer, which is keyed
//! by the exact `Class`. A syntactic false positive here therefore costs one
//! wasted lookup, never a wrong answer.
//!
//! The scan must recognize exactly what `solve_django_reverse_relations`
//! recognizes. A divergence in either direction shows up as an attribute that
//! silently does not exist.

use dupe::Dupe;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_util::visit::Visit;
use ruff_python_ast::Expr;
use ruff_python_ast::ModModule;
use ruff_python_ast::Stmt;
use ruff_python_ast::name::Name;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

const FOREIGN_KEY: Name = Name::new_static("ForeignKey");
const ONE_TO_ONE_FIELD: Name = Name::new_static("OneToOneField");
const MANY_TO_MANY_FIELD: Name = Name::new_static("ManyToManyField");

/// The substrings that must appear in a file's source for it to be able to
/// declare a relation. Checking these against the raw bytes lets us skip
/// parsing the overwhelming majority of a project's files.
pub const RELATION_TOKENS: [&str; 3] = ["ForeignKey", "OneToOneField", "ManyToManyField"];

/// What one module contributes to the routing table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DjangoScan {
    /// Short names of the models this module declares relations to.
    pub targets: SmallSet<Name>,
    /// Whether some relation here has a target we cannot read syntactically,
    /// such as a variable or an f-string. Such a module is consulted for every
    /// target, because we cannot rule it out.
    pub unresolved_target: bool,
}

impl DjangoScan {
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty() && !self.unresolved_target
    }
}

/// One module's entry in the index: how to demand its relation map, and what
/// it contributes.
pub type IndexedModule = (ModuleName, DjangoScan);

/// The project-wide routing table.
///
/// Immutable once built. A refresh produces a new one from an updated file
/// map, so a transaction that is never committed simply drops its copy.
#[derive(Debug, Default)]
pub struct DjangoRelationIndex {
    /// Only modules with something to contribute are stored; a module absent
    /// here has been scanned and found empty.
    files: SmallMap<ModulePath, IndexedModule>,
    /// Target short name to the modules that may declare a relation to it,
    /// sorted for deterministic merge order.
    by_target: SmallMap<Name, Vec<(ModuleName, ModulePath)>>,
    /// Modules whose relation target we could not read syntactically. They are
    /// consulted for every target, because we cannot rule them out.
    unresolved: Vec<(ModuleName, ModulePath)>,
}

/// Sorting by the rendered strings rather than by `ModuleName`'s derived
/// ordering, which compares interned pointers and so varies between runs.
fn candidate_sort_key(candidate: &(ModuleName, ModulePath)) -> (String, String) {
    (candidate.0.as_str().to_owned(), candidate.1.to_string())
}

impl DjangoRelationIndex {
    pub fn new(files: SmallMap<ModulePath, IndexedModule>) -> Self {
        let mut by_target: SmallMap<Name, Vec<(ModuleName, ModulePath)>> = SmallMap::new();
        let mut unresolved = Vec::new();
        for (path, (module, scan)) in &files {
            let candidate = (*module, path.dupe());
            for target in &scan.targets {
                by_target
                    .entry(target.clone())
                    .or_default()
                    .push(candidate.clone());
            }
            if scan.unresolved_target {
                unresolved.push(candidate);
            }
        }
        for candidates in by_target.values_mut() {
            candidates.sort_by_key(candidate_sort_key);
        }
        unresolved.sort_by_key(candidate_sort_key);
        Self {
            files,
            by_target,
            unresolved,
        }
    }

    pub fn files(&self) -> &SmallMap<ModulePath, IndexedModule> {
        &self.files
    }

    /// The modules that may declare a reverse accessor on a model named
    /// `target`, in a deterministic order.
    pub fn candidates(&self, target: &Name) -> Vec<(ModuleName, ModulePath)> {
        let named = self.by_target.get(target).map_or(&[][..], Vec::as_slice);
        if self.unresolved.is_empty() {
            return named.to_vec();
        }
        let mut candidates = named.to_vec();
        candidates.extend(
            self.unresolved
                .iter()
                .filter(|c| !named.contains(c))
                .cloned(),
        );
        candidates.sort_by_key(candidate_sort_key);
        candidates
    }
}

/// Which modules have asked the routing table about which model names.
///
/// A module that synthesizes fields for a model has no dependency edge to a
/// module that does not yet reference that model, so when one appears the only
/// way to find the stale readers is to remember who asked.
#[derive(Debug, Default, Clone)]
pub struct DjangoReaders(SmallMap<Name, SmallSet<ModulePath>>);

impl DjangoReaders {
    pub fn record(&mut self, target: &Name, reader: &ModulePath) {
        self.0
            .entry(target.clone())
            .or_default()
            .insert(reader.dupe());
    }

    pub fn merge(&mut self, other: DjangoReaders) {
        for (target, readers) in other.0 {
            self.0.entry(target).or_default().extend(readers);
        }
    }

    fn of(&self, target: &Name) -> impl Iterator<Item = &ModulePath> {
        self.0
            .get(target)
            .into_iter()
            .flat_map(|paths| paths.iter())
    }

    fn all(&self) -> impl Iterator<Item = &ModulePath> {
        self.0.values().flat_map(|paths| paths.iter())
    }
}

/// Accumulates the target names whose candidate set changed during a refresh.
#[derive(Debug, Default)]
pub struct ChangedTargets {
    names: SmallSet<Name>,
    /// Set when a module gained or lost an unreadable target, which changes
    /// the candidate set of every name at once.
    all: bool,
}

impl ChangedTargets {
    /// Record the difference a rescan of one file made.
    pub fn record(&mut self, before: Option<&DjangoScan>, after: Option<&DjangoScan>) {
        let empty = DjangoScan::default();
        let before = before.unwrap_or(&empty);
        let after = after.unwrap_or(&empty);
        if before.unresolved_target != after.unresolved_target {
            self.all = true;
        }
        for target in before.targets.difference(&after.targets) {
            self.names.insert(target.clone());
        }
        for target in after.targets.difference(&before.targets) {
            self.names.insert(target.clone());
        }
    }

    /// The modules whose synthesized fields these changes invalidate.
    pub fn stale_readers<'a>(
        &'a self,
        readers: &'a DjangoReaders,
    ) -> Box<dyn Iterator<Item = &'a ModulePath> + 'a> {
        if self.all {
            Box::new(readers.all())
        } else {
            Box::new(self.names.iter().flat_map(|name| readers.of(name)))
        }
    }
}

/// What the first positional argument of a relation constructor names.
enum RelationTarget {
    /// A model we can name syntactically.
    Named(Name),
    /// The literal `"self"`. The accessor lands on the declaring class, so the
    /// declaring module's own relation map already covers it and no
    /// cross-module routing entry is needed.
    SelfReference,
    /// Something we cannot read without inferring types.
    Unresolved,
}

/// Collect the Django relation routing information of a parsed module.
pub fn django_scan(module: &ModModule) -> DjangoScan {
    let mut scan = DjangoScan::default();
    for stmt in &module.body {
        scan_stmt(stmt, &mut scan);
    }
    scan
}

fn scan_stmt(stmt: &Stmt, scan: &mut DjangoScan) {
    if let Stmt::ClassDef(cls) = stmt {
        for member in &cls.body {
            scan_class_member(member, scan);
        }
    }
    // Classes and relation fields can be nested inside functions, `if
    // TYPE_CHECKING` blocks and other classes. Over-approximating costs a
    // wasted lookup; missing one costs a missing attribute.
    stmt.recurse(&mut |stmt| scan_stmt(stmt, scan));
}

/// Record the relation declared by a class-body assignment, if there is one.
fn scan_class_member(stmt: &Stmt, scan: &mut DjangoScan) {
    let value = match stmt {
        Stmt::Assign(x) => Some(&*x.value),
        Stmt::AnnAssign(x) => x.value.as_deref(),
        _ => None,
    };
    let Some(call) = value.and_then(|value| value.as_call_expr()) else {
        return;
    };
    // Match on the callee's last name component, so both `ForeignKey(...)` and
    // `models.ForeignKey(...)` are recognized. An import alias is not, which
    // matches `binding/django.rs`.
    let constructor = match &*call.func {
        Expr::Name(name) => name.id(),
        Expr::Attribute(attr) => attr.attr.id(),
        _ => return,
    };
    if *constructor != FOREIGN_KEY
        && *constructor != ONE_TO_ONE_FIELD
        && *constructor != MANY_TO_MANY_FIELD
    {
        return;
    }
    // The solver reads the target from the first positional argument only
    // (`alt/class/django.rs`), so `ForeignKey(to=Question)` contributes no
    // reverse accessor today. We must ignore it for the same reason.
    match call
        .arguments
        .args
        .first()
        .map_or(RelationTarget::Unresolved, relation_target)
    {
        RelationTarget::Named(name) => {
            scan.targets.insert(name);
        }
        RelationTarget::SelfReference => {}
        RelationTarget::Unresolved => scan.unresolved_target = true,
    }
}

fn relation_target(arg: &Expr) -> RelationTarget {
    match arg {
        Expr::Name(x) => RelationTarget::Named(x.id.clone()),
        Expr::Attribute(x) => RelationTarget::Named(x.attr.id.clone()),
        Expr::StringLiteral(x) => {
            let value = x.value.to_str();
            if value == "self" {
                return RelationTarget::SelfReference;
            }
            // `"app_label.Model"` — the app label is not used for lookup, the
            // same way `resolve_target` discards it.
            RelationTarget::Named(Name::new(
                value.rsplit_once('.').map_or(value, |(_, model)| model),
            ))
        }
        _ => RelationTarget::Unresolved,
    }
}

#[cfg(test)]
mod tests {
    use pyrefly_python::ast::Ast;
    use ruff_python_ast::PySourceType;

    use super::*;

    fn scan(contents: &str) -> DjangoScan {
        let (module, errors, _) = Ast::parse(contents, PySourceType::Python);
        assert!(errors.is_empty(), "test source failed to parse: {errors:?}");
        django_scan(&module)
    }

    fn targets(contents: &str) -> Vec<String> {
        scan(contents)
            .targets
            .into_iter()
            .map(|x| x.to_string())
            .collect()
    }

    #[test]
    fn test_bare_and_qualified_constructors() {
        let scan = scan(
            r#"
class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)
    editor = ForeignKey(Editor)
    place = models.OneToOneField(Place)
    tags = models.ManyToManyField(Tag)
"#,
        );
        assert_eq!(
            scan.targets.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
            vec!["Author", "Editor", "Place", "Tag"]
        );
        assert!(!scan.unresolved_target);
    }

    #[test]
    fn test_dotted_target_keeps_last_component() {
        assert_eq!(
            targets("class B(models.Model):\n    a = models.ForeignKey(auth.models.User)\n"),
            vec!["User"]
        );
    }

    #[test]
    fn test_string_literal_targets() {
        assert_eq!(
            targets("class B(models.Model):\n    a = models.ForeignKey('Author')\n"),
            vec!["Author"]
        );
        assert_eq!(
            targets("class B(models.Model):\n    a = models.ForeignKey('myapp.Author')\n"),
            vec!["Author"]
        );
    }

    #[test]
    fn test_self_reference_is_not_a_routing_target() {
        // The accessor lands on the declaring class, which the module's own
        // relation map already covers.
        let scan = scan("class B(models.Model):\n    parent = models.ForeignKey('self')\n");
        assert!(scan.targets.is_empty());
        assert!(!scan.unresolved_target);
    }

    #[test]
    fn test_unreadable_target_marks_module_as_always_consulted() {
        let scan = scan(
            "class B(models.Model):\n    a = models.ForeignKey(settings.AUTH_USER_MODEL_REF())\n",
        );
        assert!(scan.targets.is_empty());
        assert!(scan.unresolved_target);
    }

    #[test]
    fn test_keyword_target_is_ignored_like_the_solver_does() {
        // `alt/class/django.rs` reads the first positional argument only, so
        // `to=` synthesizes nothing. Recording it here would promise an
        // accessor the solver never produces.
        let scan = scan("class B(models.Model):\n    a = models.ForeignKey(to=Author)\n");
        assert!(scan.targets.is_empty());
        assert!(scan.unresolved_target);
    }

    #[test]
    fn test_annotated_assignment() {
        assert_eq!(
            targets(
                "class B(models.Model):\n    a: ForeignKey[Author] = models.ForeignKey(Author)\n"
            ),
            vec!["Author"]
        );
    }

    #[test]
    fn test_nested_and_conditional_declarations() {
        let scan = scan(
            r#"
if TYPE_CHECKING:
    class Guarded(models.Model):
        a = models.ForeignKey(Author)

def factory():
    class Local(models.Model):
        b = models.ForeignKey(Editor)
"#,
        );
        assert_eq!(
            scan.targets.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
            vec!["Author", "Editor"]
        );
    }

    #[test]
    fn test_non_relation_calls_are_ignored() {
        let scan = scan(
            r#"
class B(models.Model):
    name = models.CharField(max_length=100)
    count = models.IntegerField()
"#,
        );
        assert!(scan.targets.is_empty());
        assert!(!scan.unresolved_target);
    }

    #[test]
    fn test_module_level_assignment_is_not_a_relation() {
        // Only class-body assignments become fields, matching
        // `extract_django_fields_from_class_body`.
        let scan = scan("a = models.ForeignKey(Author)\n");
        assert!(scan.is_empty());
    }

    #[test]
    fn test_relation_tokens_cover_every_recognized_constructor() {
        // The byte prefilter must not be able to skip a file the scan would
        // have found something in.
        for constructor in [FOREIGN_KEY, ONE_TO_ONE_FIELD, MANY_TO_MANY_FIELD] {
            assert!(RELATION_TOKENS.contains(&constructor.as_str()));
        }
    }
}
