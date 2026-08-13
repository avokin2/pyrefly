/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::path::PathBuf;
use std::sync::Arc;

use dupe::Dupe;
use pyrefly_build::handle::Handle;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_util::thread_pool::TEST_THREAD_COUNT;

use crate::binding::binding::KeyClassSynthesizedFields;
use crate::django_testcase;
use crate::state::load::FileContents;
use crate::state::require::Require;
use crate::state::state::State;
use crate::test::django::util::django_env;
use crate::test::util::TestEnv;
use crate::test::util::get_class;
use crate::testcase;

/// A target model living in a module of its own, so that the reverse accessor
/// has to be found through the project-wide relation index.
fn django_env_with_separate_models() -> TestEnv {
    let mut env = django_env();
    env.add(
        "author",
        r#"
from django.db import models

class Author(models.Model):
    name = models.CharField(max_length=100)
"#,
    );
    env
}

fn django_env_with_module(name: &str, code: &str) -> TestEnv {
    let mut env = django_env();
    env.add(name, code);
    env
}

fn django_env_without_auto_field() -> TestEnv {
    let mut env = TestEnv::new();
    env.add("django", "");
    env.add("django.db", "");
    env.add(
        "django.db.models",
        r#"
from django.db.models.base import Model
"#,
    );
    env.add("django.db.models.base", "class Model: pass");
    env
}

testcase!(
    test_missing_auto_field_does_not_panic,
    django_env_without_auto_field(),
    r#"
from django.db import models

class Author(models.Model):
    pass

Author()
"#,
);

django_testcase!(
    test_foreign_key_reverse_default_name,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type

class Reporter(models.Model):
    full_name = models.CharField(max_length=70)

class Article(models.Model):
    reporter = models.ForeignKey(Reporter, on_delete=models.CASCADE)

reporter = Reporter()
# Default reverse name is <model_lowercase>_set
assert_type(reporter.article_set, RelatedManager[Article])
"#,
);

django_testcase!(
    test_nullable_foreign_key_reverse_manager_methods,
    r#"
from django.db import models

class Reporter(models.Model):
    pass

class Article(models.Model):
    reporter = models.ForeignKey(
        Reporter,
        null=True,
        on_delete=models.CASCADE,
    )

reporter = Reporter()
article = Article()
reporter.article_set.add(article)
reporter.article_set.remove(article)
reporter.article_set.clear()
reporter.article_set.set([article])
reporter.article_set.add("wrong")  # E: Argument `Literal['wrong']` is not assignable to parameter `*objs` with type `Article | int`
reporter.article_set.set(["wrong"])  # E: Argument `list[str]` is not assignable to parameter `objs` with type `Iterable[Article | int] | QuerySet[Article, Article]`
"#,
);

django_testcase!(
    test_foreign_key_reverse_custom_name,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type

class Author(models.Model):
    name = models.CharField(max_length=100)

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='written_books')

author = Author()
# Custom related_name should be used instead of default
assert_type(author.written_books, RelatedManager[Book])
"#,
);

django_testcase!(
    test_foreign_key_reverse_disabled,
    r#"
from django.db import models

class Author(models.Model):
    name = models.CharField(max_length=100)

class Book(models.Model):
    # related_name='+' disables the reverse accessor entirely
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='+')

author = Author()
# No reverse accessor should exist
author.book_set  # E: `Author` has no attribute `book_set`
"#,
);

// An attribute whose name is not an identifier is unreachable from Python code, so the
// only way to observe one is to look at the synthesized fields directly.
#[test]
fn test_foreign_key_reverse_invalid_identifier_synthesizes_nothing() {
    let mut env = django_env();
    env.add(
        "main",
        r#"
from django.db import models

class Author(models.Model): ...

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='written_books')

class Magazine(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='123articles')
"#,
    );
    let (state, handle_for) = env.to_state();
    let handle = handle_for("main");
    let author = get_class("Author", &handle, &state);
    let solutions = state.transaction().get_solutions(&handle).unwrap();
    let fields = solutions.get(&KeyClassSynthesizedFields(author.index()));
    let mut names = fields
        .fields()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    names.sort();
    // `123articles` is dropped entirely rather than synthesized under an unusable name.
    assert_eq!(names, vec!["id", "pk", "written_books"]);
}

// A related name that is not a valid identifier names an attribute that can never be
// accessed, so no reverse relation is created at all -- not even under the default name.
django_testcase!(
    test_foreign_key_reverse_invalid_identifier,
    r#"
from django.db import models

class Author(models.Model):
    name = models.CharField(max_length=100)

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name=' written_books ')

class Magazine(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='123articles')

