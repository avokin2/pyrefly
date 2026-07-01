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
