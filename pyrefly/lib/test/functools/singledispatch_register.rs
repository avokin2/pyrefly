/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! `functools.singledispatch` `.register` behavior: dispatch-type vs fallback subtype checks,
//! register-type vs impl-signature checks, explicit/functional registration, and calling a
//! registered implementation directly. pyrefly is stub-driven, so most register-time checks are
//! missed (false negatives); divergences are `bug=`-marked with the target as `# WANT: ...`.

use crate::functools_testcase;

functools_testcase!(
    bug = "singledispatch fallback shadowed by a registered impl with the same dispatch type as the fallback's first arg should be flagged as never-used; pyrefly is stub-driven and emits nothing",
    test_singledispatch_register_same_type_as_fallback_shadows,
    r#"
from functools import singledispatch

# WANT: singledispatch implementation 1 will never be used: implementation 2's dispatch type is the same
@singledispatch
def f(arg: int) -> None:
    pass

@f.register
def g(arg: int) -> None:
    pass
"#,
);

functools_testcase!(
    bug = "register(str) should error when the impl's first param is typed int, but pyrefly does not check dispatch-type vs signature",
    test_singledispatch_register_type_mismatches_signature,
    r#"
from functools import singledispatch

@singledispatch
def f(arg) -> None:
    pass

# WANT: Argument to register "str" is incompatible with type "int" in function signature
@f.register(str)
def g(arg: int) -> None:
    pass
"#,
);

functools_testcase!(
    test_singledispatch_register_explicit_type_not_subtype,
    r#"
from functools import singledispatch

@singledispatch
def f(arg: int) -> None:
    pass

@f.register(str)  # E: Dispatch type `str` is not a subtype of fallback first argument type `int`
def g(arg) -> None:
    pass
"#,
);

functools_testcase!(
    test_singledispatch_varargs_fallback_register_not_subtype,
    r#"
from functools import singledispatch

@singledispatch
def f(*args: int) -> None:
    pass

@f.register(str)  # E: Dispatch type `str` is not a subtype of fallback first argument type `int`
def g(arg) -> None:
    pass
"#,
);

// A `.register(C)` argument is a value, not a type form, so a name that collides with a special
// form (here `typing.Union`) must not be misread as that form and flagged `invalid-annotation`.
functools_testcase!(
    test_singledispatch_register_special_form_named_arg_no_error,
    r#"
from functools import singledispatch
from typing import Union
@singledispatch
def f(op: object) -> None: ...
@f.register(Union)
def _(op) -> None: ...
"#,
);

functools_testcase!(
    test_singledispatch_custom_class_passed_as_type_to_register,
    r#"
from functools import singledispatch
class A: pass

@singledispatch
def f(arg: int) -> None:
    pass
@f.register(A)  # E: Dispatch type `A` is not a subtype of fallback first argument type `int`
def g(arg) -> None:
    pass
"#,
);

functools_testcase!(
    test_singledispatch_dispatch_type_is_not_asubtype_of_fallback_first_argument,
    r#"
from functools import singledispatch

class A: pass
class B(A): pass
class C: pass

@singledispatch
def f(arg: A) -> None:
    pass

@f.register
def g(arg: B) -> None:
    pass

@f.register  # E: Dispatch type `C` is not a subtype of fallback first argument type `A`
def h(arg: C) -> None:
    pass
"#,
);

functools_testcase!(
    test_singledispatch_register_class_with_any_ctor_not_subtype,
    r#"
from functools import singledispatch
from typing import Any

class Base: pass
class ConstExpr:
    def __init__(self, **kwargs: Any) -> None: pass

@singledispatch
def f(arg: Base) -> ConstExpr:  # E: Function declared to return `ConstExpr` but is missing an explicit `return`
    pass

@f.register(ConstExpr)  # E: Dispatch type `ConstExpr` is not a subtype of fallback first argument type `Base`
def g(arg: ConstExpr) -> ConstExpr:  # E: Function declared to return `ConstExpr` but is missing an explicit `return`
    pass
"#,
);

