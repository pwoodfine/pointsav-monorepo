// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// End-to-end Ring 1 pipeline test — closes the loop from document
// ingest through the WORM ledger.
//
// Wires a real `service-fs` daemon (axum on an ephemeral localhost port,
// PosixTileLedger over a temp directory) to a `service-input` router
// driven via `tower::ServiceExt::oneshot`. A POST to `/mcp`
// `tools/call` for `document.ingest` flows through FsClient over real
// HTTP into service-fs, then we GET `/v1/entries?since=0` from
// service-fs and assert the ParsedDocument round-trips faithfully.
//
// The test runs on a multi-threaded tokio runtime because
// service-input's FsClient is synchronous (ureq blocking) and is
// invoked from inside an async axum handler — that blocking call needs
// a worker thread distinct from the one driving service-fs's serve
// loop.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::json;
use tower::ServiceExt;

static TMPCTR: AtomicU64 = AtomicU64::new(0);

fn tmpdir() -> PathBuf {
    let n = TMPCTR.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "svc-input-e2e-{}-{}",
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
async fn document_ingest_persists_to_service_fs_and_reads_back() {
    let module_id = "e2e-input-tenant";
    let fs_root = tmpdir();

    // ── service-fs: real HTTP server on ephemeral localhost port ──────
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

    // ── service-input: in-process router with FsClient → fs_url ──────
    let input_state = Arc::new(service_input::http::AppState {
        module_id: module_id.to_string(),
        dispatcher: service_input::Dispatcher::new()
            .with_markdown(Box::new(service_input::MarkdownParser)),
        fs_client: service_input::FsClient::new(fs_url.clone(), module_id),
    });
    let input_router = service_input::http::router(input_state);

    // ── POST /mcp tools/call document.ingest ──────────────────────────
    let md_bytes = b"# Ring 1 Test\n\nThis document exercises the end-to-end ingest path.";
    let bytes_b64 = BASE64.encode(md_bytes);

    let ingest_req = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "document.ingest",
            "arguments": {
                "filename": "ring1-test.md",
                "source_id": "e2e-doc-1",
                "bytes_base64": bytes_b64
            }
        },
        "id": 1
    });

    let ingest_resp = input_router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .header("x-foundry-module-id", module_id)
                .body(Body::from(serde_json::to_vec(&ingest_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(ingest_resp.status(), StatusCode::OK);
    let ingest_body = body_json(ingest_resp.into_body()).await;
    assert!(
        ingest_body["error"].is_null(),
        "document.ingest must not error: {ingest_body}"
    );

    // The result text is a JSON string embedded in result.content[0].text.
    let result_text = ingest_body["result"]["content"][0]["text"]
        .as_str()
        .expect("result.content[0].text must be a string");
    let result: serde_json::Value =
        serde_json::from_str(result_text).expect("result text must be valid JSON");

    let cursor = result["cursor"].as_u64().expect("cursor must be a u64");
    assert!(cursor >= 1, "cursor must be >= 1, got {cursor}");
    assert_eq!(result["source_id"], "e2e-doc-1");
    assert_eq!(result["format"], "Markdown");

    // ── GET service-fs /v1/entries?since=0 — verify round-trip ───────
    let fs_url_clone = fs_url.clone();
    let module_id_str = module_id.to_string();
    let entries: serde_json::Value = tokio::task::spawn_blocking(move || {
        let mut resp = ureq::get(&format!("{fs_url_clone}/v1/entries?since=0"))
            .header("X-Foundry-Module-ID", &module_id_str)
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
    assert_eq!(entry["payload_id"], "e2e-doc-1");
    assert_eq!(entry["payload"]["source_id"], "e2e-doc-1");
    assert_eq!(entry["payload"]["format"], "Markdown");
    assert!(
        entry["payload"]["text"]
            .as_str()
            .map(|t| t.contains("Ring 1 Test"))
            .unwrap_or(false),
        "parsed text must contain the heading: {entry}"
    );
    assert!(
        entry["payload"]["metadata"]["parser"].is_string(),
        "parser metadata must be present: {entry}"
    );

    fs_handle.abort();
}
