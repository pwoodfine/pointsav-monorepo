mod ai;
mod config;
mod render;
mod routes;
mod schema;
mod state;
mod vault;

use minijinja::{path_loader, Environment};
use moonshot_index::{Document, InvertedIndex};
use std::{path::Path, sync::Arc};
use tokio::{
    net::TcpListener,
    sync::{watch, RwLock},
};
use tower_http::compression::CompressionLayer;

use config::Config;
use state::AppState;
use vault::SECTIONS;

#[tokio::main]
async fn main() {
    let cfg = Config::from_env();
    let nav = Arc::new(vault::discover_nav(&cfg.vault));
    let index = Arc::new(RwLock::new(InvertedIndex::new()));

    populate_index(&cfg.vault, &index).await;

    eprintln!(
        "app-privategit-design v{}: vault={:?} elements={} indexed={}",
        env!("CARGO_PKG_VERSION"),
        cfg.vault,
        nav.get("elements").map(|v| v.len()).unwrap_or(0),
        index.read().await.len(),
    );

    let (watch_tx, _initial_rx) = watch::channel(());
    let watch_tx = Arc::new(watch_tx);

    // Bridge blocking inotify thread → async index update + SSE broadcast
    let watcher = moonshot_fs_watch::FsWatcher::watch(&cfg.vault);
    {
        let watch_tx2 = watch_tx.clone();
        let vault2 = cfg.vault.clone();
        let index2 = index.clone();
        let (path_tx, mut path_rx) = tokio::sync::mpsc::channel::<std::path::PathBuf>(64);

        tokio::task::spawn_blocking(move || {
            for path in watcher.rx {
                if path_tx.blocking_send(path).is_err() {
                    break;
                }
            }
        });

        tokio::spawn(async move {
            while let Some(path) = path_rx.recv().await {
                if is_indexable_md(&path) {
                    reindex_file(&path, &vault2, &index2).await;
                }
                let _ = watch_tx2.send(());
            }
        });
    }

    let edit_token = Arc::new(generate_token());
    eprintln!("app-privategit-design edit token: {}", edit_token);

    let mut jinja = Environment::new();
    jinja.set_loader(path_loader(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/templates"
    )));
    jinja.add_filter("to_title", |s: String| -> String { vault::to_title(&s) });
    let env = Arc::new(jinja);

    let state = AppState {
        vault: cfg.vault,
        nav,
        tenant: cfg.tenant,
        doorman_url: cfg.doorman_url,
        watch_tx,
        index,
        edit_token,
        env,
    };

    let app = routes::build_router(state).layer(CompressionLayer::new());

    let listener = TcpListener::bind(&cfg.bind).await.expect("bind failed");
    eprintln!("app-privategit-design listening on {}", cfg.bind);
    axum::serve(listener, app).await.expect("serve failed");
}

fn is_indexable_md(path: &Path) -> bool {
    let lossy = path.to_string_lossy();
    path.is_file() && lossy.ends_with(".md") && !lossy.ends_with(".es.md")
}

async fn reindex_file(path: &Path, _vault: &Path, index: &Arc<RwLock<InvertedIndex>>) {
    let Ok(content) = tokio::fs::read_to_string(path).await else {
        return;
    };
    let (fm, body) = vault::parse_frontmatter(&content);
    let title = fm
        .get("name")
        .or_else(|| fm.get("title"))
        .cloned()
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        });
    let doc = Document {
        id: path.to_string_lossy().to_string(),
        title,
        body,
    };
    index.write().await.insert(doc);
}

fn generate_token() -> String {
    let mut buf = [0u8; 32];
    let mut f = std::fs::File::open("/dev/urandom").expect("open /dev/urandom");
    std::io::Read::read_exact(&mut f, &mut buf).expect("read entropy");
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

async fn populate_index(vault: &Path, index: &Arc<RwLock<InvertedIndex>>) {
    let mut idx = index.write().await;
    for section in SECTIONS {
        let sec_dir = vault.join(section);
        let Ok(entries) = std::fs::read_dir(&sec_dir) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let Ok(files) = std::fs::read_dir(entry.path()) else {
                continue;
            };
            for file in files.filter_map(|e| e.ok()) {
                let name = file.file_name().to_string_lossy().to_string();
                if !name.ends_with(".md") || name.ends_with(".es.md") {
                    continue;
                }
                let Ok(content) = std::fs::read_to_string(file.path()) else {
                    continue;
                };
                let (fm, body) = vault::parse_frontmatter(&content);
                let title = fm
                    .get("name")
                    .or_else(|| fm.get("title"))
                    .cloned()
                    .unwrap_or_else(|| name[..name.len() - 3].to_string());
                idx.insert(Document {
                    id: file.path().to_string_lossy().to_string(),
                    title,
                    body,
                });
            }
        }
    }
}
