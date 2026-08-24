/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::django_testcase;

django_testcase!(
    test_auto_generated_id_field,
    r#"
from typing import assert_type

from django.db import models

class Reporter(models.Model):
    name = models.CharField(max_length=100)

reporter = Reporter()
assert_type(reporter.id, int)
assert_type(reporter.pk, int)
"#,
);

django_testcase!(
    test_existing_field,
    r#"
from typing import assert_type

from django.db import models

class Reporter(models.Model):
    id : str = "id"

reporter = Reporter()
assert_type(reporter.id, str)
"#,
);

django_testcase!(
    test_custom_pk,
    r#"
from typing import assert_type
from django.db import models
from uuid import UUID

class Article(models.Model):
    uuid = models.UUIDField(primary_key=True)

class B(Article):
    pass

article = Article()
article.id # E: Object of class `Article` has no attribute `id`
assert_type(article.uuid, UUID)
assert_type(article.pk, UUID)

article2 = B()
article2.id # E: Object of class `B` has no attribute `id`
assert_type(article2.uuid, UUID)
assert_type(article2.pk, UUID)
"#,
);

django_testcase!(
    test_abstract_model_charfield_pk,
    r#"
from typing import assert_type
from django.db import models

class StrIdMixin(models.Model):
    id = models.CharField(max_length=36, primary_key=True)
    class Meta:
        abstract = True

class StrIdChildModel(StrIdMixin):
    name = models.CharField(max_length=100)

child = StrIdChildModel()
assert_type(child.id, str)
assert_type(child.pk, str)
"#,
);

// Multiple abstract mixins: the one with primary_key=True must win,
// even if a later base has no custom PK (regression test for #2218).
django_testcase!(
    test_abstract_model_charfield_pk_multiple_mixins,
    r#"
from typing import assert_type
from django.db import models

class StrIdMixin(models.Model):
    id = models.CharField(max_length=36, primary_key=True)
    class Meta:
        abstract = True

class AuditMixin(models.Model):
    created_by = models.CharField(max_length=100)
    class Meta:
        abstract = True

class ConcreteModel(StrIdMixin, AuditMixin):
    name = models.CharField(max_length=100)

obj = ConcreteModel()
assert_type(obj.id, str)
assert_type(obj.pk, str)
"#,
);

// Django adds `get_next_by_FOO()` / `get_previous_by_FOO()` for every date and datetime field that
// cannot be null. Both return the neighbouring instance of the declaring model.
django_testcase!(
    test_get_next_and_previous_by_date_field,
    r#"
from typing import assert_type

from django.db import models

class Person(models.Model):
    birthday = models.DateField()
    created = models.DateTimeField()

person = Person()
assert_type(person.get_next_by_birthday(), Person)
assert_type(person.get_previous_by_birthday(), Person)
assert_type(person.get_next_by_created(), Person)
assert_type(person.get_previous_by_created(), Person)
# Django forwards extra keywords to the underlying `filter()` call.
assert_type(person.get_next_by_birthday(created__gt=person.created), Person)
"#,
);

django_testcase!(
    test_no_get_next_by_for_nullable_or_non_date_fields,
    r#"
from django.db import models

class Person(models.Model):
    # A nullable column has no defined neighbour, so Django adds no accessor.
    maybe_birthday = models.DateField(null=True)
    name = models.CharField(max_length=60)

person = Person()
person.get_next_by_maybe_birthday  # E: Object of class `Person` has no attribute `get_next_by_maybe_birthday`
person.get_next_by_name  # E: Object of class `Person` has no attribute `get_next_by_name`
"#,
);

django_testcase!(
    test_get_next_by_with_explicit_null_false,
    r#"
from typing import assert_type

from django.db import models

class Person(models.Model):
    birthday = models.DateField(null=False)

assert_type(Person().get_next_by_birthday(), Person)
"#,
);

// The accessors follow the field's resolved type, not its constructor name: a subclass of
// `DateField` keeps them, and a class that merely looks like one does not get them.
django_testcase!(
    test_get_next_by_follows_resolved_field_type,
    r#"
from typing import assert_type

from django.db import models

class AuditDateField(models.DateField): ...

class NotReallyADateField(models.CharField): ...

class Person(models.Model):
    reviewed = AuditDateField()
    label = NotReallyADateField()

assert_type(Person().get_next_by_reviewed(), Person)
Person().get_next_by_label  # E: Object of class `Person` has no attribute `get_next_by_label`
"#,
);
