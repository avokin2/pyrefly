/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::django_testcase;

django_testcase!(
    test_default_manager_preserves_model_type,
    r#"
from typing import assert_type
from django.db import models
from django.db.models.manager import Manager
from django.db.models.query import QuerySet

class Book(models.Model):
    title = models.CharField(max_length=100)

assert_type(Book.objects, Manager[Book])
assert_type(Book.objects.get(), Book)
assert_type(Book.objects.first(), Book | None)
assert_type(Book.objects.filter(title="Pyrefly"), QuerySet[Book, Book])
assert_type(Book.objects.all().order_by("title"), QuerySet[Book, Book])
assert_type(Book.objects.all()[0], Book)
assert_type(Book.objects.all() | Book.objects.filter(), QuerySet[Book, Book])
"#,
);

django_testcase!(
    test_custom_manager_preserves_model_type,
    r#"
from typing import assert_type
from django.db import models
from django.db.models.query import QuerySet

class BookQuerySet(models.QuerySet["Book"]):
    def published(self) -> QuerySet["Book"]:
        return self.filter()

class BookManager(models.Manager["Book"]):
    def published(self) -> QuerySet["Book"]:
        return self.filter()

class Book(models.Model):
    objects = BookManager()

assert_type(Book.objects, BookManager)
assert_type(Book.objects.get(), Book)
assert_type(Book.objects.published(), QuerySet[Book, Book])
"#,
);

django_testcase!(
    test_queryset_as_manager_preserves_model_type,
    r#"
from typing import assert_type
from django.db import models
from django.db.models.manager import Manager
from django.db.models.query import QuerySet

class Book(models.Model):
    pass

manager = QuerySet[Book].as_manager()
assert_type(manager, Manager[Book])
assert_type(manager.get(), Book)
assert_type(manager.filter(), QuerySet[Book, Book])
"#,
);

// A manager declared in a model body has to beat django-stubs' `objects: ClassVar[Manager[Self]]`,
// which would otherwise erase both the manager class and the model.
django_testcase!(
    test_as_manager_in_model_body_carries_queryset_methods,
    r#"
from typing import assert_type
from django.db import models

class ArticleQuerySet(models.QuerySet):
    def published(self) -> str: ...

class Article(models.Model):
    objects = ArticleQuerySet.as_manager()

# The queryset's own methods stay callable...
assert_type(Article.objects.published(), str)
# ...and the inherited, model-parameterized ones resolve to the enclosing model.
assert_type(Article.objects.get(), Article)
assert_type(Article.objects.filter(), models.QuerySet[Article, Article])
"#,
);

django_testcase!(
    test_bare_custom_manager_in_model_body,
    r#"
from typing import assert_type
from django.db import models

# No type parameter to bind the model to, so `BareManager[Article]` is unspellable.
class BareManager(models.Manager):
    def newest(self) -> str: ...

class Article(models.Model):
    objects = BareManager()

assert_type(Article.objects.newest(), str)
assert_type(Article.objects.get(), Article)
"#,
);

django_testcase!(
    test_generic_custom_manager_specialized_with_model,
    r#"
from typing import assert_type
from django.db import models

class ArticleManager[T: models.Model](models.Manager[T]):
    def published(self) -> models.QuerySet[T, T]: ...

class Article(models.Model):
    objects = ArticleManager()

assert_type(Article.objects, ArticleManager[Article])
assert_type(Article.objects.published(), models.QuerySet[Article, Article])
"#,
);

django_testcase!(
    test_explicitly_parameterized_manager_is_left_alone,
    r#"
from typing import assert_type
from django.db import models

class ArticleManager[T: models.Model](models.Manager[T]): ...

class Author(models.Model): ...

class Article(models.Model):
    # An explicit parameter is the author saying which model this manager serves, so it is kept
    # rather than overwritten with the enclosing model.
    authors = ArticleManager[Author]()

assert_type(Article.authors, ArticleManager[Author])
assert_type(Article.authors.get(), Author)
"#,
);

// Django's `_get_queryset_methods` copies only the public methods that are not marked
// `queryset_only`, so neither hidden method below exists on the manager at runtime. The
// intersection we build from `as_manager()` cannot express that exclusion, so both leak.
django_testcase!(
    bug = "as_manager() exposes queryset_only and underscore-prefixed queryset methods",
    test_as_manager_hides_queryset_only_methods,
    r#"
from django.db import models

class ArticleQuerySet(models.QuerySet):
    def visible(self) -> str: ...

    def queryset_only_method(self) -> str: ...
    queryset_only_method.queryset_only = True  # E: Object of class `FunctionType` has no attribute `queryset_only`

    def _private(self) -> str: ...

class Article(models.Model):
    objects = ArticleQuerySet.as_manager()

Article.objects.visible()
# Both of these should be errors -- Django does not copy them onto the manager.
Article.objects.queryset_only_method()
Article.objects._private()
"#,
);
