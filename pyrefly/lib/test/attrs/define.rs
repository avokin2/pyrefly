/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::attrs_testcase;

// DECORATOR DETECTION
//
// Coverage of every class-level attrs decorator entry point. All are marked with PEP 681
// `@dataclass_transform(...)` in attrs' stubs, so pyrefly routes them through its generic
// dataclass synthesis path with only a thin attrs-specific layer.
//
// Handled correctly by dataclass_transform:
//   - `@define` / `@attrs.define` / `@attr.define`           (modern, mutable)
//   - `@mutable` / `@attrs.mutable` / `@attr.mutable`        (alias of define)
//   - `@frozen` / `@attrs.frozen` / `@attr.frozen`           (frozen_default=True)
//   - `@attr.s` / `@attr.s()` / `@attr.attrs` / `@attr.attributes` (auto_attribs=False:
//     only `attr.ib()`/`field()` assignments are fields, bare annotations ignored)
//   - `@attr.s(auto_attribs=True)` / `@attr.dataclass`       (auto_attribs=True)
//   - per-decorator default keywords: order_default (classic), frozen_default,
//     no-order (define)
//
// NOT handled (bug-marked tests below):
//   - `field()` / `attr.ib()` specifier params -> `Unknown` (converter-param leak)

// `@define`: fields come from annotations (auto_attribs is implicit).
attrs_testcase!(
    test_attrs_define_basic,
    r#"
from typing import assert_type, reveal_type
from attrs import define

@define
class C:
    x: int
    y: int | None = None

reveal_type(C.__init__)  # E: revealed type: (self: C, x: int, y: int | None = ...) -> None

c = C(1)
assert_type(c.x, int)
assert_type(c.y, int | None)
"#,
);

// `@attrs.define` namespace form.
attrs_testcase!(
    test_attrs_namespace_define_basic,
    r#"
from typing import assert_type, reveal_type
import attrs

@attrs.define
class C:
    x: int
    y: int | None = None

reveal_type(C.__init__)  # E: revealed type: (self: C, x: int, y: int | None = ...) -> None

c = C(1)
assert_type(c.x, int)
assert_type(c.y, int | None)
"#,
);

// `@attr.define` namespace form.
attrs_testcase!(
    test_attr_namespace_define_basic,
    r#"
from typing import assert_type, reveal_type
import attr

@attr.define
class C:
    x: int
    y: int | None = None

reveal_type(C.__init__)  # E: revealed type: (self: C, x: int, y: int | None = ...) -> None

c = C(1)
assert_type(c.x, int)
assert_type(c.y, int | None)
"#,
);

// Classic `@attr.s(auto_attribs=True)`.
attrs_testcase!(
    test_attrs_classic_s_basic,
    r#"
from typing import assert_type, reveal_type
import attr

@attr.s(auto_attribs=True)
class C:
    x: int
    y: int | None = None

reveal_type(C.__init__)  # E: revealed type: (self: C, x: int, y: int | None = ...) -> None

c = C(1)
assert_type(c.x, int)
assert_type(c.y, int | None)
"#,
);

// `c.x` is correctly `int`, but the synthesized `__init__` params come out `Unknown`.
// Cause: attrs' `field()` has a `converter` parameter, so `as_param` uses the (empty)
// converter's input type instead of the field's declared type. Field specifiers without
// a `converter` (plain dataclasses, custom transforms) keep the annotation. Same root
// cause as `attributes::test_attrs_attrib_fail`. When fixed, drop the `bug` marker and
// flip the expectation to `(self: C, x: int, y: int = ...)`.
attrs_testcase!(
    bug = "field() annotation not propagated into synthesized __init__; params are Unknown, it should be (self: C, x: int, y: int = ...)",
    test_attrs_define_field_init_signature,
    r#"
from typing import reveal_type
from attrs import define, field

@define
class C:
    x: int = field()
    y: int = field(default=0)

reveal_type(C.__init__)  # E: revealed type: (self: C, x: Unknown, y: Unknown = ...) -> None
"#,
);

// `@frozen` (frozen_default=True): attributes are read-only.
attrs_testcase!(
    test_attrs_frozen_basic,
    r#"
from typing import assert_type, reveal_type
from attrs import frozen

@frozen
class C:
    x: int

reveal_type(C.__init__)  # E: revealed type: (self: C, x: int) -> None
c = C(1)
assert_type(c.x, int)
c.x = 2  # E: Cannot set field `x`
"#,
);

// `@attr.frozen` / `@attrs.frozen` namespace forms.
attrs_testcase!(
    test_attr_namespace_frozen_basic,
    r#"
import attr
import attrs

@attr.frozen
class A:
    x: int

@attrs.frozen
class B:
    x: int

A(1).x = 2  # E: Cannot set field `x`
B(1).x = 2  # E: Cannot set field `x`
"#,
);

// `@mutable` is an alias of `@define` (attributes writable). Covers all namespace forms.
attrs_testcase!(
    test_attrs_mutable_basic,
    r#"
from typing import assert_type, reveal_type
from attrs import mutable
import attr
import attrs

@mutable
class C:
    x: int
    y: int | None = None

reveal_type(C.__init__)  # E: revealed type: (self: C, x: int, y: int | None = ...) -> None
c = C(1)
assert_type(c.x, int)
c.x = 2  # OK: mutable

@attr.mutable
class A:
    x: int

@attrs.mutable
class B:
    x: int

assert_type(A(1).x, int)
assert_type(B(1).x, int)
"#,
);

