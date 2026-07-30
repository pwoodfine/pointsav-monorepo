mod fleet_watch;

use bytes::Bytes;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::env;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::UdpSocket as TokioUdpSocket;
use tokio::sync::Mutex;
use warp::Filter;

#[derive(Deserialize, Debug)]
struct TranslateRequest {
    raw_input: String,
}
#[derive(Deserialize, Serialize, Debug)]
struct AuthorizedPayload {
    intent: String,
    target: String,
}

// 16-byte binary mesh frame (Genesis Protocol wire format).
// [0..2]  op_code: u16 BE  — PING=0x0001, ISOLATE=0x0002, PONG=0x0003, unknown=0x0000
// [2..4]  target_node: u16 BE — 0x0001=NODE-CLOUD-RELAY, 0x0002=NODE-LAPTOP-A,
//                               0x0003=NODE-IMAC-12, 0xFFFF=broadcast
// [4..8]  timestamp: u32 BE  — Unix seconds
// [8..16] reserved: [u8; 8]  — zeroed
fn build_mesh_frame(intent: &str, target: &str) -> [u8; 16] {
    let op: u16 = match intent {
        "ping" => 0x0001,
        "isolate" => 0x0002,
        "pong" => 0x0003,
        _ => 0x0000,
    };
    let node: u16 = match target {
        "NODE-CLOUD-RELAY" => 0x0001,
        "NODE-LAPTOP-A" => 0x0002,
        "NODE-IMAC-12" => 0x0003,
        _ => 0xFFFF,
    };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32;
    let mut frame = [0u8; 16];
    frame[0..2].copy_from_slice(&op.to_be_bytes());
    frame[2..4].copy_from_slice(&node.to_be_bytes());
    frame[4..8].copy_from_slice(&ts.to_be_bytes());
    frame
}

fn build_pong_frame(target_node: u16) -> [u8; 16] {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32;
    let mut frame = [0u8; 16];
    frame[0..2].copy_from_slice(&0x0003u16.to_be_bytes()); // PONG
    frame[2..4].copy_from_slice(&target_node.to_be_bytes());
    frame[4..8].copy_from_slice(&ts.to_be_bytes());
    frame
}

/// Parse PPN_PEERS env var (comma-separated host:port or bare IPs).
/// Default: "10.8.0.1:9206,10.8.0.9:9206"
fn resolve_peers() -> Vec<String> {
    let raw = env::var("PPN_PEERS").unwrap_or_else(|_| "10.8.0.1:9206,10.8.0.9:9206".to_string());
    raw.split(',')
        .map(|s| {
            let s = s.trim().to_string();
            // If no port included, append canonical mesh port
            if s.contains(':') {
                s
            } else {
                format!("{}:{}", s, MESH_LISTEN_PORT)
            }
        })
        .collect()
}

#[derive(Serialize)]
struct TerminalResponse {
    status: String,
    message: String,
    data: Option<serde_json::Value>,
}

const MESH_LISTEN_PORT: u16 = 9206;
const HTTP_PORT: u16 = 8085;

