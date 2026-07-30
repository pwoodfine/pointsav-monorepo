use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use serde::Serialize;
use warp::Filter;

/// Routes bridging to service-people identity masses

#[derive(Serialize)]
struct IdentityMetric {
    total_anchors: usize,
    total_claims: usize,
}

fn get_totebox_root() -> String {
    env::var("TOTEBOX_ROOT").unwrap_or_else(|_| "/opt/woodfine/cluster-totebox-personnel".to_string())
}

// Physically count lines in the append-only JSONL ledgers
fn count_lines(path: &str) -> usize {
    match File::open(path) {
        Ok(file) => BufReader::new(file).lines().count(),
        Err(_) => 0,
    }
}

pub fn api_filters() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    get_identity_metrics()
}

// GET /api/v1/people/metrics
fn get_identity_metrics() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "v1" / "people" / "metrics")
        .and(warp::get())
        .map(|| {
            let root = get_totebox_root();
            let anchors = count_lines(&format!("{}/service-people/substrate/anchors.jsonl", root));
            let claims = count_lines(&format!("{}/service-people/substrate/claims.jsonl", root));
            
            let response = IdentityMetric {
                total_anchors: anchors,
                total_claims: claims,
            };
            warp::reply::json(&response)
        })
}
