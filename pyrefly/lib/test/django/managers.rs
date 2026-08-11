/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Django binds a manager to the model whose body declares it. django-stubs cannot express that
//! on its own — a manager class names no model — so these tests pin down how much of the model
//! reaches the manager, and through which of `objects`, a custom manager, `as_manager()` or a
//! `filter()` chain.

use crate::django_testcase;

django_testcase!(
    test_default_manager_carries_model,
    r#"
from typing import assert_type
from django.db import models
from django.db.models.manager import Manager
from django.db.models.query import QuerySet

class Article(models.Model):
    title = models.CharField(max_length=10)

assert_type(Article.objects, Manager[Article])
assert_type(Article._default_manager, Manager[Article])
assert_type(Article._base_manager, Manager[Article])
assert_type(Article.objects.get(), Article)
assert_type(Article.objects.first(), Article | None)
assert_type(Article.objects.all(), QuerySet[Article, Article])
assert_type(Article.objects.filter(id=1), QuerySet[Article, Article])
assert_type(Article.objects.all().order_by("title"), QuerySet[Article, Article])
assert_type(Article.objects.all()[0], Article)
assert_type(Article.objects.model, type[Article])
"#,
);

django_testcase!(
    test_generic_custom_manager_specialized,
    r#"
from typing import assert_type
from django.db import models

class ArticleManager[T: models.Model](models.Manager[T]):
    def published(self) -> models.QuerySet[T, T]: ...

class Article(models.Model):
    objects = ArticleManager()

assert_type(Article.objects, ArticleManager[Article])
assert_type(Article.objects.get(), Article)
assert_type(Article.objects.published(), models.QuerySet[Article, Article])
"#,
);

django_testcase!(
    test_bare_custom_manager_specialized,
    r#"
from typing import assert_type
from django.db import models

class ArticleManager(models.Manager):
    def published(self) -> str: ...

class Article(models.Model):
    objects = ArticleManager()

# The manager's own methods stay reachable...
assert_type(Article.objects.published(), str)
# ...and the model is recovered for the inherited, model-parameterized ones.
assert_type(Article.objects.get(), Article)
"#,
);

django_testcase!(
    test_multiple_managers,
    r#"
from typing import assert_type
from django.db import models

class FirstManager[T: models.Model](models.Manager[T]): ...
class SecondManager[T: models.Model](models.Manager[T]): ...

class Article(models.Model):
    objects = FirstManager()
    special = SecondManager()

assert_type(Article.objects, FirstManager[Article])
assert_type(Article.special, SecondManager[Article])
"#,
);

django_testcase!(
    test_manager_already_parameterized,
    r#"
from typing import assert_type
from django.db import models

class ArticleManager[T: models.Model](models.Manager[T]): ...

class Author(models.Model): ...

class Article(models.Model):
    # An explicit parameter is the user telling us which model this manager serves, so it is kept
    # rather than overwritten with the enclosing model.
    authors = ArticleManager[Author]()

assert_type(Article.authors, ArticleManager[Author])
assert_type(Article.authors.get(), Author)
"#,
);

django_testcase!(
    bug = "A manager declared on an abstract base binds to the base, not to each concrete subclass",
    test_manager_inherited_from_abstract_model,
    r#"
from typing import assert_type
from django.db import models

class ArticleManager[T: models.Model](models.Manager[T]): ...

class Base(models.Model):
    objects = ArticleManager()

    class Meta:
        abstract = True

class Article(Base): ...

assert_type(Article.objects, ArticleManager[Base])
"#,
);

django_testcase!(
    test_as_manager_carries_model_and_queryset_methods,
    r#"
from typing import assert_type
from django.db import models

class ArticleQuerySet(models.QuerySet):
    def published(self) -> str: ...

class Article(models.Model):
    objects = ArticleQuerySet.as_manager()

assert_type(Article.objects.get(), Article)
assert_type(Article.objects.published(), str)
"#,
);

django_testcase!(
    bug = "from_queryset() loses the queryset's own methods",
    test_from_queryset_carries_model,
    r#"
from typing import assert_type
from django.db import models

class ArticleQuerySet(models.QuerySet):
    def published(self) -> str: ...

ArticleManager = models.Manager.from_queryset(ArticleQuerySet)

class Article(models.Model):
    objects = ArticleManager()

# The model is recovered...
assert_type(Article.objects.get(), Article)
# ...but `from_queryset` is declared as `-> type[Self]`, so the concrete queryset is erased before
# we see the assignment, unlike `as_manager()` where the queryset is named at the call site.
assert_type(Article.objects.published(), str)  # E: assert_type(Unknown, str) failed # E: Object of class `Manager` has no attribute `published`
"#,
);

django_testcase!(
    test_manager_union_propagation,
    r#"
from typing import assert_type
from django.db import models
from django.db.models.manager import Manager

class Article(models.Model): ...

class ArticleManager[T: models.Model](models.Manager[T]): ...

def f(m: ArticleManager[Article] | Manager[Article]) -> None:
    assert_type(m.get(), Article)
    assert_type(m.filter(id=1), models.QuerySet[Article, Article])
"#,
);
