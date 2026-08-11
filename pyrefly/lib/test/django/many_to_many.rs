/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::django_testcase;

django_testcase!(
    test_basic,
    r#"
from django.db.models.query import QuerySet
from django.db.models.fields.related_descriptors import ManyRelatedManager
from typing import assert_type
from django.db import models

class Author(models.Model):
    name = models.CharField(max_length=100)

class Book(models.Model):
    title = models.CharField(max_length=200)
    authors = models.ManyToManyField(Author, related_name='books')

book = Book()
assert_type(book.authors, ManyRelatedManager[Author, models.Model])
assert_type(book.authors.all(), QuerySet[Author, Author]) 

assert_type(book.authors.filter(name="Bob"), QuerySet[Author, Author])
assert_type(book.authors.create(name="Alice"), Author) 

book.authors.add("wrong type") # E: Argument `Literal['wrong type']` is not assignable to parameter `*objs` with type `Author | int` 
book.authors.remove(Author())
book.authors.clear()
book.authors.set([Author()])
"#,
);

django_testcase!(
    test_explicit_through_model,
    r#"
from typing import assert_type, reveal_type
from django.db import models
from django.db.models.fields.related_descriptors import ManyRelatedManager

class Author(models.Model):
    pass

class Authorship(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)
    book = models.ForeignKey("Book", on_delete=models.CASCADE)

class Book(models.Model):
    authors = models.ManyToManyField(Author, through=Authorship)

book = Book()
assert_type(book.authors, ManyRelatedManager[Author, Authorship])
assert_type(book.authors.through, type[Authorship])
reveal_type(book.authors.through)  # E: revealed type: type[Authorship]
book.authors.add(Author(), through_defaults={})
book.authors.set([Author()], through_defaults={})
"#,
);
