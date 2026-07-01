use std::{collections::HashMap, env, path::PathBuf};

pub struct Config {
    pub vault: PathBuf,
    pub bind: String,
    pub doorman_url: String,
    pub tenant: String,
    /// DESIGN-BUNDLE mounts: bundle name -> canonical source directory owned by
    /// another archive (mounted read-only for serving + /download; never copied).
    pub bundle_mounts: HashMap<String, PathBuf>,
}

impl Config {
    pub fn from_env() -> Self {
        let mut bundle_mounts = HashMap::new();
        bundle_mounts.insert(
            "editorial-style-guide".to_string(),
            PathBuf::from(env::var("BUNDLE_MOUNT_EDITORIAL_STYLE_GUIDE").unwrap_or_else(|_| {
                "/srv/foundry/clones/project-editorial/media-knowledge-documentation/.internal/style-guides".to_string()
            })),
        );

        Config {
            vault: PathBuf::from(
                env::var("DESIGN_VAULT_DIR")
                    .or_else(|_| env::var("DESIGN_VAULT"))
                    .unwrap_or_else(|_| {
                        "/srv/foundry/deployments/vault-privategit-design-1".to_string()
                    }),
            ),
            bind: env::var("DESIGN_BIND").unwrap_or_else(|_| "127.0.0.1:9094".to_string()),
            doorman_url: env::var("DOORMAN_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:9092".to_string()),
            tenant: env::var("DESIGN_TENANT").unwrap_or_else(|_| "pointsav".to_string()),
            bundle_mounts,
        }
    }
}
