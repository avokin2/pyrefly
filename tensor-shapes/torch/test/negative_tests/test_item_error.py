# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""Test item() metashape validation actually works"""

from typing import TYPE_CHECKING

import torch

if TYPE_CHECKING:
    from torch import Tensor


def test_item_on_1d_tensor():
    """item() should fail on 1D tensor"""
    x: Tensor[10] = torch.randn(10)
    # This should error because item() requires 0D tensor
    # E: item() only works on 0-dimensional tensors, got 1D tensor
    x.item()


def test_item_on_2d_tensor():
    """item() should fail on 2D tensor"""
    x: Tensor[5, 7] = torch.randn(5, 7)
    # This should error
    # E: item() only works on 0-dimensional tensors, got 2D tensor
    x.item()
