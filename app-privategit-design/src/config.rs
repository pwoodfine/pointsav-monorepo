use std::{env, path::PathBuf};

pub struct Config {
    pub vault: PathBuf,
    pub bind: String,
    pub doorman_url: String,
    pub tenant: String,
}

impl Config {
    pub fn from_env() -> Self {
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
        }
    }
}
