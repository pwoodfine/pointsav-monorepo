// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Engine error type and its HTTP response mapping.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// All failures surfaced to the HTTP layer.
#[derive(Debug, thiserror::Error)]
pub enum WikiError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid slug: {0}")]
    InvalidSlug(String),

    #[error("already exists: {0}")]
    AlreadyExists(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl WikiError {
    pub fn status(&self) -> StatusCode {
        match self {
            WikiError::NotFound(_) => StatusCode::NOT_FOUND,
            WikiError::InvalidSlug(_) => StatusCode::BAD_REQUEST,
            WikiError::AlreadyExists(_) => StatusCode::CONFLICT,
            WikiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for WikiError {
    fn into_response(self) -> Response {
        let status = self.status();
        (status, self.to_string()).into_response()
    }
}

/// Convenience alias for handler results.
pub type WikiResult<T> = Result<T, WikiError>;
