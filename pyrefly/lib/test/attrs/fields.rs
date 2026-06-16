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
