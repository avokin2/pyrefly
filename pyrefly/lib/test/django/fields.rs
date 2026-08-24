/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::django_testcase;

django_testcase!(
    test_textfield_nullable,
    r#"
from django.db import models

class Group(models.Model):
    name = models.TextField()

class Customer(models.Model):
    name = models.TextField(null=True)

def test():
    c = Customer.objects.create()
    c.name = None
"#,
);

django_testcase!(
    test_get_foo_display,
    r#"
from typing import assert_type

from django.db import models

class Person(models.Model):
    SHIRT_SIZES = {
        "S": "Small",
        "M": "Medium",
        "L": "Large",
    }
    name = models.CharField(max_length=60)
    shirt_size = models.CharField(max_length=2, choices=SHIRT_SIZES)

p = Person(name="Fred Flintstone", shirt_size="L")
p.save()
assert_type(p.shirt_size, str) 
assert_type(p.get_shirt_size_display(), str)
"#,
);

django_testcase!(
    test_charfield_choices_inline_tuple,
    r#"
from typing import assert_type, Literal

from django.db import models

class Card(models.Model):
    suit = models.CharField(
        max_length=100,
        choices=(
            ("CLUBS", "Clubs"),
            ("SPADES", "Spades"),
            ("HEARTS", "Hearts"),
            ("DIAMONDS", "Diamonds"),
        ),
    )

card = Card(suit="CLUBS")
assert_type(card.suit, Literal["CLUBS", "SPADES", "HEARTS", "DIAMONDS"])
"#,
);

// We can consider narrowing the type further
// but it's unclear if it's worth doing so since
// django stubs give type Any for the aggregate values
django_testcase!(
    test_int_field,
    r#"
from django.db import models
from typing import assert_type, Any

class Default(models.Model):
    int_field = models.IntegerField(default=0)

total_sum_typed = Default.objects.aggregate(
        total=models.Sum("int_field", output_field=models.IntegerField())
)
assert_type(total_sum_typed, dict[str, Any]) 
"#,
);

django_testcase!(
    test_file_field_is_field_file,
    r#"
from typing import assert_type

from django.db import models
from django.db.models.fields.files import FieldFile, ImageFieldFile

class Layout(models.Model):
    attachment = models.FileField(upload_to="file")
    avatar = models.ImageField(upload_to="file")

layout = Layout()
# django-stubs types `FileField.__get__` as `Any` because the attribute also accepts a bare path.
# Django hands back a `FieldFile`, which is what makes `.path` and `.width` resolve.
assert_type(layout.attachment, FieldFile)
assert_type(layout.avatar, ImageFieldFile)
assert_type(layout.attachment.path, str)
assert_type(layout.avatar.width, int)
"#,
);

django_testcase!(
    test_file_field_subclass_is_field_file,
    r#"
from typing import assert_type

from django.db import models
from django.db.models.fields.files import FieldFile, ImageFieldFile

class MyFileField(models.FileField): ...
class MyImageField(models.ImageField): ...

class Layout(models.Model):
    attachment = MyFileField()
    avatar = MyImageField()

layout = Layout()
assert_type(layout.attachment, FieldFile)
assert_type(layout.avatar, ImageFieldFile)
"#,
);

django_testcase!(
    test_nullable_file_field,
    r#"
from typing import assert_type

from django.db import models
from django.db.models.fields.files import FieldFile

class Layout(models.Model):
    attachment = models.FileField(null=True)

assert_type(Layout().attachment, FieldFile | None)
"#,
);