class Poem(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='written.poems')

author = Author()
author.written_books  # E: `Author` has no attribute `written_books`
author.book_set  # E: `Author` has no attribute `book_set`
author.magazine_set  # E: `Author` has no attribute `magazine_set`
author.poem_set  # E: `Author` has no attribute `poem_set`
"#,
);

django_testcase!(
    test_foreign_key_reverse_unknown_app_label,
    r#"
from django.db import models

class Author(models.Model): ...

class Book(models.Model):
    author = models.ForeignKey(
        Author,
        on_delete=models.CASCADE,
        related_name='%(app_label)s_books',
    )

author = Author()
author.main_books  # E: `Author` has no attribute `main_books`
"#,
);

testcase!(
    test_foreign_key_reverse_app_label,
    django_env_with_module(
        "myapp.models",
        r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type

class Author(models.Model): ...

class Book(models.Model):
    author = models.ForeignKey(
        Author,
        on_delete=models.CASCADE,
        related_name='%(app_label)s_%(class)s_books',
    )

author = Author()
assert_type(author.myapp_book_books, RelatedManager[Book])
"#
    ),
    "",
);

// Self-referential FK creates reverse accessor on the same model
django_testcase!(
    test_foreign_key_reverse_self_reference,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type

class Person(models.Model):
    name = models.CharField(max_length=100)
    # Self-referential FK: a person can have a parent who is also a Person
    parent = models.ForeignKey('self', null=True, on_delete=models.CASCADE)

person = Person()
assert_type(person.person_set, RelatedManager[Person])
"#,
);

django_testcase!(
    test_foreign_key_reverse_unicode_default_name,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type

class Reporter(models.Model): ...

class ÜberBook(models.Model):
    reporter = models.ForeignKey(Reporter, on_delete=models.CASCADE)

reporter = Reporter()
assert_type(reporter.überbook_set, RelatedManager[ÜberBook])
"#,
);

testcase!(
    test_foreign_key_reverse_cross_module,
    django_env_with_separate_models(),
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type
from .author import Author

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)

author = Author()
assert_type(author.book_set, RelatedManager[Book])
"#,
);

// OneToOneField reverse relation: returns single object (not a manager like FK)
// Default name is just the lowercase model name without `_set`
django_testcase!(
    test_one_to_one_reverse_default_name,
    r#"
from django.db import models
from typing import assert_type

class Place(models.Model):
    name = models.CharField(max_length=50)

class Restaurant(models.Model):
    place = models.OneToOneField(Place, on_delete=models.CASCADE)

place = Place()
# OneToOne reverse is just the lowercase model name (no _set suffix)
assert_type(place.restaurant, Restaurant)
"#,
);

// ManyToManyField reverse relation: returns a manager like FK
django_testcase!(
    test_many_to_many_reverse_default_name,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import ManyRelatedManager
from typing import assert_type

class Tag(models.Model):
    name = models.CharField(max_length=50)

class Article(models.Model):
    tags = models.ManyToManyField(Tag)

tag = Tag()
# ManyToMany default reverse name is <model_lowercase>_set
assert_type(tag.article_set, ManyRelatedManager[Article, models.Model])
"#,
);

django_testcase!(
    test_one_to_one_reverse_custom_name,
    r#"
from django.db import models
from typing import assert_type

class Place(models.Model):
    name = models.CharField(max_length=50)

class Restaurant(models.Model):
    place = models.OneToOneField(Place, on_delete=models.CASCADE, related_name='dining_spot')

place = Place()
assert_type(place.dining_spot, Restaurant)
"#,
);

django_testcase!(
    test_one_to_one_reverse_disabled,
    r#"
from django.db import models

class Place(models.Model):
    name = models.CharField(max_length=50)

class Restaurant(models.Model):
    place = models.OneToOneField(Place, on_delete=models.CASCADE, related_name='+')

place = Place()
# No reverse accessor should exist
place.restaurant  # E: `Place` has no attribute `restaurant`
"#,
);

django_testcase!(
    test_many_to_many_reverse_custom_name,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import ManyRelatedManager
from typing import assert_type

class Tag(models.Model):
    name = models.CharField(max_length=50)

class Article(models.Model):
    tags = models.ManyToManyField(Tag, related_name='tagged_articles')

tag = Tag()
assert_type(tag.tagged_articles, ManyRelatedManager[Article, models.Model])
"#,
);

