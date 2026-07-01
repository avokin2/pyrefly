/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::path::PathBuf;

use anyhow::bail;
use clap::Parser;
use pyrefly_config::error_kind::Severity;
use pyrefly_util::thread_pool::ThreadCount;

use crate::commands::util::CommandExitStatus;

/// Arguments for Bazel-powered type checking.
#[derive(Debug, Clone, Parser)]
pub struct BazelCheckArgs {
    /// Path to Bazel input JSON.
    input_path: PathBuf,

    /// Path to output JSON file containing Pyrefly type check results.
    #[arg(long = "output", short = 'o', value_name = "FILE")]
    output_path: PathBuf,

    /// Minimum severity level for errors to be displayed.
    /// Errors below this severity will not be shown. Defaults to "error".
    #[arg(long, value_enum, default_value_t = Severity::Error)]
    min_severity: Severity,
}

impl BazelCheckArgs {
    pub fn run(self, _thread_count: ThreadCount) -> anyhow::Result<CommandExitStatus> {
        let Self {
            input_path: _,
            output_path: _,
            min_severity: _,
        } = self;
        bail!("`pyrefly bazel-check` is not implemented yet")
    }
}
