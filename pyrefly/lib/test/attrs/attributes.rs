/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::attrs_testcase;

attrs_testcase!(
    bug = "Pyrefly does not recognize attrs' automatic underscore stripping behavior for private attributes",
    private_attrs,
    r#"
import attr

@attr.s(auto_attribs=True)
class Example:
    _private: str
    public: int

# This is the correct usage per attrs behavior:
obj = Example(private="secret", public=42) # E: Missing argument `_private` in function `Example.__init__` # E: Unexpected keyword argument `private` in function `Example.__init__`
"#,
);

// `@attr.s(auto_attribs=True)` opts into annotation-driven fields (classic
// `@attr.s` defaults to `auto_attribs=False`, see the tests below).
attrs_testcase!(
    test_attrs_basic,
    r#"
from typing import assert_type, reveal_type

import attr

@attr.s(auto_attribs=True)
class A:
    x: int
    y: int | None = None

reveal_type(A.__init__)  # E: revealed type: (self: A, x: int, y: int | None = ...) -> None

a = A(1)
assert_type(a.x, int)
assert_type(a.y, int | None)
"#,
);

// `@attr.s()` defaults to `auto_attribs=False`: bare annotations are NOT attrs
// fields, so `__init__` takes no parameters and `A(1)` is an error. The
// annotations still describe ordinary attributes.
attrs_testcase!(
    test_attrs_classic_s_default_ignores_bare_annotations,
    r#"
from typing import assert_type, reveal_type
import attr

@attr.s()
class A:
    x: int
    y: int | None = None

reveal_type(A.__init__)  # E: revealed type: (self: A) -> None
A(1)  # E: Expected 0 positional arguments
assert_type(A().x, int)
assert_type(A().y, int | None)
"#,
);

// Explicit `auto_attribs=False` matches the default: only `attr.ib()`-assigned names
// become fields; the bare `y` is not a field.
attrs_testcase!(
    test_attrs_classic_s_explicit_no_auto_attribs,
    r#"
from typing import reveal_type
import attr

@attr.s(auto_attribs=False)
class A:
    x: int = attr.ib()
    y: int

reveal_type(A.__init__)  # E: revealed type: (self: A, x: int) -> None
"#,
);

// `attr.attr` is an alias of `attr.attrib` and must be a recognized field specifier.
// Checked via construction (not the inferred param type) to stay robust to the separate
// converter-inference fix that changes these params from `Unknown` to their annotation.
attrs_testcase!(
    test_attrs_attr_alias_field_specifier,
    r#"
import attr

@attr.s
class A:
    x: int = attr.ib()
    y: int = attr.attr()

A(x=1, y=2)  # `y` (attr.attr) is a field, so it is an init parameter
"#,
);

// An inherited `attr.attr()` field becomes an init parameter on the subclass.
attrs_testcase!(
    test_attrs_attr_alias_field_specifier_inherited,
    r#"
import attr

@attr.s
class Base:
    a: int = attr.attr()

@attr.s
class Sub(Base):
    b: int = attr.ib()

Sub(a=1, b=2)  # inherited `a` (attr.attr) is an init parameter
"#,
);

// `@attr.dataclass` aliases the classic `attrs` stub function (`dataclass = attrs`)
// but is `partial(attrs, auto_attribs=True)`, so unlike `@attr.s` it IS
// annotation-driven: bare annotations are fields. We distinguish it from `@attr.s`
// by the decorator's syntactic name.
attrs_testcase!(
    test_attr_dataclass_is_auto_attribs_true,
    r#"
from typing import assert_type, reveal_type
import attr

@attr.dataclass
class C:
    x: int
    y: int | None = None

reveal_type(C.__init__)  # E: revealed type: (self: C, x: int, y: int | None = ...) -> None
c = C(1)
assert_type(c.x, int)
assert_type(c.y, int | None)
"#,
);

attrs_testcase!(
    test_attrs_attrib_pass,
    r#"
from typing import assert_type, reveal_type

import attr

@attr.s()
class A:
    x: int = attr.ib()
    y: int | None = attr.ib(None)

a = A(1)
assert_type(a.x, int)
assert_type(a.y, int | None)
"#,
);

attrs_testcase!(
    test_attrs_attrib_init_signature,
    r#"
from typing import assert_type, reveal_type

import attr

@attr.s()
class A:
    x: int = attr.ib()
    y: int | None = attr.ib(None)

reveal_type(A.__init__)  # E: revealed type: (self: A, x: int, y: int | None = ...) -> None
"#,
);

