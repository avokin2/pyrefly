# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# pyre-ignore-all-errors

"""Library-agnostic shape typing primitives.

The .pyi stub provides full type information to pyrefly. This .py file
provides minimal runtime classes so that annotations using these types
don't crash when evaluated by Python.
"""

import typing
from dataclasses import dataclass

import torch
import torch.nn as nn

# Make torch types subscriptable at runtime so that annotations like
# Tensor[B, T, N] or nn.Linear[In, Out] evaluate as no-ops instead of
# crashing with "type is not subscriptable".
_subscriptable_classes = [
    torch.Tensor,
    nn.Embedding,
    nn.Linear,
    nn.ModuleList,
    # Convolution modules
    nn.Conv1d,
    nn.Conv2d,
    nn.Conv3d,
    nn.ConvTranspose1d,
    nn.ConvTranspose2d,
    nn.ConvTranspose3d,
    # Pooling modules
    nn.MaxPool1d,
    nn.MaxPool2d,
    nn.MaxPool3d,
    nn.AvgPool1d,
    nn.AvgPool2d,
    nn.AvgPool3d,
    nn.AdaptiveAvgPool1d,
    nn.AdaptiveAvgPool2d,
    nn.AdaptiveAvgPool3d,
    nn.AdaptiveMaxPool1d,
    nn.AdaptiveMaxPool2d,
    nn.AdaptiveMaxPool3d,
]
for _cls in _subscriptable_classes:
    if not hasattr(_cls, "__class_getitem__"):
        _cls.__class_getitem__ = classmethod(lambda cls, params: cls)


class Dim[T]:
    """Symbolic integer type for dimension values.

    At runtime this is a no-op generic class. The type checker uses the
    .pyi stub for shape inference.
    """

    pass


@dataclass(frozen=True)
class SymbolicArithExpr:
    """Runtime representation of symbolic dimension arithmetic."""

    op: str
    args: tuple[typing.Any, ...]

    def __str__(self):
        if self.op == "var":
            return str(self.args[0])
        if self.op == "-" and len(self.args) == 2 and self.args[0] == 0:
            return f"-{_format_symbolic_arg(self.args[1])}"
        if len(self.args) == 2:
            return (
                f"{_format_symbolic_arg(self.args[0])} "
                f"{self.op} {_format_symbolic_arg(self.args[1])}"
            )
        return repr(self)

    def __add__(self, other):
        return SymbolicArithExpr("+", (self, other))

    def __radd__(self, other):
        return SymbolicArithExpr("+", (other, self))

    def __sub__(self, other):
        return SymbolicArithExpr("-", (self, other))

    def __rsub__(self, other):
        return SymbolicArithExpr("-", (other, self))

    def __mul__(self, other):
        return SymbolicArithExpr("*", (self, other))

    def __rmul__(self, other):
        return SymbolicArithExpr("*", (other, self))

    def __floordiv__(self, other):
        return SymbolicArithExpr("//", (self, other))

    def __rfloordiv__(self, other):
        return SymbolicArithExpr("//", (other, self))

    def __pow__(self, other):
        return SymbolicArithExpr("**", (self, other))

    def __rpow__(self, other):
        return SymbolicArithExpr("**", (other, self))

    def __neg__(self):
        return SymbolicArithExpr("-", (0, self))


def _format_symbolic_arg(value):
    if (
        isinstance(value, SymbolicArithExpr)
        and value.op != "var"
        and not (value.op == "-" and len(value.args) == 2 and value.args[0] == 0)
    ):
        return f"({value})"
    return str(value)


class D:
    """Wrap a shape type variable so Python can evaluate dimension arithmetic."""

    def __new__(cls, value):
        return SymbolicArithExpr("var", (value,))

    def __class_getitem__(cls, value):
        return cls(value)


def shaped_array(*, shape: str) -> typing.Callable[[type], type]:
    """Decorator that marks a class as carrying a shape TypeVarTuple."""

    def decorator(cls: type) -> type:
        return cls

    return decorator


class TypeVar:
    """TypeVar with arithmetic support for tensor shape dimensions.

    Like typing.TypeVar but arithmetic operations (N + 1, N * 2, etc.)
    return self instead of raising TypeError. Setting
    __class__ = typing.TypeVar makes isinstance(x, typing.TypeVar)
    return True, so Generic[N] and TypedDict + Generic[N] both work.

    In pyrefly, shape_extensions.TypeVar is treated identically to
    typing.TypeVar.
    """

    __class__ = typing.TypeVar

    def __init__(self, name: str):
        self.__name__ = name
        self.name = name

    def __repr__(self):
        return self.name

    def __hash__(self):
        return hash(self.name)

    def __eq__(self, other):
        return self is other

    def __add__(self, other):
        return self

    def __radd__(self, other):
        return self

    def __sub__(self, other):
        return self

    def __rsub__(self, other):
        return self

    def __mul__(self, other):
        return self

    def __rmul__(self, other):
        return self

    def __floordiv__(self, other):
        return self

    def __typing_subst__(self, arg):
        return arg


class TypeVarTuple:
    """TypeVarTuple with support for integer shape dimensions.

    Like typing.TypeVarTuple but for use in tensor shape annotations.
    Setting __class__ = typing.TypeVarTuple and providing
    __typing_is_unpacked_typevartuple__ makes Generic[*Ns] work.

    In pyrefly, shape_extensions.TypeVarTuple is treated identically to
    typing.TypeVarTuple.

    __iter__ yields self so that *Ns unpacking works in subscripts
    like Generic[*Ns] or Tensor[*Ns, 3]. Python's star-unpacking
    calls __iter__ on the object.
    """

    __class__ = typing.TypeVarTuple

    def __init__(self, name: str):
        self.__name__ = name
        self.name = name

    def __repr__(self):
        return f"*{self.name}"

    def __hash__(self):
        return hash(self.name)

    def __eq__(self, other):
        return self is other

    def __iter__(self):
        yield self

    @property
    def __typing_is_unpacked_typevartuple__(self):
        return True


def uses_shape_dsl(
    ir_fn: typing.Callable,
    *,
    capture_init: list[str] | None = None,
) -> typing.Callable[[typing.Callable], typing.Callable]:
    """Decorator that associates a shape DSL function with an API function.

    At runtime this is a no-op: the decorator arguments are ignored and the
    decorated function is returned unchanged. Pyrefly uses this decorator
    at type-checking time to route bound arguments through the shape DSL
    for return-type refinement.
    """

    def decorator(fn: typing.Callable) -> typing.Callable:
        return fn

    return decorator
