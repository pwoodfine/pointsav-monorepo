//! Phase S3 — fleet watch + automated WireGuard peer-table + WORM ledger.
//!
//! Polls service-vm-fleet every 30s. For each node in the approved-nodes registry
//! (~/.local/share/ppn/nodes.jsonl) that now appears in the fleet, issues `wg set`
//! to add it as a WireGuard peer and writes a WORM topology event to service-fs.
//!
//! Requires CAP_NET_ADMIN (or root) to execute `wg set`.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tokio::time::{sleep, Duration};

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ApprovedNode {
    pub node_id: String,
    pub wireguard_pubkey: String,
    pub wg_ip: Option<String>, // populated from fleet after match
}

#[derive(Debug, Clone, Deserialize)]
pub struct FleetStatus {
    pub nodes: Vec<FleetNode>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FleetNode {
    pub node_id: String,
    pub wg_ip: String,
}

/// A peer that needs to be programmed into WireGuard.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingPeer {
    pub node_id: String,
    pub pubkey: String,
    pub wg_ip: String,
}

// ── Pure logic (testable) ────────────────────────────────────────────────────

/// Given approved nodes, fleet state, and already-programmed set,
/// return the list of peers that need to be added to WireGuard.
pub fn compute_pending_peers(
    approved: &[ApprovedNode],
    fleet_map: &HashMap<String, String>, // node_id → wg_ip
    programmed: &HashSet<String>,        // node_ids already in WG
) -> Vec<PendingPeer> {
    let mut out = Vec::new();
    for node in approved {
        if programmed.contains(&node.node_id) {
            continue;
        }
        if let Some(wg_ip) = fleet_map.get(&node.node_id) {
            out.push(PendingPeer {
                node_id: node.node_id.clone(),
                pubkey: node.wireguard_pubkey.clone(),
                wg_ip: wg_ip.clone(),
            });
        }
    }
    out
}

// ── I/O helpers ──────────────────────────────────────────────────────────────

/// Read approved nodes from the pairing service's nodes.jsonl file.
/// Each line is a JSON object with node_id and wireguard_pubkey fields.
pub fn read_approved_nodes(path: &PathBuf) -> Vec<ApprovedNode> {
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(path = %path.display(), err = %e, "nodes.jsonl not readable (ok if pairing service not yet run)");
            return Vec::new();
        }
    };
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| match serde_json::from_str::<ApprovedNode>(line) {
            Ok(n) => Some(n),
            Err(e) => {
                tracing::warn!(err = %e, line = %line, "skipping malformed nodes.jsonl line");
                None
            }
        })
        .collect()
}

/// Fetch fleet status from service-vm-fleet.
pub async fn fetch_fleet(fleet_url: &str) -> Result<FleetStatus, String> {
    let url = format!("{}/v1/fleet", fleet_url);
    let client = reqwest::Client::new();
    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => resp
            .json::<FleetStatus>()
            .await
            .map_err(|e| format!("fleet parse error: {e}")),
        Ok(resp) => Err(format!("fleet HTTP {}", resp.status())),
        Err(e) => Err(format!("fleet unreachable: {e}")),
    }
}

/// Execute `wg set <iface> peer <pubkey> allowed-ips <wg_ip>/32`.
/// Returns Ok(()) on success, Err(message) on failure.
pub fn program_wg_peer(iface: &str, pubkey: &str, wg_ip: &str) -> Result<(), String> {
    let allowed = format!("{}/32", wg_ip);
    let output = std::process::Command::new("wg")
        .args(["set", iface, "peer", pubkey, "allowed-ips", &allowed])
        .output()
        .map_err(|e| format!("wg exec failed: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "wg set failed ({}): {}",
            output.status,
            stderr.trim()
        ))
    }
}

/// Fire-and-forget WORM topology event to service-fs.
pub async fn write_worm_event(fs_url: &str, node_id: &str, wg_ip: &str, pubkey: &str) {
    let payload_id = format!(
        "ppn-topology-{}-{}",
        node_id,
        chrono::Utc::now().timestamp()
    );
    let body = serde_json::json!({
        "payload_id": payload_id,
        "payload": {
            "event": "peer_programmed",
            "node_id": node_id,
            "wg_ip": wg_ip,
            "pubkey_prefix": &pubkey[..8.min(pubkey.len())],
        }
    });
    let url = format!("{}/v1/append", fs_url);
    let client = reqwest::Client::new();
    match client.post(&url).json(&body).send().await {
        Ok(_) => tracing::info!(node_id = %node_id, "WORM topology event written"),
        Err(e) => tracing::warn!(node_id = %node_id, err = %e, "WORM write failed (non-fatal)"),
    }
}

// ── Main loop ────────────────────────────────────────────────────────────────