functools_testcase!(
    bug = "pyrefly flags `x` as uninitialized; the bare `x: Missing` annotation is a declared local and the registered impl using `f` before all registrations are defined is fine (no error)",
    test_singledispatch_dispatch_call_from_registered_impl,
    r#"
from functools import singledispatch
from typing import Union

class Node: pass
class Module(Node): pass
class Missing: pass

@singledispatch
def f(a: Union[Node, Missing]) -> None:
    pass

@f.register
def g(a: Module) -> None:
    x: Missing
    # WANT: no error (`x: Missing` is a declared local; calling dispatch `f` from a registered impl before later registrations are defined is valid)
    f(x)  # E: `x` is uninitialized

@f.register
def h(a: Missing) -> None:
    pass
"#,
);

// Bare `@f.register` returns the impl's own type, so direct calls to the registered function are
// argument-checked.
functools_testcase!(
    test_singledispatch_registered_impl_bare_direct_call_arg_check,
    r#"
from functools import singledispatch

@singledispatch
def f(arg, arg2: str) -> bool:
    return False

@f.register
def g(arg: int, arg2: str) -> bool:
    return True

g('a', 'a')  # E: Argument `Literal['a']` is not assignable to parameter `arg` with type `int` in function `g`
g(1, 1)  # E: Argument `Literal[1]` is not assignable to parameter `arg2` with type `str` in function `g`
g(1, 'a')
"#,
);

functools_testcase!(
    bug = "explicit `@f.register(C)` does not return the impl's type, so direct calls to it are not argument-checked",
    test_singledispatch_registered_impl_explicit_direct_call_arg_check,
    r#"
from functools import singledispatch

@singledispatch
def f(arg, arg2: str) -> bool:
    return False

@f.register(str)
def h(arg, arg2: str) -> bool:
    return True

# don't show errors for the first argument (no annotation on the fallback's first param)
h(1, 'a')
# WANT: Argument 2 to "h" has incompatible type "int"; expected "str"
h('a', 1)
"#,
);

functools_testcase!(
    bug = "pyrefly does not emit the 'Need type annotation' error when singledispatch is given a non-callable argument",
    test_singledispatch_register_after_noncallable_arg,
    r#"
from typing import reveal_type, assert_type
import functools
# WANT: Need type annotation for "a"
a = functools.singledispatch('a')  # E: Argument `Literal['a']` is not assignable to parameter `func` with type `(...) -> @_` in function `functools.singledispatch`

@a.register(int)
def default(val) -> int:
    return 3
"#,
);

// Edge case
functools_testcase!(
    test_singledispatch_register_subtype_of_fallback,
    r#"
from functools import singledispatch
class A: pass
class B(A): pass
class C: pass
@singledispatch
def f(arg: A) -> None: ...
@f.register
def g(arg: B) -> None: ...
@f.register  # E: Dispatch type `C` is not a subtype of fallback first argument type `A`
def h(arg: C) -> None: ...
"#,
);

// Edge case
functools_testcase!(
    test_singledispatch_register_explicit_type_decorator,
    r#"
from typing import reveal_type
from functools import singledispatch
@singledispatch
def f(arg: object) -> str: return ""
@f.register(int)
def _(arg: int) -> str: return "int"
reveal_type(f(1))  # E: revealed type: str
"#,
);

// Edge case
functools_testcase!(
    test_singledispatch_register_functional_form,
    r#"
from typing import reveal_type, assert_type
from functools import singledispatch
def f_impl(arg: object) -> str: return ""
def int_impl(arg: int) -> str: return "int"
g = singledispatch(f_impl)
reveal_type(g)  # E: revealed type: _SingleDispatchCallable[str]
g.register(int, int_impl)
reveal_type(g(1))  # E: revealed type: str
"#,
);
