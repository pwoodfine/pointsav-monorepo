use std::env;
use std::fs;
use serde::{Deserialize, Serialize};
use warp::Filter;

/// Routes bridging to service-content ledgers

#[derive(Serialize)]
struct ContentMetric {
    pending_drafts: usize,
    verified_ledgers: usize,
    throttle_remaining: usize,
}

fn get_totebox_root() -> String {
    env::var("TOTEBOX_ROOT").unwrap_or_else(|_| "/opt/woodfine/cluster-totebox-personnel".to_string())
}

fn count_markdown_files(path: &str) -> usize {
    match fs::read_dir(path) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
            .count(),
        Err(_) => 0,
    }
}

pub fn api_filters() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    get_content_status().or(post_verify_entity())
}

// GET /api/v1/content/status
fn get_content_status() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "v1" / "content" / "status")
        .and(warp::get())
        .map(|| {
            let root = get_totebox_root();
            let pending = count_markdown_files(&format!("{}/service-content/knowledge-graph", root));
            let verified = count_markdown_files(&format!("{}/service-content/verified-ledger", root));
            
            let response = ContentMetric {
                pending_drafts: pending,
                verified_ledgers: verified,
                throttle_remaining: 10, // Mathematical constant per 24H cycle
            };
            warp::reply::json(&response)
        })
}

#[derive(Deserialize)]
struct VerifyRequest {
    filename: String,
}

// POST /api/v1/content/verify
fn post_verify_entity() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "v1" / "content" / "verify")
        .and(warp::post())
        .and(warp::body::json())
        .map(|req: VerifyRequest| {
            // Execution stub for the Self-Healing Wiki trigger
            warp::reply::json(&format!("[SYSTEM] Verification signal locked for: {}", req.filename))
        })
}
