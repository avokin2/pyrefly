# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import Any

import shape_extensions
from shape_extensions import Dim

type _Shape = tuple[int, ...]
type _AnyShape = tuple[Any, ...]

@shape_extensions.shaped_array(shape="Shape")
class ndarray[Shape: _Shape = _AnyShape, DType = Any]:
    shape: Shape

def zeros[N](
    shape: Dim[N], dtype: Any = ..., order: str = ...
) -> ndarray[tuple[Dim[N]]]: ...
def ones[N](
    shape: Dim[N], dtype: Any = ..., order: str = ...
) -> ndarray[tuple[Dim[N]]]: ...
def full[N](
    shape: Dim[N], fill_value: Any, dtype: Any = ..., order: str = ...
) -> ndarray[tuple[Dim[N]]]: ...
def empty[N](
    shape: Dim[N], dtype: Any = ..., order: str = ...
) -> ndarray[tuple[Dim[N]]]: ...
