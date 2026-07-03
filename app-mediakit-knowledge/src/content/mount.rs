// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Content mounts — the ordered set of source directories an instance serves.
//!
//! The first `primary` mount is editable (articles); `guide` mounts are
//! read-only operational runbooks. Built from `[[mount]]` config entries.

use std::path::PathBuf;

use crate::config::Config;

/// One content source directory.
#[derive(Debug, Clone)]
pub struct Mount {
    pub path: PathBuf,
    pub role: String,
    pub blueprint_set: Vec<String>,
}

/// The ordered mount set for an instance.
#[derive(Debug, Clone, Default)]
pub struct MountSet {
    pub mounts: Vec<Mount>,
}

impl MountSet {
    /// Build the mount set from parsed config.
    pub fn from_config(cfg: &Config) -> Self {
        let mounts = cfg
            .mounts
            .iter()
            .map(|m| Mount {
                path: m.path.clone(),
                role: m.role.clone(),
                blueprint_set: m.blueprint_set.clone(),
            })
            .collect();
        Self { mounts }
    }

    /// The primary (editable) mount, if any.
    pub fn primary(&self) -> Option<&Mount> {
        self.mounts.iter().find(|m| m.role == "primary")
    }

    pub fn is_empty(&self) -> bool {
        self.mounts.is_empty()
    }
}