pub async fn run_fleet_watch(
    wg_iface: String,
    fleet_url: String,
    fs_url: String,
    nodes_path: PathBuf,
) {
    let poll_secs: u64 = std::env::var("FLEET_WATCH_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    tracing::info!(
        iface = %wg_iface,
        fleet_url = %fleet_url,
        nodes_path = %nodes_path.display(),
        poll_secs,
        "fleet watch started"
    );

    let mut programmed: HashSet<String> = HashSet::new();

    loop {
        let approved = read_approved_nodes(&nodes_path);

        let fleet = match fetch_fleet(&fleet_url).await {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(err = %e, "fleet fetch failed — will retry");
                sleep(Duration::from_secs(poll_secs)).await;
                continue;
            }
        };

        let fleet_map: HashMap<String, String> = fleet
            .nodes
            .into_iter()
            .map(|n| (n.node_id, n.wg_ip))
            .collect();

        let pending = compute_pending_peers(&approved, &fleet_map, &programmed);

        for peer in pending {
            match program_wg_peer(&wg_iface, &peer.pubkey, &peer.wg_ip) {
                Ok(()) => {
                    tracing::info!(
                        node_id = %peer.node_id,
                        wg_ip = %peer.wg_ip,
                        "WireGuard peer programmed"
                    );
                    write_worm_event(&fs_url, &peer.node_id, &peer.wg_ip, &peer.pubkey).await;
                    programmed.insert(peer.node_id);
                }
                Err(e) => {
                    tracing::warn!(
                        node_id = %peer.node_id,
                        wg_ip = %peer.wg_ip,
                        err = %e,
                        "wg set failed — will retry next cycle"
                    );
                }
            }
        }

        sleep(Duration::from_secs(poll_secs)).await;
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_fleet_map(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn make_programmed(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn approved(node_id: &str, pubkey: &str) -> ApprovedNode {
        ApprovedNode {
            node_id: node_id.to_string(),
            wireguard_pubkey: pubkey.to_string(),
            wg_ip: None,
        }
    }

    #[test]
    fn no_approved_nodes_returns_empty() {
        let fleet = make_fleet_map(&[("laptop-a-1", "10.8.0.6")]);
        let result = compute_pending_peers(&[], &fleet, &HashSet::new());
        assert!(result.is_empty());
    }

    #[test]
    fn approved_not_in_fleet_returns_empty() {
        let approved_nodes = vec![approved("new-node", "PUBKEY123")];
        let fleet = make_fleet_map(&[("different-node", "10.8.0.9")]);
        let result = compute_pending_peers(&approved_nodes, &fleet, &HashSet::new());
        assert!(result.is_empty());
    }

    #[test]
    fn approved_in_fleet_not_programmed_returns_peer() {
        let approved_nodes = vec![approved("laptop-a-1", "ABC123pubkey")];
        let fleet = make_fleet_map(&[("laptop-a-1", "10.8.0.6")]);
        let result = compute_pending_peers(&approved_nodes, &fleet, &HashSet::new());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].node_id, "laptop-a-1");
        assert_eq!(result[0].wg_ip, "10.8.0.6");
        assert_eq!(result[0].pubkey, "ABC123pubkey");
    }

    #[test]
    fn already_programmed_node_skipped() {
        let approved_nodes = vec![approved("laptop-a-1", "ABC123pubkey")];
        let fleet = make_fleet_map(&[("laptop-a-1", "10.8.0.6")]);
        let programmed = make_programmed(&["laptop-a-1"]);
        let result = compute_pending_peers(&approved_nodes, &fleet, &programmed);
        assert!(result.is_empty());
    }

    #[test]
    fn multiple_nodes_only_new_ones_returned() {
        let approved_nodes = vec![
            approved("gcp-cloud-1", "GCPKEY"),
            approved("laptop-a-1", "LAKEY"),
            approved("laptop-b-1", "LBKEY"),
        ];
        let fleet = make_fleet_map(&[
            ("gcp-cloud-1", "10.8.0.9"),
            ("laptop-a-1", "10.8.0.6"),
            ("laptop-b-1", "10.8.0.1"),
        ]);
        let programmed = make_programmed(&["gcp-cloud-1"]); // already done
        let result = compute_pending_peers(&approved_nodes, &fleet, &programmed);
        assert_eq!(result.len(), 2);
        let ids: Vec<_> = result.iter().map(|p| p.node_id.as_str()).collect();
        assert!(ids.contains(&"laptop-a-1"));
        assert!(ids.contains(&"laptop-b-1"));
    }

    #[test]
    fn read_approved_nodes_parses_jsonl() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"node_id":"n1","wireguard_pubkey":"PK1","bottom":"seL4","arch":"x86_64","request_id":"r1","approved_at":"2026-06-14T00:00:00Z"}}"#
        ).unwrap();
        writeln!(
            file,
            r#"{{"node_id":"n2","wireguard_pubkey":"PK2","bottom":"netbsd-compat","arch":"aarch64","request_id":"r2","approved_at":"2026-06-14T00:00:01Z"}}"#
        ).unwrap();
        let path = file.path().to_path_buf();
        let nodes = read_approved_nodes(&path);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].node_id, "n1");
        assert_eq!(nodes[0].wireguard_pubkey, "PK1");
        assert_eq!(nodes[1].node_id, "n2");
    }

    #[test]
    fn read_approved_nodes_skips_malformed_lines() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"node_id":"n1","wireguard_pubkey":"PK1","bottom":"seL4","arch":"x86_64","request_id":"r1","approved_at":"2026-06-14T00:00:00Z"}}"#).unwrap();
        writeln!(file, "not-valid-json").unwrap();
        writeln!(file, r#"{{"node_id":"n3","wireguard_pubkey":"PK3","bottom":"seL4","arch":"x86_64","request_id":"r3","approved_at":"2026-06-14T00:00:02Z"}}"#).unwrap();
        let path = file.path().to_path_buf();
        let nodes = read_approved_nodes(&path);
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn read_approved_nodes_missing_file_returns_empty() {
        let path = PathBuf::from("/tmp/does-not-exist-ppn-nodes.jsonl");
        let nodes = read_approved_nodes(&path);
        assert!(nodes.is_empty());
    }
}
