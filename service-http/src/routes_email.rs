use std::env;
use std::fs;
use serde::Serialize;
use warp::Filter;

/// Routes bridging to service-email local vaults

#[derive(Serialize)]
struct VaultMetric {
    target: String,
    asset_count: usize,
    status: String,
}

pub fn api_filters() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    get_spool_status().or(get_vault_metrics())
}

// Dynamically resolve the Totebox boundary from the environment, defaulting to the GCP production path
fn get_totebox_root() -> String {
    env::var("TOTEBOX_ROOT").unwrap_or_else(|_| "/opt/woodfine/cluster-totebox-personnel".to_string())
}

// Physical directory traversal
fn count_assets_in_directory(path: &str) -> usize {
    match fs::read_dir(path) {
        Ok(entries) => entries.filter_map(Result::ok).filter(|e| e.path().is_file()).count(),
        Err(_) => 0,
    }
}

// GET /api/v1/email/spool
fn get_spool_status() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "v1" / "email" / "spool")
        .and(warp::get())
        .map(|| {
            let path = format!("{}/service-email/personnel-maildir/new", get_totebox_root());
            let count = count_assets_in_directory(&path);
            let response = VaultMetric {
                target: "Sovereign Ingestion Spool (/new)".to_string(),
                asset_count: count,
                status: if count > 0 { "Active Processing Required".to_string() } else { "Cleared".to_string() },
            };
            warp::reply::json(&response)
        })
}

// GET /api/v1/email/vault
fn get_vault_metrics() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "v1" / "email" / "vault")
        .and(warp::get())
        .map(|| {
            let path = format!("{}/service-email/personnel-maildir/cur", get_totebox_root());
            let count = count_assets_in_directory(&path);
            let response = VaultMetric {
                target: "Immutable Cold Vault (/cur)".to_string(),
                asset_count: count,
                status: "Secured".to_string(),
            };
            warp::reply::json(&response)
        })
}
