# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import overload

from shape_extensions import Dim

from .. import ndarray

# MVP shape surface only; NumPy dtype promotion is intentionally not modeled.
@overload
def solve[N, DType](
    a: ndarray[tuple[Dim[N], Dim[N]], DType],
    b: ndarray[tuple[Dim[N]]],
) -> ndarray[tuple[Dim[N]], DType]: ...
@overload
def solve[N, K, DType](
    a: ndarray[tuple[Dim[N], Dim[N]], DType],
    b: ndarray[tuple[Dim[N], Dim[K]]],
) -> ndarray[tuple[Dim[N], Dim[K]], DType]: ...
