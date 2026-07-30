//! Integration tests for the PPN node-join ceremony.
//!
//! Each test spawns the ppn-pairing-server binary against an isolated tempdir
//! (via `HOME` env var) and exercises the full HTTP flow.

use std::net::TcpListener;
use std::process::{Child, Command};
use std::time::Duration;
use tempfile::TempDir;

struct TestServer {
    child: Child,
    port: u16,
    _tmpdir: TempDir,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

fn start_server() -> TestServer {
    let tmpdir = TempDir::new().unwrap();
    let port = free_port();
    let bin = env!("CARGO_BIN_EXE_ppn-pairing-server");
    let child = Command::new(bin)
        .arg(format!("127.0.0.1:{port}"))
        .env("HOME", tmpdir.path())
        .spawn()
        .expect("failed to spawn ppn-pairing-server");
    std::thread::sleep(Duration::from_millis(300));
    TestServer {
        child,
        port,
        _tmpdir: tmpdir,
    }
}

fn url(port: u16, path: &str) -> String {
    format!("http://127.0.0.1:{port}{path}")
}

fn join_body() -> serde_json::Value {
    serde_json::json!({
        "node_id": "test-node-1",
        "wireguard_pubkey": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        "bottom": "netbsd-compat",
        "arch": "x86_64"
    })
}

/// Full approve flow: request → approve with correct code → status shows approved.
#[test]
fn test_full_approve_flow() {
    let srv = start_server();
    let port = srv.port;

    let resp = ureq::post(&url(port, "/v1/node-join/request"))
        .set("Content-Type", "application/json")
        .send_string(&join_body().to_string())
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
    let request_id = body["request_id"].as_str().unwrap().to_string();
    let code = body["code"].as_str().unwrap().to_string();
    assert!(!request_id.is_empty());
    assert!(!code.is_empty() && code.contains('-'));

    let approve = ureq::post(&url(port, "/v1/node-join/approve"))
        .set("Content-Type", "application/json")
        .send_string(&serde_json::json!({"code": code}).to_string())
        .unwrap();
    assert_eq!(approve.status(), 200);
    let ab: serde_json::Value = serde_json::from_str(&approve.into_string().unwrap()).unwrap();
    assert_eq!(ab["status"], "approved");

    let status = ureq::get(&url(port, &format!("/v1/node-join/status/{request_id}")))
        .call()
        .unwrap();
    assert_eq!(status.status(), 200);
    let sb: serde_json::Value = serde_json::from_str(&status.into_string().unwrap()).unwrap();
    assert_eq!(sb["state"], "approved");
}

/// Deny flow: request → deny → status shows denied.
#[test]
fn test_deny_flow() {
    let srv = start_server();
    let port = srv.port;

    let resp = ureq::post(&url(port, "/v1/node-join/request"))
        .set("Content-Type", "application/json")
        .send_string(&join_body().to_string())
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
    let request_id = body["request_id"].as_str().unwrap().to_string();
    let code = body["code"].as_str().unwrap().to_string();

    let deny = ureq::post(&url(port, "/v1/node-join/deny"))
        .set("Content-Type", "application/json")
        .send_string(&serde_json::json!({"code": code}).to_string())
        .unwrap();
    assert_eq!(deny.status(), 200);
    let db: serde_json::Value = serde_json::from_str(&deny.into_string().unwrap()).unwrap();
    assert_eq!(db["status"], "denied");

    let status = ureq::get(&url(port, &format!("/v1/node-join/status/{request_id}")))
        .call()
        .unwrap();
    assert_eq!(status.status(), 200);
    let sb: serde_json::Value = serde_json::from_str(&status.into_string().unwrap()).unwrap();
    assert_eq!(sb["state"], "denied");
}

/// Wrong code returns 404 — the short code is unguessable in practice.
#[test]
fn test_wrong_code_rejected() {
    let srv = start_server();
    let port = srv.port;

    // Register a real request so there's state in the DB.
    let resp = ureq::post(&url(port, "/v1/node-join/request"))
        .set("Content-Type", "application/json")
        .send_string(&join_body().to_string())
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Approve with a code that doesn't exist.
    let result = ureq::post(&url(port, "/v1/node-join/approve"))
        .set("Content-Type", "application/json")
        .send_string(&serde_json::json!({"code": "ZZZZ-ZZZZ"}).to_string());
    match result {
        Err(ureq::Error::Status(code, _)) => assert_eq!(code, 404),
        Ok(r) => panic!("expected 404, got {}", r.status()),
        Err(e) => panic!("unexpected error: {e}"),
    }
}

/// Pending list includes a newly submitted request.
#[test]
fn test_pending_list() {
    let srv = start_server();
    let port = srv.port;

    let resp = ureq::post(&url(port, "/v1/node-join/request"))
        .set("Content-Type", "application/json")
        .send_string(&join_body().to_string())
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
    let request_id = body["request_id"].as_str().unwrap().to_string();

    let list = ureq::get(&url(port, "/v1/node-join/pending"))
        .call()
        .unwrap();
    assert_eq!(list.status(), 200);
    let lb: serde_json::Value = serde_json::from_str(&list.into_string().unwrap()).unwrap();
    let pending = lb["pending"].as_array().unwrap();
    assert!(
        pending.iter().any(|r| r["request_id"] == request_id),
        "submitted request_id not found in pending list"
    );
}
