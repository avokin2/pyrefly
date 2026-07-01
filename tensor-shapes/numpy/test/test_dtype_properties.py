# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from typing import Any, assert_type

import numpy as np
from shape_extensions import assert_shape


def test_zeros_default_dtype() -> None:
    x = np.zeros(5)

    assert_shape(x, (5,))
    assert_type(x.dtype, np.dtype[np.float64])
    assert x.dtype == np.dtype(np.float64)


def test_ones_default_dtype() -> None:
    x = np.ones((2, 3))

    assert_shape(x, (2, 3))
    assert_type(x.dtype, np.dtype[np.float64])
    assert x.dtype == np.dtype(np.float64)


def test_empty_default_dtype() -> None:
    x = np.empty((4,))

    assert_shape(x, (4,))
    assert_type(x.dtype, np.dtype[np.float64])
    assert x.dtype == np.dtype(np.float64)


def test_full_omitted_dtype_uses_fill_value_dtype() -> None:
    x = np.full((2, 3), 7)

    assert_shape(x, (2, 3))
    assert_type(x.dtype, Any)
    assert x.dtype == np.dtype(int)


def test_eye_default_dtype() -> None:
    x = np.eye(3)

    assert_shape(x, (3, 3))
    assert_type(x.dtype, np.dtype[np.float64])
    assert x.dtype == np.dtype(np.float64)


def test_identity_default_dtype() -> None:
    x = np.identity(4)

    assert_shape(x, (4, 4))
    assert_type(x.dtype, np.dtype[np.float64])
    assert x.dtype == np.dtype(np.float64)
