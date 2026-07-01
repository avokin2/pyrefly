# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

import sys
import unittest
from pathlib import Path


class NumpyRuntimeTest(unittest.TestCase):
    def test_runtime_suites(self) -> None:
        numpy_root = Path(__file__).resolve().parent.parent.parent
        sys.path.insert(0, str(numpy_root))
        try:
            import run_runtime_tests

            self.assertIn("examples", run_runtime_tests.SUITES)
            total_tests = run_runtime_tests.run_suites(numpy_root, ["all"])
        finally:
            sys.path.pop(0)

        self.assertGreater(total_tests, 0)
