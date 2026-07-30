use std::{env, net::SocketAddr, path::PathBuf};

#[derive(Clone)]
pub struct Config {
    pub design_system_dir: PathBuf,
    pub vault_dir: PathBuf,
    pub library_dir: PathBuf,
    pub static_dir: PathBuf,
    #[allow(dead_code)]
    pub tenant: String,
    #[allow(dead_code)]
    pub public_url: String,
    pub bind: SocketAddr,
}

impl Config {
    pub fn from_env() -> Self {
        let cwd = env::current_dir().unwrap_or_default();
        Self {
            design_system_dir: env::var("BIM_DESIGN_SYSTEM_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| cwd.join("woodfine-bim-library")),
            vault_dir: env::var("BIM_VAULT_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| cwd.join("woodfine-bim-library")),
            library_dir: env::var("BIM_LIBRARY_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| cwd.join("woodfine-bim-library")),
            static_dir: env::var("BIM_STATIC_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| cwd.join("app-privategit-bim/src/assets")),
            tenant: env::var("BIM_TENANT").unwrap_or_else(|_| "woodfine".into()),
            public_url: env::var("BIM_PUBLIC_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:9204".into()),
            bind: env::var("BIM_BIND")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| "127.0.0.1:9204".parse().unwrap()),
        }
    }
}
