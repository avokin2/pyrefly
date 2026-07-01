# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from typing import Any, cast, Literal

import numpy as np
from shape_extensions import assert_shape


def make_array(shape: Any) -> Any:
    return np.ones(shape)


def test_reduce_matrix_no_axis() -> None:
    a = np.ones((3, 4))

    assert_shape(np.sum(a), ())
    assert_shape(np.mean(a), ())


def test_reduce_higher_rank_no_axis() -> None:
    a = cast(
        "np.ndarray[tuple[Literal[2], Literal[3], Literal[4]]]",
        make_array((2, 3, 4)),
    )

    assert_shape(np.sum(a), ())
    assert_shape(np.mean(a, keepdims=True), (1, 1, 1))
    assert_shape(np.max(a, keepdims=True), (1, 1, 1))


def test_reduce_matrix_axis_zero() -> None:
    a = np.ones((3, 4))

    assert_shape(np.sum(a, axis=0), (4,))
    assert_shape(np.mean(a, axis=0), (4,))
    assert_shape(np.min(a, axis=0), (4,))
    assert_shape(np.max(a, axis=0), (4,))


def test_mean_method_axis_zero_broadcasts_over_matrix() -> None:
    a = np.ones((3, 4))
    column_means = a.mean(axis=0)

    assert_shape(column_means, (4,))
    assert_shape(a - column_means, (3, 4))


def test_reduce_matrix_axis_one() -> None:
    a = np.ones((3, 4))

    assert_shape(np.sum(a, axis=1), (3,))
    assert_shape(np.mean(a, axis=1), (3,))
    assert_shape(np.min(a, axis=1), (3,))
    assert_shape(np.max(a, axis=1), (3,))


def test_reduce_higher_rank_axis() -> None:
    a = cast(
        "np.ndarray[tuple[Literal[2], Literal[3], Literal[4]]]",
        make_array((2, 3, 4)),
    )

    assert_shape(np.sum(a, axis=1), (2, 4))
    assert_shape(np.mean(a, axis=-1), (2, 3))


def test_reduce_rejects_invalid_axes() -> None:
    a = np.ones((3, 4))

    assert_shape(np.sum(a, axis=0), (4,))
    try:
        # E: axis out of bounds
        np.sum(a, axis=3)
    except ValueError:
        pass
    else:
        raise AssertionError("expected NumPy to reject out-of-bounds axis")

    try:
        # E: duplicate axis
        np.sum(a, axis=(0, 0))
    except ValueError:
        pass
    else:
        raise AssertionError("expected NumPy to reject duplicate axes")


def test_reduce_matrix_axis_zero_keepdims() -> None:
    a = np.ones((3, 4))

    assert_shape(np.sum(a, axis=0, keepdims=True), (1, 4))
    assert_shape(np.mean(a, axis=0, keepdims=True), (1, 4))
    assert_shape(np.min(a, axis=0, keepdims=True), (1, 4))
    assert_shape(np.max(a, axis=0, keepdims=True), (1, 4))


def test_reduce_matrix_axis_one_keepdims() -> None:
    a = np.ones((3, 4))

    assert_shape(np.sum(a, axis=1, keepdims=True), (3, 1))
    assert_shape(np.mean(a, axis=1, keepdims=True), (3, 1))
    assert_shape(np.min(a, axis=1, keepdims=True), (3, 1))
    assert_shape(np.max(a, axis=1, keepdims=True), (3, 1))


def test_reduce_higher_rank_keepdims() -> None:
    a = cast(
        "np.ndarray[tuple[Literal[2], Literal[3], Literal[4]]]",
        make_array((2, 3, 4)),
    )

    assert_shape(np.sum(a, axis=(1, 2), keepdims=True), (2, 1, 1))
    assert_shape(np.min(a, axis=0, keepdims=True), (1, 3, 4))


def test_keepdims_reduction_broadcasts_over_matrix() -> None:
    a = np.ones((3, 4))
    row_totals = np.sum(a, axis=1, keepdims=True)

    assert_shape(a / row_totals, (3, 4))