django_testcase!(
    test_many_to_many_reverse_explicit_through_model,
    r#"
from typing import assert_type
from django.db import models
from django.db.models.fields.related_descriptors import ManyRelatedManager

class Author(models.Model):
    pass

class Book(models.Model):
    authors = models.ManyToManyField(
        Author,
        through="Authorship",
        related_name="books",
    )

class Authorship(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)
    book = models.ForeignKey(Book, on_delete=models.CASCADE)

author = Author()
assert_type(author.books, ManyRelatedManager[Book, Authorship])
assert_type(author.books.through, type[Authorship])
"#,
);

django_testcase!(
    test_many_to_many_reverse_disabled,
    r#"
from django.db import models

class Tag(models.Model):
    name = models.CharField(max_length=50)

class Article(models.Model):
    tags = models.ManyToManyField(Tag, related_name='+')

tag = Tag()
# No reverse accessor should exist
tag.article_set  # E: `Tag` has no attribute `article_set`
"#,
);

// Self-referential ManyToMany is symmetrical by default, meaning no reverse accessor
// is created because the relation is bidirectional through the same field
django_testcase!(
    test_many_to_many_self_reference_symmetrical,
    r#"
from django.db import models

class Person(models.Model):
    name = models.CharField(max_length=100)
    # Symmetrical M2M: friends is accessible from both sides via the same field
    friends = models.ManyToManyField('self')

person = Person()
# No person_set because symmetrical=True (default for self-referential M2M)
person.person_set  # E: `Person` has no attribute `person_set`
"#,
);

django_testcase!(
    test_many_to_many_self_reference_asymmetrical,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import ManyRelatedManager
from typing import assert_type

class Person(models.Model):
    name = models.CharField(max_length=100)
    # Asymmetrical M2M: followers vs following relationship
    following = models.ManyToManyField('self', symmetrical=False, related_name='followers')

person = Person()
# With symmetrical=False, reverse accessor is created
assert_type(person.followers, ManyRelatedManager[Person, models.Model])
"#,
);

django_testcase!(
    test_many_to_many_self_reference_dynamic_symmetry,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import ManyRelatedManager
from typing import assert_type

symmetrical = False

class Person(models.Model):
    following = models.ManyToManyField('self', symmetrical=symmetrical, related_name='followers')

person = Person()
assert_type(person.followers, ManyRelatedManager[Person, models.Model])
"#,
);

testcase!(
    test_one_to_one_reverse_cross_module,
    django_env_with_separate_models(),
    r#"
from django.db import models
from typing import assert_type
from .author import Author

class Profile(models.Model):
    author = models.OneToOneField(Author, on_delete=models.CASCADE)

author = Author()
assert_type(author.profile, Profile)
"#,
);

testcase!(
    test_many_to_many_reverse_cross_module,
    django_env_with_separate_models(),
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import ManyRelatedManager
from typing import assert_type
from .author import Author

class Anthology(models.Model):
    authors = models.ManyToManyField(Author)

author = Author()
assert_type(author.anthology_set, ManyRelatedManager[Anthology, models.Model])
"#,
);

testcase!(
    test_foreign_key_reverse_cross_module_custom_name,
    django_env_with_separate_models(),
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type
from .author import Author

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='written_books')

author = Author()
assert_type(author.written_books, RelatedManager[Book])
"#,
);

testcase!(
    test_foreign_key_reverse_cross_module_disabled,
    django_env_with_separate_models(),
    r#"
from django.db import models
from .author import Author

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='+')

author = Author()
author.book_set  # E: `Author` has no attribute `book_set`
"#,
);

// A string forward reference resolves against the exports of the module that
// declares the relation, so it reaches a model defined elsewhere as long as the
// name is imported here.
testcase!(
    test_foreign_key_reverse_cross_module_string_target,
    django_env_with_separate_models(),
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type
from .author import Author

class Book(models.Model):
    author = models.ForeignKey('Author', on_delete=models.CASCADE)

author = Author()
assert_type(author.book_set, RelatedManager[Book])
"#,
);

// Two source modules add accessors to the same target, and the target module
// itself is a third one.
#[test]
fn test_foreign_key_reverse_from_several_modules() {
    let mut env = django_env_with_separate_models();
    env.add(
        "library",
        r#"
from django.db import models
from author import Author

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)
"#,
    );
    env.add(
        "press",
        r#"
from django.db import models
from author import Author

class Magazine(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)
"#,
    );
    env.add(
        "main",
        r#"
from author import Author

author = Author()
author.book_set
author.magazine_set
"#,
    );
    let (state, handle_for) = env.to_state();
    let handle = handle_for("main");
    let errors = state.transaction().get_errors([&handle]);
    assert_eq!(
        errors.collect_errors().ordinary.len(),
        0,
        "accessors from both source modules should exist: {:?}",
        errors.collect_errors().ordinary
    );
}

