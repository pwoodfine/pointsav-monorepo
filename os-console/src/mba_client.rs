use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use russh::{
    client,
    keys::{load_secret_key, PrivateKeyWithHashAlg},
};
use system_gateway_mba::auth::compute_fingerprint;

pub struct MbaResult {
    pub active: bool,
    /// Client (identity) key fingerprint — shown in the pairing flow.
    pub fingerprint: String,
    /// Set when TOFU was used this connection (no prior pinned key).
    /// The caller should write this to the sidecar file on a successful connect.
    pub tofu_server_fingerprint: Option<String>,
}

struct MbaHandler {
    expected_fingerprint: Option<String>,
    /// Captures the server key fingerprint during TOFU (expected == None).
    observed: Arc<Mutex<Option<String>>>,
}

impl client::Handler for MbaHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        let got = compute_fingerprint(key);
        match &self.expected_fingerprint {
            Some(expected) if got != *expected => {
                eprintln!(
                    "os-console: MBA: HOST KEY MISMATCH — got {got}, expected {expected}; \
                    connection rejected (possible MITM)"
                );
                Ok(false)
            }
            None => {
                // TOFU: record the fingerprint so the caller can auto-pin it.
                eprintln!(
                    "os-console: MBA: TOFU — no pinned server key; accepting {got}; \
                    will pin to sidecar on successful auth"
                );
                *self.observed.lock().unwrap() = Some(got);
                Ok(true)
            }
            Some(_) => Ok(true),
        }
    }
}

// ---------------------------------------------------------------------------
// Sidecar — auto-pin on first TOFU; verified on all subsequent connections.
// Path: ~/.config/os-console/server-hostkey  (one line, the SHA256:… fingerprint)
// Priority at load time: explicit config > sidecar > TOFU.
// ---------------------------------------------------------------------------

fn sidecar_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config/os-console/server-hostkey")
}

fn read_sidecar() -> Option<String> {
    std::fs::read_to_string(sidecar_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Write the server fingerprint to the sidecar file so subsequent connections
/// are verified. Called by main.rs after a confirmed successful TOFU connect.
pub fn pin_server_key(fingerprint: &str) {
    let path = sidecar_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::write(&path, fingerprint).is_ok() {
        eprintln!(
            "os-console: MBA: server key pinned to {} — all future connections verified",
            path.display()
        );
    } else {
        eprintln!("os-console: MBA: WARNING — could not write server-hostkey sidecar; \
            next connection will TOFU again. Set totebox_known_host_key in config to pin manually.");
    }
}

pub async fn connect_mba(
    host: &str,
    port: u16,
    username: &str,
    key_path: &str,
    known_host_key: &str,
) -> MbaResult {
    let key = match load_secret_key(key_path, None) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("os-console: MBA: failed to load key at {key_path}: {e}");
            return MbaResult {
                active: false,
                fingerprint: "(key load failed)".into(),
                tofu_server_fingerprint: None,
            };
        }
    };

    let fingerprint = compute_fingerprint(key.public_key());

    // Resolve expected server key: explicit config > sidecar > TOFU.
    let expected = if !known_host_key.is_empty() {
        Some(known_host_key.to_string())
    } else {
        read_sidecar()
    };

    let observed: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let config = Arc::new(client::Config::default());
    let mut handle = match client::connect(
        config,
        (host, port),
        MbaHandler {
            expected_fingerprint: expected,
            observed: Arc::clone(&observed),
        },
    )
    .await
    {
        Ok(h) => h,
        Err(e) => {
            eprintln!("os-console: MBA: connection to {host}:{port} failed: {e}");
            return MbaResult {
                active: false,
                fingerprint,
                tofu_server_fingerprint: None,
            };
        }
    };

    let tofu_server_fingerprint = observed.lock().unwrap().clone();

    let key_with_alg = PrivateKeyWithHashAlg::new(Arc::new(key), None);
    match handle.authenticate_publickey(username, key_with_alg).await {
        Ok(result) if result.success() => {
            eprintln!("os-console: MBA: link active — {username}@{host}:{port}");
            MbaResult {
                active: true,
                fingerprint,
                tofu_server_fingerprint,
            }
        }
        Ok(_) => {
            eprintln!("os-console: MBA: fingerprint not registered — {fingerprint}");
            MbaResult {
                active: false,
                fingerprint,
                tofu_server_fingerprint,
            }
        }
        Err(e) => {
            eprintln!("os-console: MBA: auth error: {e}");
            MbaResult {
                active: false,
                fingerprint,
                tofu_server_fingerprint,
            }
        }
    }
}