// `@attr.attrs` / `@attr.attributes` are aliases of `@attr.s`.
attrs_testcase!(
    test_attr_classic_aliases,
    r#"
from typing import assert_type
import attr

@attr.attrs(auto_attribs=True)
class A:
    x: int

@attr.attributes(auto_attribs=True)
class B:
    x: int

assert_type(A(1).x, int)
assert_type(B(1).x, int)
"#,
);

// Same `converter`-parameter bug as `field()`: an `attr.ib()` field's `__init__` param
// is `Unknown`, though the attribute type is read correctly.
attrs_testcase!(
    bug = "classic attr.ib() fields get Unknown __init__ params; should be (self: C, x: int)",
    test_attrs_classic_s_no_auto_attribs,
    r#"
from typing import reveal_type
import attr

@attr.s
class C:
    x: int = attr.ib()

reveal_type(C.__init__)  # E: revealed type: (self: C, x: Unknown) -> None
"#,
);

attrs_testcase!(
    test_attrs_classic_s_no_auto_attribs_ignores_annotations,
    r#"
from typing import reveal_type
import attr

@attr.s()
class C:
    x: int
    y: int

reveal_type(C.__init__)  # E: revealed type: (self: C) -> None
"#,
);

// Classic `@attr.s` sets `order_default=True`, so ordering methods (`__lt__`, etc.) are
// synthesized.
attrs_testcase!(
    test_attrs_classic_order,
    r#"
import attr

@attr.s(auto_attribs=True)
class C:
    x: int

C(1) < C(2)  # OK: order_default=True synthesizes __lt__
"#,
);

// `@define` does NOT set `order_default`, so ordering methods are not synthesized.
attrs_testcase!(
    test_attrs_define_no_order,
    r#"
from attrs import define

@define
class C:
    x: int

C(1) < C(2)  # E: `<` is not supported
"#,
);

// DECORATOR KEYWORDS
//
// Decorator-level keywords (init / frozen / slots / kw_only / match_args / order /
// unsafe_hash) and frozen-subclass propagation. These map onto the standard dataclass
// keywords, so pyrefly handles them through the generic dataclass_transform path.

// `@attr.dataclass`, a fourth class-level entry point (stub-aliased to classic `attrs`).
attrs_testcase!(
    test_attr_dataclass_basic,
    r#"
from typing import assert_type
import attr

@attr.dataclass
class C:
    x: int

assert_type(C(1).x, int)
"#,
);

// `init=False` suppresses `__init__` synthesis.
attrs_testcase!(
    test_attrs_define_init_false,
    r#"
from attrs import define

@define(init=False)
class C:
    x: int

C()    # OK: no synthesized __init__, falls back to object.__init__
C(1)   # E: Expected 0 positional arguments
"#,
);

// Explicit `frozen=True` keyword on `@define`.
attrs_testcase!(
    test_attrs_define_frozen_kwarg,
    r#"
from attrs import define

@define(frozen=True)
class C:
    x: int

C(1).x = 2  # E: Cannot set field `x`
"#,
);

// `kw_only=True` at the class level makes all fields keyword-only in `__init__`.
attrs_testcase!(
    test_attrs_define_kw_only,
    r#"
from attrs import define

@define(kw_only=True)
class C:
    x: int

C(x=1)  # OK
C(1)    # E: Expected argument `x` to be passed by name
"#,
);

// `order=True` keyword on `@define` enables ordering methods.
attrs_testcase!(
    test_attrs_define_order_kwarg,
    r#"
from attrs import define

@define(order=True)
class C:
    x: int

C(1) < C(2)  # OK
"#,
);

// `match_args` controls `__match_args__` synthesis (default True for `@define`).
attrs_testcase!(
    test_attrs_define_match_args,
    r#"
from typing import reveal_type
from attrs import define

@define
class C:
    x: int
    y: int

reveal_type(C.__match_args__)  # E: revealed type: tuple[Literal['x'], Literal['y']]
"#,
);

// `slots=True` keyword is accepted; field synthesis still works.
attrs_testcase!(
    test_attrs_define_slots,
    r#"
from typing import assert_type
from attrs import define

@define(slots=True)
class C:
    x: int

assert_type(C(1).x, int)
"#,
);

// `unsafe_hash=True` keeps the class hashable.
attrs_testcase!(
    test_attrs_define_unsafe_hash,
    r#"
from attrs import define

@define(unsafe_hash=True)
class C:
    x: int

hash(C(1))  # OK
"#,
);

// Frozen propagates to subclasses: a plain subclass of a `@frozen` class inherits the
// synthesized frozen `__setattr__`, so attribute assignment is still rejected.
attrs_testcase!(
    test_attrs_frozen_subclass,
    r#"
from attrs import frozen

@frozen
class Base:
    x: int

class Sub(Base):
    pass

Sub(1).x = 2  # E: Cannot set field `x`
"#,
);
