use crate::config::Config;
use crate::schema::dtcg;
use serde_json::Value;
use std::{collections::HashMap, path::Path, sync::Arc};
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub tokens: Arc<HashMap<String, Value>>,
    pub token_count: usize,
    pub components_count: usize,
    pub research_count: usize,
    pub events_tx: broadcast::Sender<String>,
}

impl AppState {
    pub async fn new(config: &Config) -> Result<Self, Box<dyn std::error::Error>> {
        let tokens = dtcg::load_tokens(&config.design_system_dir)?;
        let token_count = count_entities(&tokens);
        let components_count = count_ifc_files(&config.library_dir.join("key-plans"));
        let research_count = count_md_files(&config.vault_dir.join("research"));
        let (events_tx, _) = broadcast::channel::<String>(64);
        Ok(Self {
            config: Arc::new(config.clone()),
            tokens: Arc::new(tokens),
            token_count,
            components_count,
            research_count,
            events_tx,
        })
    }
}

fn count_entities(tokens: &HashMap<String, Value>) -> usize {
    tokens
        .values()
        .filter_map(|file| file.get("bim").and_then(|b| b.as_object()))
        .flat_map(|bim| bim.values())
        .filter_map(|cat| cat.as_object())
        .flat_map(|cat| cat.values())
        .filter(|v| v.get("$type").is_some())
        .count()
}

fn count_ifc_files(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("ifc"))
                .count()
        })
        .unwrap_or(0)
}

fn count_md_files(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
                .count()
        })
        .unwrap_or(0)
}

pub fn spawn_file_watcher(state: AppState, config: &Config) {
    use notify::{RecursiveMode, Watcher};

    let watcher_tx = state.events_tx.clone();
    let watch_dir = config.design_system_dir.join("tokens").join("bim");

    tokio::spawn(async move {
        let (inner_tx, mut inner_rx) = tokio::sync::mpsc::channel::<String>(8);
        let mut watcher =
            match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    let path_str = event
                        .paths
                        .first()
                        .and_then(|p| p.to_str())
                        .unwrap_or("")
                        .to_string();
                    let _ = inner_tx.blocking_send(path_str);
                }
            }) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("warn: file watcher init failed: {e}");
                    return;
                }
            };
        if let Err(e) = watcher.watch(&watch_dir, RecursiveMode::Recursive) {
            eprintln!("warn: file watcher watch failed: {e}");
            return;
        }
        while let Some(path_str) = inner_rx.recv().await {
            let msg = format!(
                r#"{{"event":"token-updated","path":"{}"}}"#,
                path_str.replace('\\', "/")
            );
            let _ = watcher_tx.send(msg);
        }
    });
}
