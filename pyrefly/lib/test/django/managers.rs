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
