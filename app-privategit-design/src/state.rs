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
}
