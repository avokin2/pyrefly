# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

import numpy as np
from shape_extensions import assert_shape, Dim, TypeVar

N = TypeVar("N")
P = TypeVar("P")


def ordinary_least_squares(
    x: np.ndarray[tuple[Dim[N], Dim[P]]],
    y: np.ndarray[tuple[Dim[N], Dim[1]]],
) -> np.ndarray[tuple[Dim[P], Dim[1]]]:
    return np.linalg.solve(x.T @ x, x.T @ y)


def test_ordinary_least_squares() -> None:
    x = np.eye(3)
    y = np.ones((3, 1))

    assert_shape(x, (3, 3))
    assert_shape(y, (3, 1))
    assert_shape(ordinary_least_squares(x, y), (3, 1))
