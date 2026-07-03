// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Engine error type and its axum response mapping.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum MarketingError {
    #[error("content directory does not exist or is not a directory: {0}")]
    ContentDirMissing(String),

    #[error("page not found: {0}")]
    PageNotFound(String),

    #[error("manifest error in {slug}: {source}")]
    Manifest {
        slug: String,
        #[source]
        source: serde_yaml::Error,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl IntoResponse for MarketingError {
    fn into_response(self) -> Response {
        let status = match &self {
            MarketingError::PageNotFound(_) => StatusCode::NOT_FOUND,
            MarketingError::ContentDirMissing(_) => StatusCode::INTERNAL_SERVER_ERROR,
            MarketingError::Manifest { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            MarketingError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}
