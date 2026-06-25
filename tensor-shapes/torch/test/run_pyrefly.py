# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path


SUITES = (
    "torch-positive",
    "torch-negative",
    "jaxtyping-positive",
    "jaxtyping-negative",
)


def default_pyrefly(repo_root: Path, release: bool) -> Path:
    target_dir = (
        Path(os.environ["CARGO_TARGET_DIR"])
        if "CARGO_TARGET_DIR" in os.environ
        else repo_root / "target"
    )
    executable = "pyrefly.exe" if os.name == "nt" else "pyrefly"
    return target_dir / ("release" if release else "debug") / executable


def resolve_pyrefly(pyrefly: Path) -> Path:
    """Resolve the pyrefly executable, tolerating a missing `.exe` suffix so
    callers (e.g. CI) can pass an OS-agnostic path."""
    if not pyrefly.exists():
        with_exe = pyrefly.with_name(pyrefly.name + ".exe")
        if with_exe.exists():
            return with_exe.resolve()
    return pyrefly.resolve()


def files(pattern: str, root: Path) -> list[str]:
    return [str(path.relative_to(root)) for path in sorted(root.glob(pattern))]


def run_check(
    *,
    pyrefly: Path,
    test_root: Path,
    tensor_shapes_root: Path,
    suite: str,
    nocapture: bool,
) -> int:
    jaxtyping_root = test_root / "jaxtyping"
    if suite == "torch-positive":
        python_version = "3.13"
        search_paths = [tensor_shapes_root]
        check_files = files("test_*.py", test_root)
        expectations = False
    elif suite == "torch-negative":
        python_version = "3.13"
        search_paths = [tensor_shapes_root]
        check_files = files("negative_tests/test_*.py", test_root)
        expectations = True
    elif suite == "jaxtyping-positive":
        python_version = "3.12"
        search_paths = [jaxtyping_root / "fixtures", tensor_shapes_root]
        check_files = files("jaxtyping/test_*.py", test_root)
        expectations = False
    elif suite == "jaxtyping-negative":
        python_version = "3.12"
        search_paths = [jaxtyping_root / "fixtures", tensor_shapes_root]
        check_files = files("jaxtyping/negative_tests/test_*.py", test_root)
        expectations = True
    else:
        raise ValueError(f"unknown suite: {suite}")

    if not check_files:
        raise ValueError(f"no files found for suite: {suite}")

    command = [
        str(pyrefly),
        "check",
        "--config",
        os.devnull,
        "--python-version",
        python_version,
    ]
    for search_path in search_paths:
        command.extend(["--search-path", str(search_path)])
    if expectations:
        command.append("--expectations")
    command.extend(check_files)

    if nocapture:
        print("+ " + " ".join(command), flush=True)
        return subprocess.run(command, cwd=test_root).returncode

    result = subprocess.run(
        command,
        cwd=test_root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        print("+ " + " ".join(command), flush=True)
        print(result.stdout, end="")
        print(result.stderr, end="", file=sys.stderr)
    else:
        print(f"PASS {suite} ({len(check_files)} files)", flush=True)
    return result.returncode


def main() -> int:
    test_root_default = Path(__file__).resolve().parent
    repo_root_default = test_root_default.parent.parent.parent
    parser = argparse.ArgumentParser()
    # Default deferred until after parsing so `--release` can pick the profile.
    parser.add_argument("--pyrefly", type=Path, default=None)
    parser.add_argument(
        "--release",
        action="store_true",
        help="resolve the default pyrefly from target/release instead of target/debug",
    )
    parser.add_argument("--test-root", type=Path, default=test_root_default)
    parser.add_argument(
        "--tensor-shapes-root",
        type=Path,
        default=(
            Path(os.environ["TENSOR_SHAPES_ROOT"])
            if "TENSOR_SHAPES_ROOT" in os.environ
            else (repo_root_default / "tensor-shapes")
        ),
    )
    parser.add_argument(
        "--suite",
        choices=("all",) + SUITES,
        action="append",
        default=[],
    )
    parser.add_argument(
        "--nocapture",
        action="store_true",
        help="stream Pyrefly output instead of printing it only on failure",
    )
    args = parser.parse_args()

    if args.pyrefly is not None:
        pyrefly = args.pyrefly
    elif "PYREFLY" in os.environ:
        pyrefly = Path(os.environ["PYREFLY"])
    else:
        pyrefly = default_pyrefly(repo_root_default, args.release)
    pyrefly = resolve_pyrefly(pyrefly)

    suites = args.suite or ["all"]
    if "all" in suites:
        suites = list(SUITES)

    for suite in suites:
        result = run_check(
            pyrefly=pyrefly,
            test_root=args.test_root.resolve(),
            tensor_shapes_root=args.tensor_shapes_root.resolve(),
            suite=suite,
            nocapture=args.nocapture,
        )
        if result != 0:
            return result
    return 0


if __name__ == "__main__":
    sys.exit(main())
