/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

/// Opaque configuration owned by framework integrations.
///
/// Core configuration and solving code only transport these string options. Their meaning is
/// interpreted by the corresponding framework module.
#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct FrameworkConfig {
    #[serde(flatten)]
    frameworks: BTreeMap<String, BTreeMap<String, String>>,
}

impl FrameworkConfig {
    pub fn option(&self, framework: &str, option: &str) -> Option<&str> {
        self.frameworks
            .get(framework)?
            .get(option)
            .map(String::as_str)
    }

    pub fn set_option(&mut self, framework: &str, option: &str, value: String) {
        self.frameworks
            .entry(framework.to_owned())
            .or_default()
            .insert(option.to_owned(), value);
    }

    pub fn is_empty(&self) -> bool {
        self.frameworks.is_empty()
    }
}
