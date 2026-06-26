# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

import numpy as np
from shape_extensions import assert_shape


def test_matrix_arithmetic() -> None:
    a = np.ones((3, 4))
    b = np.full((3, 4), 2.0)

    assert_shape(a + b, (3, 4))
    assert_shape(a * b, (3, 4))
    assert_shape(b**2, (3, 4))
