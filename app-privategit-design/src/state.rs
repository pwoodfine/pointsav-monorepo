// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use minijinja::Environment;
use moonshot_index::InvertedIndex;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::{watch, RwLock};

#[derive(Clone)]
pub struct AppState {
    pub vault: PathBuf,
    pub nav: Arc<HashMap<String, Vec<String>>>,
    #[allow(dead_code)]
    pub tenant: String,
    pub doorman_url: String,
    pub watch_tx: Arc<watch::Sender<()>>,
    pub index: Arc<RwLock<InvertedIndex>>,
    pub edit_token: Arc<String>,
    pub env: Arc<Environment<'static>>,
    pub bundle_mounts: Arc<HashMap<String, PathBuf>>,
    pub static_dir: PathBuf,
    /// `components/` slugs grouped by recipe.json `category` (generic vs GIS-origin vs
    /// wiki-origin) — precomputed once at startup, same pattern as `nav`.
    pub component_groups: Arc<Vec<(String, Vec<String>)>>,
}
