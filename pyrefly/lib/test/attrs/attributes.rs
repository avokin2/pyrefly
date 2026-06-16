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

// Explicit `auto_attribs=False` matches the default, and only `attr.ib()`-assigned
// names become fields. (The `x` param type is `Unknown` due to a separate,
// pre-existing `attr.ib()` converter-inference bug; the point here is that `x` is a
// field and the bare `y` is not.)
attrs_testcase!(
    test_attrs_classic_s_explicit_no_auto_attribs,
    r#"
from typing import reveal_type
import attr

@attr.s(auto_attribs=False)
class A:
    x: int = attr.ib()
    y: int

reveal_type(A.__init__)  # E: revealed type: (self: A, x: Unknown) -> None
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
    bug = "attr.ib inferred as Unknown in __init__ function",
    test_attrs_attrib_fail,
    r#"
from typing import assert_type, reveal_type

import attr

@attr.s()
class A:
    x: int = attr.ib()
    y: int | None = attr.ib(None)

reveal_type(A.__init__)  # E: revealed type: (self: A, x: Unknown, y: Unknown = ...) -> None
"#,
);

attrs_testcase!(
    test_attrs_unannotated_attrib_ok,
    r#"
import attr
@attr.s()
class A:
    x = attr.ib()  # !E: type annotation
    y = attr.ib(default=0)  # !E: type annotation
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
