# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from typing import assert_type

import numpy as np
from shape_extensions import assert_shape


def test_arange_from_array_length() -> None:
    targets = np.zeros(5, dtype=np.intp)
    indices = np.arange(len(targets))

    assert_shape(indices, (5,))
    assert_type(indices.dtype, np.dtype[np.intp])
    assert indices.dtype == np.dtype(np.intp)


def test_paired_row_column_indexing() -> None:
    logits = np.ones((5, 3))
    targets = np.zeros(5, dtype=np.intp)
    selected = logits[np.arange(len(targets)), targets]

    assert_shape(selected, (5,))
    assert_type(selected.dtype, np.dtype[np.float64])


def test_paired_row_column_indexing_accepts_integer_dtypes() -> None:
    logits = np.ones((5, 3))
    int64_targets = np.zeros(5, dtype=np.int64)
    int32_targets = np.zeros(5, dtype=np.int32)

    assert_shape(logits[np.arange(len(int64_targets)), int64_targets], (5,))
    assert_shape(logits[np.arange(len(int32_targets)), int32_targets], (5,))


def test_paired_row_column_indexing_uses_index_shape() -> None:
    logits = np.ones((5, 3))
    rows = np.arange(2)
    columns = np.zeros(2, dtype=np.int64)
    selected = logits[rows, columns]

    assert_shape(selected, (2,))
