# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from shape_extensions.dsl import Error, shape_dsl_function, ShapedArray, symint, Unknown

@shape_dsl_function
def int_max(a: int, b: int) -> int:
    if a > b:
        return a
    return b

@shape_dsl_function
def broadcast_dim(
    a: int | symint,
    b: int | symint,
) -> int | symint:
    if a == 1:
        return b
    if b == 1:
        return a
    if a == b:
        return a
    if isinstance(a, int) and isinstance(b, int):
        raise Error("operands could not be broadcast together")
    return Unknown

@shape_dsl_function
def broadcast_shape(a: list[int | symint], b: list[int | symint]) -> list[int | symint]:
    max_len = int_max(len(a), len(b))
    padded_a = [1 for _ in range(max_len - len(a))] + a
    padded_b = [1 for _ in range(max_len - len(b))] + b
    return [broadcast_dim(ad, bd) for ad, bd in zip(padded_a, padded_b)]

@shape_dsl_function
def binary_ufunc_ir(x1: ShapedArray, x2: ShapedArray) -> ShapedArray:
    return ShapedArray(shape=broadcast_shape(x1.shape, x2.shape))

@shape_dsl_function
def normalize_axis(rank: int, axis: int) -> int:
    if axis < 0:
        return axis + rank
    return axis

@shape_dsl_function
def count_axis(axes: list[int], axis: int) -> int:
    return len([candidate for candidate in axes if candidate == axis])

@shape_dsl_function
def reduce_shape(
    shape: list[int | symint],
    axis: int | list[int] | None,
    keepdims: bool,
) -> list[int | symint]:
    if axis == None:
        if keepdims:
            return [1 for _ in range(len(shape))]
        return []
    axes = axis if isinstance(axis, list) else [axis]
    normalized = [normalize_axis(len(shape), axis) for axis in axes]
    out_of_bounds = [axis for axis in normalized if axis < 0 or axis > len(shape) - 1]
    if len(out_of_bounds) > 0:
        raise Error("axis out of bounds")
    duplicate_axes = [axis for axis in normalized if count_axis(normalized, axis) > 1]
    if len(duplicate_axes) > 0:
        raise Error("duplicate axis")
    return [
        1 if i in normalized else dim
        for i, dim in enumerate(shape)
        if keepdims or not (i in normalized)
    ]

@shape_dsl_function
def reduce_ir(
    a: ShapedArray,
    axis: int | list[int] | None = None,
    keepdims: bool = False,
) -> ShapedArray:
    return ShapedArray(shape=reduce_shape(a.shape, axis, keepdims))
