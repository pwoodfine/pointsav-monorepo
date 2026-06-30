use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;

/// Mirrors service-content's `PairingRecord` (service-content/src/pairing.rs).
/// Extra fields on the wire (e.g. `nonce`) are ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct PeerRecord {
    pub public_key: String,
    pub issuer: String,
    pub peer_type: String,
    pub role: String,
    #[serde(default)]
    pub archive_scope: Vec<String>,
    pub node_label: String,
    pub paired_on: String,
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("http client")
}

/// Fetch all paired nodes from service-content `GET /v1/pairs`.
pub fn fetch_peers(content_endpoint: &str) -> Result<Vec<PeerRecord>> {
    let url = format!("{}/v1/pairs", content_endpoint.trim_end_matches('/'));
    let peers: Vec<PeerRecord> = client().get(&url).send()?.error_for_status()?.json()?;
    Ok(peers)
}
