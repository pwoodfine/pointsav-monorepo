// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Round-trip integration test — exercises the full HTTP + ledger stack.
//!
//! Verifies that a payload appended via POST /v1/append is returned
//! unchanged by GET /v1/entries, and that every field in the envelope
//! is present and correct. Does not spin up a TCP listener; uses
//! `tower::ServiceExt::oneshot` to drive the axum router directly.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

static TMPCTR: AtomicU64 = AtomicU64::new(0);

fn tmpdir() -> PathBuf {
    let n = TMPCTR.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "svc-fs-rt-test-{}-{}",
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

/// POST /v1/append → GET /v1/entries: the returned payload must equal
/// what was appended, cursor must be monotonically assigned, and
/// next_cursor in the response must equal the last entry's cursor.
#[tokio::test]
async fn append_then_entries_returns_payload() {
    let root = tmpdir();
    let module_id = "rt-tenant";

    let ledger = service_fs::posix_tile_open(&root, module_id, None::<&std::path::Path>).unwrap();
    let audit_ledger =
        service_fs::posix_tile_open(root.join(module_id), "audit-log", None::<&std::path::Path>)
            .unwrap();

    let state = Arc::new(service_fs::AppState {
        module_id: module_id.to_string(),
        ledger,
        audit_ledger,
    });
    let app = service_fs::router(state);

    // --- Append ---
    let payload = serde_json::json!({
        "content": "hello integration test",
        "version": 1
    });
    let append_body = serde_json::json!({
        "payload_id": "doc-001",
        "payload": payload
    });

    let append_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/append")
                .header("content-type", "application/json")
                .header("x-foundry-module-id", module_id)
                .body(Body::from(serde_json::to_vec(&append_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(append_resp.status(), StatusCode::OK);
    let append_json = body_json(append_resp.into_body()).await;
    let cursor: u64 = append_json["cursor"].as_u64().unwrap();
    assert_eq!(append_json["payload_id"], "doc-001");
    assert!(cursor >= 1, "cursor must be at least 1");

    // --- Read back ---
    let entries_resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/entries?since=0")
                .header("x-foundry-module-id", module_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(entries_resp.status(), StatusCode::OK);
    let entries_json = body_json(entries_resp.into_body()).await;

    assert_eq!(entries_json["module_id"], module_id);
    let entries = entries_json["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "exactly one entry must be returned");

    let entry = &entries[0];
    assert_eq!(entry["cursor"], cursor, "cursor must round-trip unchanged");
    assert_eq!(entry["payload_id"], "doc-001");
    assert_eq!(
        entry["payload"], payload,
        "payload must round-trip byte-for-byte"
    );

    let next_cursor = entries_json["next_cursor"].as_u64().unwrap();
    assert_eq!(next_cursor, cursor, "next_cursor must equal the last entry's cursor");
}

/// Two appends then GET since=<first_cursor>: only the second entry
/// is returned, confirming the `since` filter excludes the boundary.
#[tokio::test]
async fn entries_since_excludes_boundary() {
    let root = tmpdir();
    let module_id = "rt-tenant-2";

    let ledger = service_fs::posix_tile_open(&root, module_id, None::<&std::path::Path>).unwrap();
    let audit_ledger =
        service_fs::posix_tile_open(root.join(module_id), "audit-log", None::<&std::path::Path>)
            .unwrap();

    let state = Arc::new(service_fs::AppState {
        module_id: module_id.to_string(),
        ledger,
        audit_ledger,
    });
    let app = service_fs::router(state);

    let make_append = |pid: &str, val: u64| -> Request<Body> {
        let b = serde_json::json!({ "payload_id": pid, "payload": {"v": val} });
        Request::builder()
            .method("POST")
            .uri("/v1/append")
            .header("content-type", "application/json")
            .header("x-foundry-module-id", module_id)
            .body(Body::from(serde_json::to_vec(&b).unwrap()))
            .unwrap()
    };

    let r1 = app.clone().oneshot(make_append("a", 1)).await.unwrap();
    let c1: u64 = body_json(r1.into_body()).await["cursor"].as_u64().unwrap();

    app.clone().oneshot(make_append("b", 2)).await.unwrap();

    let entries_resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/entries?since={c1}"))
                .header("x-foundry-module-id", module_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(entries_resp.status(), StatusCode::OK);
    let entries_json = body_json(entries_resp.into_body()).await;
    let entries = entries_json["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "only second entry is returned when since=c1");
    assert_eq!(entries[0]["payload_id"], "b");
}
