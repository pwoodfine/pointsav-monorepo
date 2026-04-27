// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// End-to-end Ring 1 pipeline test — closes the loop from identity input
// to persisted WORM record.
//
// Wires a real `service-fs` daemon (axum on an ephemeral localhost port,
// PosixTileLedger over a temp directory) to a `service-people` router
// driven via `tower::ServiceExt::oneshot`. A POST to `/mcp`
// `tools/call` for `identity.append` flows through the FsClient over
// real HTTP into service-fs, then we GET `/v1/entries?since=0` from
// service-fs and assert the Person record round-trips byte-faithfully.
// Then we POST `tools/call` for `identity.lookup` and assert the local
// PeopleStore cache also has the record.
//
// The test runs on a multi-threaded tokio runtime because
// service-people's FsClient is synchronous (ureq blocking) and is
// invoked from inside an async axum handler — that blocking call needs
// a worker thread distinct from the one driving service-fs's serve
// loop.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

static TMPCTR: AtomicU64 = AtomicU64::new(0);

fn tmpdir() -> PathBuf {
    let n = TMPCTR.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "svc-people-e2e-{}-{}",
        std::process::id(),
        n
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn body_json(body: Body) -> serde_json::Value {
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn wait_for_ready(url: String) {
    for _ in 0..100 {
        let probe = url.clone();
        let ok = tokio::task::spawn_blocking(move || {
            ureq::get(&probe)
                .config()
                .timeout_global(Some(Duration::from_millis(200)))
                .build()
                .call()
                .is_ok()
        })
        .await
        .unwrap();
        if ok {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("service did not become ready at {url}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn identity_append_persists_to_service_fs_and_cache() {
    let module_id = "e2e-tenant";
    let fs_root = tmpdir();

    // ── service-fs: real HTTP server on ephemeral localhost port ──
    let fs_ledger =
        service_fs::posix_tile_open(&fs_root, module_id, None::<&Path>).unwrap();
    let fs_audit_ledger = service_fs::posix_tile_open(
        fs_root.join(module_id),
        "audit-log",
        None::<&Path>,
    )
    .unwrap();
    let fs_state = Arc::new(service_fs::AppState {
        module_id: module_id.to_string(),
        ledger: fs_ledger,
        audit_ledger: fs_audit_ledger,
    });
    let fs_router = service_fs::router(fs_state);

    let fs_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let fs_addr = fs_listener.local_addr().unwrap();
    let fs_url = format!("http://{fs_addr}");

    let fs_handle = tokio::spawn(async move {
        axum::serve(fs_listener, fs_router).await.unwrap();
    });

    wait_for_ready(format!("{fs_url}/healthz")).await;

    // ── service-people: in-process router with FsClient pointing at fs_url ──
    let people_state = service_people::AppState {
        module_id: module_id.to_string(),
        fs_client: service_people::FsClient::new(fs_url.clone(), module_id),
        people_store: Arc::new(service_people::PeopleStore::new()),
    };
    let people_router = service_people::router(people_state);

    // ── Step 1: POST /mcp tools/call identity.append ──
    let append_req = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "identity.append",
            "arguments": {
                "name": "Alice Anderson",
                "primary_email": "alice@example.com",
                "organisation": "Acme Inc."
            }
        },
        "id": 1
    });
    let append_resp = people_router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&append_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(append_resp.status(), StatusCode::OK);
    let append_body = body_json(append_resp.into_body()).await;
    assert!(
        append_body["error"].is_null(),
        "identity.append must not error: {append_body}"
    );
    let cursor = append_body["result"]["cursor"]
        .as_u64()
        .expect("cursor must be a u64");
    assert!(cursor >= 1, "cursor must be ≥ 1, got {cursor}");
    let person_id = append_body["result"]["person_id"]
        .as_str()
        .expect("person_id must be a string")
        .to_string();

    // ── Step 2: GET service-fs /v1/entries?since=0 — verify round-trip ──
    let fs_url_clone = fs_url.clone();
    let module_id_clone = module_id.to_string();
    let entries: serde_json::Value = tokio::task::spawn_blocking(move || {
        let mut resp = ureq::get(&format!("{fs_url_clone}/v1/entries?since=0"))
            .header("X-Foundry-Module-ID", &module_id_clone)
            .call()
            .expect("/v1/entries call must succeed");
        resp.body_mut()
            .read_json::<serde_json::Value>()
            .expect("entries response must be valid JSON")
    })
    .await
    .unwrap();

    let entries_arr = entries["entries"]
        .as_array()
        .expect("entries field must be an array");
    assert_eq!(
        entries_arr.len(),
        1,
        "exactly one entry must be persisted: {entries}"
    );

    let entry = &entries_arr[0];
    assert_eq!(entry["cursor"].as_u64().unwrap(), cursor);
    assert_eq!(entry["payload_id"], person_id);
    assert_eq!(entry["payload"]["id"], person_id);
    assert_eq!(entry["payload"]["name"], "Alice Anderson");
    assert_eq!(entry["payload"]["primary_email"], "alice@example.com");
    assert_eq!(entry["payload"]["organisation"], "Acme Inc.");
    assert!(
        entry["payload"]["created_at"].is_string(),
        "created_at must be present and a string"
    );

    // ── Step 3: POST /mcp tools/call identity.lookup — verify cache ──
    let lookup_req = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "identity.lookup",
            "arguments": {
                "query_type": "email",
                "value": "alice@example.com"
            }
        },
        "id": 2
    });
    let lookup_resp = people_router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&lookup_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(lookup_resp.status(), StatusCode::OK);
    let lookup_body = body_json(lookup_resp.into_body()).await;
    assert!(
        lookup_body["error"].is_null(),
        "identity.lookup must not error: {lookup_body}"
    );
    assert_eq!(lookup_body["result"]["id"], person_id);
    assert_eq!(lookup_body["result"]["name"], "Alice Anderson");
    assert_eq!(lookup_body["result"]["primary_email"], "alice@example.com");

    fs_handle.abort();
}
