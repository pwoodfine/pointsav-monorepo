use std::sync::Arc;

use russh::{
    client,
    keys::{load_secret_key, PrivateKeyWithHashAlg},
};
use system_gateway_mba::auth::compute_fingerprint;

pub struct MbaResult {
    pub active: bool,
    pub fingerprint: String,
}

struct MbaHandler;

impl client::Handler for MbaHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        _key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        // Accept any server host key for peer-to-peer simulation.
        // Production: verify against a stored host key or known_hosts.
        Ok(true)
    }
}

pub async fn connect_mba(host: &str, port: u16, username: &str, key_path: &str) -> MbaResult {
    let key = match load_secret_key(key_path, None) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("os-console: MBA: failed to load key at {key_path}: {e}");
            return MbaResult {
                active: false,
                fingerprint: "(key load failed)".into(),
            };
        }
    };

    let fingerprint = compute_fingerprint(key.public_key());

    let config = Arc::new(client::Config::default());
    let mut handle = match client::connect(config, (host, port), MbaHandler).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("os-console: MBA: connection to {host}:{port} failed: {e}");
            return MbaResult {
                active: false,
                fingerprint,
            };
        }
    };

    let key_with_alg = PrivateKeyWithHashAlg::new(Arc::new(key), None);
    match handle.authenticate_publickey(username, key_with_alg).await {
        Ok(result) if result.success() => {
            eprintln!("os-console: MBA: link active — {username}@{host}:{port}");
            MbaResult {
                active: true,
                fingerprint,
            }
        }
        Ok(_) => {
            eprintln!("os-console: MBA: fingerprint not registered — {fingerprint}");
            MbaResult {
                active: false,
                fingerprint,
            }
        }
        Err(e) => {
            eprintln!("os-console: MBA: auth error: {e}");
            MbaResult {
                active: false,
                fingerprint,
            }
        }
    }
}