/// Spawn the UDP listening task on :9206 (or PPN_MESH_LISTEN_PORT override).
/// Receives 16-byte mesh frames from WireGuard peers.
async fn spawn_mesh_listener() {
    let listen_port: u16 = env::var("PPN_MESH_LISTEN_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(MESH_LISTEN_PORT);

    let bind_addr = format!("0.0.0.0:{}", listen_port);
    let sock = match TokioUdpSocket::bind(&bind_addr).await {
        Ok(s) => {
            tracing::info!(port = listen_port, "mesh listener bound");
            s
        }
        Err(e) => {
            tracing::error!(port = listen_port, err = %e, "failed to bind mesh listener");
            return;
        }
    };

    let mut buf = [0u8; 256];
    loop {
        match sock.recv_from(&mut buf).await {
            Ok((n, src_addr)) => {
                if n < 16 {
                    tracing::warn!(from = %src_addr, bytes = n, "short mesh frame ignored");
                    continue;
                }
                let frame = &buf[..16];
                let op_code = u16::from_be_bytes([frame[0], frame[1]]);
                let target_node = u16::from_be_bytes([frame[2], frame[3]]);
                let timestamp = u32::from_be_bytes([frame[4], frame[5], frame[6], frame[7]]);

                tracing::info!(
                    op_code = format!("{:#06x}", op_code),
                    target_node = format!("{:#06x}", target_node),
                    timestamp,
                    from = %src_addr,
                    "mesh frame received"
                );

                // PING (0x0001) addressed to this node or broadcast (0xFFFF) — reply PONG
                if op_code == 0x0001 && (target_node == 0xFFFF || target_node <= 0x0003) {
                    let pong = build_pong_frame(target_node);
                    match sock.send_to(&pong, src_addr).await {
                        Ok(_) => tracing::info!(to = %src_addr, "PONG sent"),
                        Err(e) => tracing::warn!(to = %src_addr, err = %e, "PONG send failed"),
                    }
                }
            }
            Err(e) => {
                tracing::error!(err = %e, "mesh listener recv error");
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_env("RUST_LOG")
                .add_directive("app_network_admin=info".parse().unwrap()),
        )
        .init();

    let node_id = env::var("NODE_ID").unwrap_or_else(|_| "F8-TERMINAL-GATEWAY".to_string());
    let udp_socket = UdpSocket::bind("0.0.0.0:0").expect("[FATAL] Hardware rejection.");
    let socket_arc = Arc::new(Mutex::new(udp_socket));

    // Spawn UDP mesh listener on :9206
    tokio::spawn(spawn_mesh_listener());

    // Spawn Phase S3 fleet watch — polls fleet + programs WireGuard peers + writes WORM events.
    // Requires CAP_NET_ADMIN / root to execute `wg set`. Skips gracefully if wg is unavailable.
    {
        let wg_iface = env::var("WG_IFACE").unwrap_or_else(|_| "wg0".to_string());
        let fleet_url =
            env::var("FLEET_URL").unwrap_or_else(|_| "http://127.0.0.1:9203".to_string());
        let fs_url =
            env::var("SERVICE_FS_URL").unwrap_or_else(|_| "http://127.0.0.1:9100".to_string());
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let nodes_path = env::var("NODES_JSONL_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(home).join(".local/share/ppn/nodes.jsonl"));
        tokio::spawn(fleet_watch::run_fleet_watch(
            wg_iface, fleet_url, fs_url, nodes_path,
        ));
    }

    let translate_route = warp::post()
        .and(warp::path("translate"))
        .and(warp::body::json())
        .and_then(handle_translation);
    let authorize_route = warp::post()
        .and(warp::path("authorize"))
        .and(warp::body::json())
        .and(with_socket(socket_arc.clone()))
        .and(with_node_id(node_id.clone()))
        .and_then(handle_authorization);
    let upload_route = warp::post()
        .and(warp::path("upload"))
        .and(warp::header::<String>("x-file-name"))
        .and(warp::body::bytes())
        .and_then(handle_upload);

    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["Content-Type", "x-file-name"])
        .allow_methods(vec!["POST"]);

    tracing::info!(port = HTTP_PORT, "HTTP server starting");
    warp::serve(
        translate_route
            .or(authorize_route)
            .or(upload_route)
            .with(cors),
    )
    .run(([0, 0, 0, 0], HTTP_PORT))
    .await;
}

fn with_socket(
    socket: Arc<Mutex<UdpSocket>>,
) -> impl Filter<Extract = (Arc<Mutex<UdpSocket>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || socket.clone())
}
fn with_node_id(
    node_id: String,
) -> impl Filter<Extract = (String,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || node_id.clone())
}

// F12 INGESTION: Routed through service-fs
async fn handle_upload(filename: String, body: Bytes) -> Result<impl warp::Reply, warp::Rejection> {
    let safe_filename = filename.replace("/", "_").replace("\\", "_");
    let target_filename = format!("VAULT_INGRESS_MANUAL_{}", safe_filename);

    let client = reqwest::Client::new();
    match client
        .post("http://127.0.0.1:8095/vault/ingress")
        .header("x-file-name", target_filename.clone())
        .body(body.to_vec())
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => {
            tracing::info!(file = %target_filename, "F12 payload routed to service-fs");
            Ok(warp::reply::json(&TerminalResponse {
                status: "SUCCESS".to_string(),
                message: "Asset locked into cold storage.".to_string(),
                data: None,
            }))
        }
        _ => {
            tracing::error!(file = %target_filename, "failed to route asset to service-fs");
            Ok(warp::reply::json(&TerminalResponse {
                status: "ERROR".to_string(),
                message: "File System Gatekeeper rejected payload.".to_string(),
                data: None,
            }))
        }
    }
}

