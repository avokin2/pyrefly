/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::attrs_testcase;

attrs_testcase!(
    bug = "Correctly recognize field and default decorator",
    field_default_decorator,
    r#"
from attrs import define, field

@define
class C:
    a: dict = field()

    @a.default # E: Object of class `dict` has no attribute `default`
    def _default_a(self):
        return {}

c = C() # E: Missing argument `a` in function `C.__init__`
"#,
);

attrs_testcase!(
    bug = "Recognize validator decorator",
    field_validator_decorator,
    r#"
from attrs import define, field

@define
class C:
    x: int = field()

    @x.validator # E: Object of class `int` has no attribute `validator`
    def _check_x(self, attribute, value):
        if value < 0:
            raise ValueError("x must be non-negative")
"#,
);

// A field's declared type flows to its `__init__` param, so construction args are
// type-checked: passing a `str` for an `int` field is an error.
attrs_testcase!(
    test_attrs_field_no_converter_construct_typecheck,
    r#"
from attrs import define, field

@define
class C:
    x: int = field()

C("nope")  # E: not assignable to parameter `x`
"#,
);

// With a `converter=`, the `__init__` param takes the converter's input type (`str`),
// while the stored attribute keeps the declared/output type (`int`).
attrs_testcase!(
    test_attrs_field_with_converter_still_uses_converter_input,
    r#"
from typing import assert_type, reveal_type
from attrs import define, field

def to_int(s: str) -> int:
    return int(s)

@define
class C:
    x: int = field(converter=to_int)

reveal_type(C.__init__)  # E: revealed type: (self: C, x: str) -> None
c = C("5")
assert_type(c.x, int)
"#,
);

// A `factory=` field is optional in `__init__`, but its param keeps the declared
// annotation type so construction args are still type-checked.
attrs_testcase!(
    test_attrs_field_factory_param_type,
    r#"
from attrs import define, field

@define
class C:
    items: list[int] = field(factory=list)

C()              # OK: factory supplies the default
C("not a list")  # E: not assignable to parameter `items`
"#,
);

// attrs raises `ValueError` at runtime when both `default=` and `factory=` are given.
attrs_testcase!(
    test_attrs_field_default_and_factory_conflict,
    r#"
from attrs import define, field

@define
class C:
    x: int = field(default=1, factory=int)  # E: cannot specify both `default` and `factory`
"#,
);

// The same conflict applies to the classic `attr.ib()` specifier.
attrs_testcase!(
    test_attrs_attr_ib_default_and_factory_conflict,
    r#"
import attr

@attr.s(auto_attribs=True)
class C:
    x: int = attr.ib(default=1, factory=int)  # E: cannot specify both `default` and `factory`
"#,
);

// `attr.ib` accepts `default` positionally, so a positional default plus `factory=`
// is also a conflict (`field()` is keyword-only, so this can only happen via `attr.ib`).
attrs_testcase!(
    test_attrs_attr_ib_positional_default_and_factory_conflict,
    r#"
import attr

@attr.s(auto_attribs=True)
class C:
    x: int = attr.ib(1, factory=int)  # E: cannot specify both `default` and `factory`
"#,
);

// `default=Factory(...)` is the canonical desugaring of `factory=`: only `default` is
// passed, so it must NOT be reported as a conflict.
attrs_testcase!(
    test_attrs_field_default_factory_value_ok,
    r#"
from attrs import define, field, Factory

@define
class C:
    items: list[int] = field(default=Factory(list))
"#,
);

// A non-specifier call that merely happens to use `default`/`factory` keyword names is
// not a field specifier, so no conflict is reported.
attrs_testcase!(
    test_attrs_non_field_specifier_default_factory_ok,
    r#"
from attrs import define

def helper(default: int, factory: int) -> int:
    return default

@define
class C:
    x: int = helper(default=1, factory=2)
"#,
);

// Inherited `field()` fields keep their declared param type in the subclass
// `__init__`, collected base-first in MRO order.
attrs_testcase!(
    test_attrs_field_inherited_param_type,
    r#"
from typing import reveal_type
from attrs import define, field

@define
class Base:
    x: int = field()

@define
class Sub(Base):
    y: str = field(default="a")

reveal_type(Sub.__init__)  # E: revealed type: (self: Sub, x: int, y: str = ...) -> None
"#,
);