// The routing table keys on the model's short name, so two same-named models in
// different modules land in the same bucket. The relation maps key on the exact
// class, so the accessor must not leak from one to the other.
#[test]
fn test_same_named_models_do_not_share_reverse_accessors() {
    let mut env = django_env_with_separate_models();
    env.add(
        "other",
        r#"
from django.db import models

class Author(models.Model): ...

class Note(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)
"#,
    );
    env.add(
        "main",
        r#"
from author import Author

author = Author()
author.note_set
"#,
    );
    let (state, handle_for) = env.to_state();
    let handle = handle_for("main");
    let errors = state.transaction().get_errors([&handle]);
    let shown = errors.collect_errors().ordinary;
    assert_eq!(
        shown.len(),
        1,
        "`note_set` belongs to `other.Author`, not `author.Author`: {shown:?}"
    );
}

// Models in two modules pointing at each other must not deadlock or lose their
// accessors to a solve cycle.
#[test]
fn test_mutually_referencing_modules() {
    let mut env = django_env();
    env.add(
        "left",
        r#"
from django.db import models
from right import Right

class Left(models.Model):
    right = models.ForeignKey(Right, on_delete=models.CASCADE)
"#,
    );
    env.add(
        "right",
        r#"
from django.db import models

class Right(models.Model): ...
"#,
    );
    env.add(
        "main",
        r#"
from left import Left
from right import Right

Right().left_set
"#,
    );
    let (state, handle_for) = env.to_state();
    let handle = handle_for("main");
    let errors = state.transaction().get_errors([&handle]);
    let shown = errors.collect_errors().ordinary;
    assert_eq!(shown.len(), 0, "{shown:?}");
}

/// A relation appearing in a module the target does not import must still reach
/// the target on a recheck. No dependency edge exists yet at the moment the
/// relation is added, so this only works if refreshing the relation index marks
/// the modules defining the target as needing recomputation.
#[test]
fn test_relation_added_in_another_module_reaches_the_target() {
    let mut env = django_env();
    env.add(
        "author",
        r#"
from django.db import models

class Author(models.Model): ...
"#,
    );
    env.add("book", "");
    env.add(
        "main",
        r#"
from author import Author

Author().book_set
"#,
    );
    let sys_info = env.sys_info();
    let handles = ["author", "book", "main"]
        .map(|name| {
            Handle::new(
                ModuleName::from_str(name),
                ModulePath::memory(PathBuf::from(format!("{name}.py"))),
                sys_info.dupe(),
            )
        })
        .to_vec();
    let state = State::new(env.config_finder(), TEST_THREAD_COUNT);

    let run = |memory: Vec<(PathBuf, Option<Arc<FileContents>>)>| {
        let mut transaction = state.new_committable_transaction(Require::Exports, None);
        transaction.as_mut().set_memory(memory);
        state.run_with_committing_transaction(
            transaction,
            &handles,
            Require::Everything,
            None,
            None,
        );
        state
            .transaction()
            .get_errors(&handles)
            .collect_errors()
            .ordinary
            .len()
    };

    let book_with_relation = r#"
from django.db import models
from author import Author

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)
"#;
    let set_book = |contents: &str| {
        vec![(
            PathBuf::from("book.py"),
            Some(Arc::new(FileContents::from_source(contents.to_owned()))),
        )]
    };

    assert_eq!(run(env.get_memory()), 1, "`book_set` should not exist yet");
    assert_eq!(
        run(set_book(book_with_relation)),
        0,
        "adding the relation should make `book_set` appear on `Author`"
    );
    assert_eq!(
        run(set_book("")),
        1,
        "removing the relation should make `book_set` disappear again"
    );
}

// The scan matches the target by name, so an alias is invisible to it and the
// accessor is not synthesized across modules. Documented in `django.mdx`.
testcase!(
    test_foreign_key_reverse_cross_module_alias_target,
    django_env_with_separate_models(),
    r#"
from django.db import models
from .author import Author

MyAlias = Author

class Book(models.Model):
    author = models.ForeignKey(MyAlias, on_delete=models.CASCADE)

author = Author()
author.book_set  # E: `Author` has no attribute `book_set`
"#,
);

// The solver reads the target from the first positional argument only, so a
// keyword target synthesizes nothing even within one module.
django_testcase!(
    test_foreign_key_reverse_keyword_target,
    r#"
from django.db import models

class Author(models.Model): ...

class Book(models.Model):
    author = models.ForeignKey(to=Author, on_delete=models.CASCADE)

author = Author()
author.book_set  # E: `Author` has no attribute `book_set`
"#,
);