async fn handle_translation(req: TranslateRequest) -> Result<impl warp::Reply, warp::Rejection> {
    let res = reqwest::Client::new()
        .post("http://localhost:9080/v1/translate")
        .json(&serde_json::json!({"input": req.raw_input}))
        .send()
        .await;
    match res {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<serde_json::Value>().await {
                    Ok(json_val) => Ok(warp::reply::json(&TerminalResponse {
                        status: "PENDING_AUTHORIZATION".to_string(),
                        message: "Awaiting Verification.".to_string(),
                        data: Some(json_val),
                    })),
                    Err(_) => Ok(warp::reply::json(&TerminalResponse {
                        status: "ERROR".to_string(),
                        message: "Invalid JSON.".to_string(),
                        data: None,
                    })),
                }
            } else {
                Ok(warp::reply::json(&TerminalResponse {
                    status: "ERROR".to_string(),
                    message: resp.status().to_string(),
                    data: None,
                }))
            }
        }
        Err(e) => Ok(warp::reply::json(&TerminalResponse {
            status: "FATAL".to_string(),
            message: e.to_string(),
            data: None,
        })),
    }
}

async fn handle_authorization(
    auth: AuthorizedPayload,
    socket: Arc<Mutex<UdpSocket>>,
    _node_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let timestamp = Utc::now().to_rfc3339();
    let frame = build_mesh_frame(&auth.intent, &auth.target);
    let sock = socket.lock().await;
    let mut success_count = 0;

    // Resolve peers from env (PPN_PEERS) at request time so live changes are respected.
    let all_peers = resolve_peers();

    let target_addrs: Vec<String> = match auth.target.as_str() {
        "NODE-CLOUD-RELAY" => vec![format!("10.8.0.1:{}", MESH_LISTEN_PORT)],
        "NODE-LAPTOP-A" => vec![format!("10.8.0.2:{}", MESH_LISTEN_PORT)],
        "NODE-IMAC-12" => vec![format!("10.8.0.3:{}", MESH_LISTEN_PORT)],
        _ => all_peers,
    };

    for addr in &target_addrs {
        if sock.send_to(&frame, addr).is_ok() {
            tracing::info!(to = %addr, intent = %auth.intent, "mesh frame sent");
            success_count += 1;
        } else {
            tracing::warn!(to = %addr, "mesh frame send failed");
        }
    }

    if success_count > 0 {
        Ok(warp::reply::json(&TerminalResponse {
            status: "SUCCESS".to_string(),
            message: format!("Payload injected to {} mesh nodes.", success_count),
            data: Some(serde_json::json!({"timestamp": timestamp})),
        }))
    } else {
        Ok(warp::reply::json(&TerminalResponse {
            status: "FATAL".to_string(),
            message: "Hardware rejection".to_string(),
            data: None,
        }))
    }
}
