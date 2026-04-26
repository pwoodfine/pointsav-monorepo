// SPDX-License-Identifier: Apache-2.0 OR MIT

//! EWS credential reader — consumes a pre-acquired OAuth 2.0 access
//! token from the `AZURE_ACCESS_TOKEN` environment variable.
//!
//! The token is acquired out-of-process (e.g., via
//! `az account get-access-token --resource https://outlook.office365.com`
//! or the operator's token-refresh sidecar). This crate does not run
//! the OAuth handshake inline.
//!
//! Per operator decision 2026-04-25 and the pattern proven in
//! `service-email-egress-ews/egress-ingress/src/main.rs` and
//! `service-email-egress-ews/egress-roster/src/main.rs`.

use std::env;

pub struct EwsCredentials {
    pub access_token: String,
    pub target_user: String,
    /// EWS endpoint URL. Defaults to Exchange Online if not set.
    pub ews_endpoint: String,
}

impl EwsCredentials {
    pub fn from_env() -> Result<Self, String> {
        let access_token = env::var("AZURE_ACCESS_TOKEN")
            .map_err(|_| "AZURE_ACCESS_TOKEN is required".to_string())?;
        let target_user = env::var("EXCHANGE_TARGET_USER")
            .map_err(|_| "EXCHANGE_TARGET_USER is required".to_string())?;
        let ews_endpoint = env::var("EWS_ENDPOINT")
            .unwrap_or_else(|_| "https://outlook.office365.com/EWS/Exchange.asmx".to_string());

        Ok(Self {
            access_token,
            target_user,
            ews_endpoint,
        })
    }
}
