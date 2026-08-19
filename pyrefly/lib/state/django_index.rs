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

use crate::binding::django::django_relation_target;

const FOREIGN_KEY: Name = Name::new_static("ForeignKey");
const ONE_TO_ONE_FIELD: Name = Name::new_static("OneToOneField");
const MANY_TO_MANY_FIELD: Name = Name::new_static("ManyToManyField");

/// The substrings that must appear in a file's source for it to be able to
/// declare a relation. Checking these against the raw bytes lets us skip
/// parsing the overwhelming majority of a project's files.
pub const RELATION_TOKENS: [&str; 3] = ["ForeignKey", "OneToOneField", "ManyToManyField"];

/// The relation-routing information contributed by one module.
#[derive(Debug, Clone)]
pub struct DjangoModuleData {
    pub module_name: ModuleName,
    pub relation_targets: SmallSet<Name>,
}

/// The project-wide routing table.
///
/// Immutable once built. A refresh produces a new one from updated module
/// information, so a transaction that is never committed simply drops its copy.
#[derive(Debug, Default)]
pub struct DjangoRelationIndex {
    /// Only modules with something to contribute are stored; a module absent
    /// here has been scanned and found empty.
    modules: SmallMap<ModulePath, DjangoModuleData>,
    /// Model short name to the modules that may declare a relation to it,
    /// sorted for deterministic merge order.
    modules_by_relation_target: SmallMap<Name, Vec<ModulePath>>,
}

impl DjangoRelationIndex {
    pub fn new(module_data_by_path: SmallMap<ModulePath, DjangoModuleData>) -> Self {
        let mut module_paths_by_relation_target: SmallMap<Name, Vec<ModulePath>> = SmallMap::new();
        for (path, data) in &module_data_by_path {
            for model_name in &data.relation_targets {
                module_paths_by_relation_target
                    .entry(model_name.clone())
                    .or_default()
                    .push(path.dupe());
            }
        }
        for paths in module_paths_by_relation_target.values_mut() {
            // Sort by rendered names because `ModuleName`'s derived ordering
            // compares interned pointers and varies between runs.
            paths.sort_by_key(|path| {
                let data = module_data_by_path
                    .get(path)
                    .expect("the reverse index must reference an indexed module");
                (data.module_name.as_str().to_owned(), path.to_string())
            });
        }
        Self {
            modules: module_data_by_path,
            modules_by_relation_target: module_paths_by_relation_target,
        }
    }

    pub fn module_data_by_path(&self) -> &SmallMap<ModulePath, DjangoModuleData> {
        &self.modules
    }

    /// The modules that may declare a reverse accessor on `model_name`, in a
    /// deterministic order.
    pub fn modules_for_relation_target(&self, model_name: &Name) -> Vec<(ModuleName, ModulePath)> {
        self.modules_by_relation_target
            .get(model_name)
            .into_iter()
            .flatten()
            .map(|path| {
                let data = self
                    .modules
                    .get(path)
                    .expect("the reverse index must reference an indexed module");
                (data.module_name, path.dupe())
            })
            .collect()
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
}

/// Accumulates the target names whose candidate set changed during a refresh.
#[derive(Debug, Default)]
pub struct ChangedTargets {
    names: SmallSet<Name>,
}

impl ChangedTargets {
    /// Record the difference a rescan of one file made.
    pub fn record(&mut self, before: Option<&SmallSet<Name>>, after: Option<&SmallSet<Name>>) {
        let empty = SmallSet::new();
        let before = before.unwrap_or(&empty);
        let after = after.unwrap_or(&empty);
        for target in before.difference(after) {
            self.names.insert(target.clone());
        }
        for target in after.difference(before) {
            self.names.insert(target.clone());
        }
    }

    /// The modules whose synthesized fields these changes invalidate.
    pub fn stale_readers<'a>(
        &'a self,
        readers: &'a DjangoReaders,
    ) -> impl Iterator<Item = &'a ModulePath> + 'a {
        self.names.iter().flat_map(|name| readers.of(name))
    }
}

/// Collect the Django relation routing information of a parsed module.
pub fn django_scan(module: &ModModule) -> SmallSet<Name> {
    let mut scan = SmallSet::new();
    for stmt in &module.body {
        scan_stmt(stmt, &mut scan);
    }
    scan
}

fn scan_stmt(stmt: &Stmt, scan: &mut SmallSet<Name>) {
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
fn scan_class_member(stmt: &Stmt, scan: &mut SmallSet<Name>) {
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
    let Some(target) = django_relation_target(call) else {
        return;
    };
    if let Some(name) = relation_target(target) {
        scan.insert(name);
    }
}

fn relation_target(arg: &Expr) -> Option<Name> {
    match arg {
        Expr::Name(x) => Some(x.id.clone()),
        Expr::Attribute(x) => Some(x.attr.id.clone()),
        Expr::StringLiteral(x) => {
            let value = x.value.to_str();
            if value == "self" {
                // The accessor lands on the declaring class, so the module's
                // own relation map already covers it.
                return None;
            }
            // `"app_label.Model"` — the app label is not used for lookup, the
            // same way `resolve_target` discards it.
            Some(Name::new(
                value.rsplit_once('.').map_or(value, |(_, model)| model),
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use pyrefly_python::ast::Ast;
    use ruff_python_ast::PySourceType;

    use super::*;

    fn scan(contents: &str) -> SmallSet<Name> {
        let (module, errors, _) = Ast::parse(contents, PySourceType::Python);
        assert!(errors.is_empty(), "test source failed to parse: {errors:?}");
        django_scan(&module)
    }

    fn targets(contents: &str) -> Vec<String> {
        scan(contents).into_iter().map(|x| x.to_string()).collect()
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
            scan.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
            vec!["Author", "Editor", "Place", "Tag"]
        );
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
        assert!(scan.is_empty());
    }

    #[test]
    fn test_unreadable_target_is_ignored() {
        let scan = scan(
            "class B(models.Model):\n    a = models.ForeignKey(settings.AUTH_USER_MODEL_REF())\n",
        );
        assert!(scan.is_empty());
    }

    #[test]
    fn test_keyword_target_is_indexed() {
        assert_eq!(
            targets("class B(models.Model):\n    a = models.ForeignKey(to=Author)\n"),
            vec!["Author"]
        );
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
            scan.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
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
        assert!(scan.is_empty());
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
