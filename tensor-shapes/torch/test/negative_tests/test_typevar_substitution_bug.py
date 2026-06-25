# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""More detailed test of type variable substitution"""

from typing import reveal_type, TYPE_CHECKING

import torch

if TYPE_CHECKING:
    from torch import Tensor


def accepts_symbolic_returns_symbolic[N](x: Tensor[N, 3]) -> Tensor[N, 3]:
    """Identity function with symbolic dimension - preserves shape"""
    return x


def test_detailed():
    """Check types at each step"""
    # Step 1: Create concrete tensor
    x_concrete: Tensor[2, 3] = torch.randn(2, 3)
    reveal_type(x_concrete)  # E: revealed type: Tensor[2, 3]

    # Step 2: Call function
    result = accepts_symbolic_returns_symbolic(x_concrete)
    # E: revealed type: Tensor[2, 3]
    reveal_type(result)

    # Step 3: These should both work because N is unbound
    case1: Tensor[2, 3] = result  # OK (N=2)
    # E: `Tensor[2, 3]` is not assignable to `Tensor[4, 3]`
    case2: Tensor[4, 3] = result  # Should error but doesn't (N=4)
    # E: `Tensor[2, 3]` is not assignable to `Tensor[100, 3]`
    case3: Tensor[100, 3] = result  # Should error but probably doesn't (N=100)
    _ = (case1, case2, case3)
