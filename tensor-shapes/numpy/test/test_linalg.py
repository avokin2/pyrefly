# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

import numpy as np
from shape_extensions import assert_shape


def test_matmul_function_2d() -> None:
    a = np.ones((3, 4))
    b = np.ones((4, 5))

    assert_shape(np.matmul(a, b), (3, 5))


def test_matmul_operator_2d() -> None:
    a = np.ones((3, 4))
    b = np.ones((4, 5))

    assert_shape(a @ b, (3, 5))


def test_transpose_property_2d() -> None:
    x = np.ones((3, 4))
    y = np.ones((3, 1))

    assert_shape(x.T, (4, 3))
    assert_shape(x.T.T, (3, 4))
    assert_shape(x.T @ x, (4, 4))
    assert_shape(x.T @ y, (4, 1))


def test_solve_vector_rhs() -> None:
    a = np.eye(3)
    b = np.ones(3)

    assert_shape(np.linalg.solve(a, b), (3,))


def test_solve_matrix_rhs() -> None:
    a = np.eye(3)
    b = np.ones((3, 2))

    assert_shape(np.linalg.solve(a, b), (3, 2))


def test_solve_column_rhs_regression_composition() -> None:
    x = np.random.randn(5, 3)
    y = np.random.randn(5, 1)

    assert_shape(np.linalg.solve(x.T @ x, x.T @ y), (3, 1))


def test_matmul_operator_rejects_mismatched_inner_dimension() -> None:
    a = np.ones((3, 4))
    b = np.ones((6, 5))

    # The mismatched matmul below raises before assert_shape runs, so anchor the
    # well-formed shape here to satisfy run_runtime_tests' "every test asserts a
    # shape" invariant.
    assert_shape(np.ones((3, 4)) @ np.ones((4, 5)), (3, 5))
    try:
        # The bridge dunder reports the finite-overload shape, not the DSL mismatch.
        # E: assert_shape((3, 5), (3, 4)) failed
        assert_shape(a @ b, (3, 4))
    except ValueError:
        pass
    else:
        raise AssertionError("expected NumPy to reject mismatched inner dimensions")


def test_matmul_rejects_mismatched_inner_dimension() -> None:
    a = np.ones((3, 4))
    b = np.ones((6, 5))

    # The mismatched matmul below raises before assert_shape runs, so anchor the
    # well-formed shape here to satisfy run_runtime_tests' "every test asserts a
    # shape" invariant.
    assert_shape(np.matmul(np.ones((3, 4)), np.ones((4, 5))), (3, 5))
    try:
        # E: matmul inner dimensions must match
        np.matmul(a, b)
    except ValueError:
        pass
    else:
        raise AssertionError("expected NumPy to reject mismatched inner dimensions")
