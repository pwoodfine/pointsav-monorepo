use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum PairingState {
    /// MBA probe failed; pairing request not yet sent
    Unpaired { fingerprint: String },
    /// POST pair/request sent; showing code, waiting for operator
    AwaitingApproval {
        code: String,
        request_id: String,
        fingerprint: String,
    },
    /// Operator approved; MBA link transitioning to active
    Approved,
    /// Operator declined
    Denied,
    /// Code expired (10-minute TTL)
    Expired,
    /// Network or server error
    Error(String),
}

impl Default for PairingState {
    fn default() -> Self {
        PairingState::Unpaired {
            fingerprint: String::new(),
        }
    }
}

#[derive(Debug)]
pub enum PairingEvent {
    Approved,
    Denied,
    Expired,
    Error(String),
    /// Pair POST succeeded after retry; chassis should transition to AwaitingApproval.
    InitSuccess {
        code: String,
        request_id: String,
        fingerprint: String,
    },
}

#[derive(Serialize)]
struct NodeJoinReq {
    node_id: String,
    wireguard_pubkey: String,
    bottom: &'static str,
    arch: &'static str,
}

#[derive(Deserialize)]
struct NodeJoinResp {
    request_id: String,
    code: String,
}

#[derive(Deserialize)]
struct StatusResponse {
    state: String,
}

/// POST /v1/node-join/request — returns (request_id, code) on success.
/// Maps username@tenant → node_id, SSH public key → wireguard_pubkey field.
/// fingerprint is for display only and is not sent to the server.
pub fn post_pair_request(
    base_url: &str,
    username: &str,
    tenant: &str,
    public_key: &str,
    _fingerprint: &str,
) -> Result<(String, String)> {
    let url = format!("{}/v1/node-join/request", base_url.trim_end_matches('/'));
    let arch: &'static str = if std::env::consts::ARCH == "aarch64" {
        "aarch64"
    } else {
        "x86_64"
    };
    let bottom: &'static str = if arch == "aarch64" {
        "seL4"
    } else {
        "netbsd-compat"
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let resp: NodeJoinResp = client
        .post(&url)
        .json(&NodeJoinReq {
            node_id: format!("{username}@{tenant}"),
            wireguard_pubkey: public_key.to_string(),
            bottom,
            arch,
        })
        .send()?
        .error_for_status()?
        .json()?;
    Ok((resp.request_id, resp.code))
}

/// Spawns a background thread that polls /v1/pair/status/{request_id}.
/// Returns the receiver end; the thread sends a single terminal PairingEvent then exits.
pub fn spawn_status_poll(base_url: String, request_id: String) -> mpsc::Receiver<PairingEvent> {
    let (tx, rx) = mpsc::channel::<PairingEvent>();
    std::thread::spawn(move || {
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(PairingEvent::Error(e.to_string()));
                return;
            }
        };
        loop {
            let url = format!(
                "{}/v1/node-join/status/{}",
                base_url.trim_end_matches('/'),
                request_id
            );
            match client
                .get(&url)
                .send()
                .and_then(|r| r.json::<StatusResponse>())
            {
                Ok(s) => match s.state.as_str() {
                    "approved" => {
                        let _ = tx.send(PairingEvent::Approved);
                        break;
                    }
                    "denied" => {
                        let _ = tx.send(PairingEvent::Denied);
                        break;
                    }
                    "expired" => {
                        let _ = tx.send(PairingEvent::Expired);
                        break;
                    }
                    _ => {} // still pending — wait before next poll
                },
                Err(e) => {
                    let _ = tx.send(PairingEvent::Error(e.to_string()));
                    std::thread::sleep(Duration::from_secs(3));
                    continue;
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
    rx
}

/// Retry the pair POST in a background thread until it succeeds.
/// Sends `PairingEvent::InitSuccess` when the tunnel becomes reachable.
/// Used when the initial pair POST fails (tunnel not ready yet).
pub fn spawn_pair_init(
    base_url: String,
    username: String,
    tenant: String,
    public_key: String,
    fingerprint: String,
) -> mpsc::Receiver<PairingEvent> {
    let (tx, rx) = mpsc::channel::<PairingEvent>();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(3));
        match post_pair_request(&base_url, &username, &tenant, &public_key, &fingerprint) {
            Ok((request_id, code)) => {
                let _ = tx.send(PairingEvent::InitSuccess {
                    code,
                    request_id,
                    fingerprint: fingerprint.clone(),
                });
                break;
            }
            Err(e) => {
                let port_bound = std::net::TcpStream::connect_timeout(
                    &"127.0.0.1:9205".parse().unwrap(),
                    std::time::Duration::from_millis(200),
                )
                .is_ok();
                eprintln!("os-console: pair: POST failed (9205 bound={port_bound}): {e:?}");
            }
        }
    });
    rx
}