// Classic `@attr.s` (`auto_attribs=False`): an unannotated `attr.ib()` is a field, so it
// becomes an `__init__` parameter without requiring a type annotation. The type is `Any`
// when nothing constrains it, or inferred from `default`.
attrs_testcase!(
    test_attrs_unannotated_attrib_ok,
    r#"
from typing import reveal_type
import attr
@attr.s()
class A:
    x = attr.ib()  # !E: type annotation
    y = attr.ib(default=0)  # !E: type annotation

reveal_type(A.__init__)  # E: revealed type: (self: A, x: Any, y: int = ...) -> None
A()  # E: Missing argument `x`
"#,
);

attrs_testcase!(
    bug = "auto_attribs=True requires annotations, but we suppress the field-annotation error for all attrs classes",
    test_attrs_unannotated_attrib_auto_attribs,
    r#"
import attr
@attr.s(auto_attribs=True)
class A:
    x = attr.ib()  # !E: type annotation
"#,
);

// `attr.ib(type=T)` supplies the field type when there is no annotation, so the field
// becomes a typed `__init__` parameter (annotation wins if both are given).
attrs_testcase!(
    test_attrs_attrib_type_keyword,
    r#"
from typing import reveal_type
import attr

@attr.s
class C:
    x = attr.ib(type=int)

reveal_type(C.__init__)  # E: revealed type: (self: C, x: int) -> None
C("hello")  # E: not assignable to parameter `x`
"#,
);

// Next-gen `field()` is the opposite of `attr.ib`: attrs documents that type checkers ignore
// its `type=` metadata, so the field stays untyped (`Any`) rather than `int`.
attrs_testcase!(
    test_attrs_field_type_keyword_ignored,
    r#"
from typing import reveal_type
import attr, attrs

@attr.s
class C:
    x = attrs.field(type=int)

reveal_type(C.__init__)  # E: revealed type: (self: C, x: Any) -> None
C("anything")  # `x` is untyped, so any argument is accepted
"#,
);

// A `type=`-typed field declared in a base class is typed in the subclass `__init__`.
attrs_testcase!(
    test_attrs_attrib_type_keyword_inherited,
    r#"
from typing import reveal_type
import attr

@attr.s
class Base:
    a = attr.ib(type=int)

@attr.s
class Sub(Base):
    b = attr.ib(type=str)

reveal_type(Sub.__init__)  # E: revealed type: (self: Sub, a: int, b: str) -> None
"#,
);

// An unannotated `attr.ib()` inherited from a classic-attrs base is a parameter of the
// subclass `__init__`, so the field's metadata is resolved against its defining class.
attrs_testcase!(
    test_attrs_unannotated_attrib_inherited,
    r#"
import attr

@attr.s
class Base:
    a = attr.ib()

@attr.s
class Sub(Base):
    b = attr.ib(default=0)

Sub(1)
Sub()  # E: Missing argument `a`
"#,
);

// attrs raises `ValueError` at runtime if a field has both a type annotation and a
// `type=` argument. Matching types are used so the only error is the conflict itself.
attrs_testcase!(
    test_attrs_attrib_type_and_annotation_conflict,
    r#"
import attr

@attr.s
class C:
    x: int = attr.ib(type=int)  # E: both a type annotation and a `type` argument
"#,
);

// The conflict applies to the modern API too: next-gen `field()` under `@define`.
attrs_testcase!(
    test_attrs_field_type_and_annotation_conflict_define,
    r#"
from attrs import define, field

@define
class C:
    x: int = field(type=int)  # E: both a type annotation and a `type` argument
"#,
);

// The conflict is reported regardless of whether the annotation and `type=` agree. The
// extra assignment error comes from the general annotated-assignment check, not attrs.
attrs_testcase!(
    test_attrs_attrib_type_and_annotation_conflict_mismatch,
    r#"
import attr

@attr.s
class C:
    x: int = attr.ib(type=str)  # E: both a type annotation and a `type` argument # E: `str` is not assignable to `int`
"#,
);

// A `type=` keyword on a non-specifier call is not a field declaration, so no conflict error
// fires even with an annotation (attrs only inspects `attr.ib`/`field` results).
attrs_testcase!(
    test_attrs_type_keyword_on_non_specifier_call,
    r#"
import attr

def helper(type: int) -> int:
    return 0

@attr.s(auto_attribs=True)
class C:
    x: int = helper(type=5)
"#,
);

// Scoping: `type=` handling is attrs-only. A stdlib `@dataclass` is unaffected
// (`dataclasses.field` has no `type=`), so `x` does not become a typed field.
attrs_testcase!(
    test_attrs_type_keyword_dataclass_unaffected,
    r#"
from dataclasses import dataclass, field
from typing import reveal_type

@dataclass
class C:
    x = field(type=int)  # E: has no type annotation # E: No matching overload

reveal_type(C.__init__)  # E: revealed type: (self: C) -> None
"#,
);
