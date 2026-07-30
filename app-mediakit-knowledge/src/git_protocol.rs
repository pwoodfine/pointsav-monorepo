use crate::server::AppState;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

#[derive(Deserialize)]
pub struct InfoRefsQuery {
    pub service: String,
}

pub async fn info_refs(
    State(state): State<Arc<AppState>>,
    Path(tenant): Path<String>,
    Query(params): Query<InfoRefsQuery>,
) -> Response {
    tracing::info!(tenant = %tenant, service = %params.service, "git info/refs requested");
    if params.service != "git-upload-pack" {
        return (StatusCode::BAD_REQUEST, "Only git-upload-pack is supported").into_response();
    }

    // Validate tenant
    if tenant != state.git_tenant && format!("{}.git", state.git_tenant) != tenant {
        return (StatusCode::NOT_FOUND, "Tenant not found").into_response();
    }

    let mut cmd = tokio::process::Command::new("git");
    cmd.args(["upload-pack", "--stateless-rpc", "--advertise-refs"])
        .arg(state.primary_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = match cmd.output().await {
        Ok(o) => o,
        Err(e) => {
            tracing::error!(error = %e, "failed to run git upload-pack --advertise-refs");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run git upload-pack: {e}"),
            )
                .into_response();
        }
    };

    let mut body = Vec::new();
    body.extend_from_slice(b"001e# service=git-upload-pack\n0000");
    body.extend_from_slice(&output.stdout);

    tracing::debug!("git info/refs advertisement sent");
    (
        [
            (
                header::CONTENT_TYPE,
                "application/x-git-upload-pack-advertisement",
            ),
            (
                header::CACHE_CONTROL,
                "no-cache, max-age=0, must-revalidate",
            ),
        ],
        body,
    )
        .into_response()
}

pub async fn upload_pack(
    State(state): State<Arc<AppState>>,
    Path(tenant): Path<String>,
    body: Bytes,
) -> Response {
    tracing::info!(tenant = %tenant, body_len = body.len(), "git upload-pack requested");
    // Validate tenant
    if tenant != state.git_tenant && format!("{}.git", state.git_tenant) != tenant {
        return (StatusCode::NOT_FOUND, "Tenant not found").into_response();
    }

    let mut child = match tokio::process::Command::new("git")
        .args(["upload-pack", "--stateless-rpc"])
        .arg(state.primary_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "failed to spawn git upload-pack --stateless-rpc");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to spawn git upload-pack: {e}"),
            )
                .into_response();
        }
    };

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    tokio::spawn(async move {
        if let Err(e) = stdin.write_all(&body).await {
            tracing::error!(error = %e, "failed to write body to git upload-pack stdin");
        }
        let _ = stdin.flush().await;
        let _ = stdin.shutdown().await;
    });

    let reader = tokio_util::io::ReaderStream::new(stdout);
    let body = axum::body::Body::from_stream(reader);

    tracing::debug!("git upload-pack response stream established");
    (
        [(header::CONTENT_TYPE, "application/x-git-upload-pack-result")],
        body,
    )
        .into_response()
}
