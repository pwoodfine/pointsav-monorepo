use std::collections::HashMap;

pub struct TenantConfig {
    pub tenant_id: String,
    pub max_vms: u32,
    pub max_ram_mb: u64,
}

pub struct TenantRegistry {
    tenants: HashMap<String, TenantConfig>,
    /// Maps opaque bearer token → tenant_id.
    /// Empty when TOKEN_MAP is not set (fallback: bearer IS tenant_id).
    tokens: HashMap<String, String>,
}

impl TenantRegistry {
    /// Build from environment variables.
    ///
    /// TENANT_IDS=alice,bob
    /// TENANT_ALICE_MAX_VMS=3          (optional; default 5)
    /// TENANT_ALICE_MAX_RAM_MB=4096    (optional; default 8192)
    ///
    /// TOKEN_MAP=token1:alice,token2:bob  (optional; if absent, bearer == tenant_id — insecure)
    pub fn from_env() -> Self {
        let ids_raw = std::env::var("TENANT_IDS").unwrap_or_default();
        let mut tenants = HashMap::new();

        for id in ids_raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let upper = id.to_uppercase();
            let max_vms = std::env::var(format!("TENANT_{upper}_MAX_VMS"))
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5u32);
            let max_ram_mb = std::env::var(format!("TENANT_{upper}_MAX_RAM_MB"))
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8192u64);

            tenants.insert(
                id.to_string(),
                TenantConfig {
                    tenant_id: id.to_string(),
                    max_vms,
                    max_ram_mb,
                },
            );
            tracing::info!(tenant_id = id, max_vms, max_ram_mb, "registered tenant");
        }

        // Parse TOKEN_MAP=token1:alice,token2:bob
        let tokens = match std::env::var("TOKEN_MAP") {
            Ok(raw) if !raw.trim().is_empty() => {
                let mut map = HashMap::new();
                for pair in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    if let Some((token, tid)) = pair.split_once(':') {
                        let token = token.trim().to_string();
                        let tid = tid.trim().to_string();
                        tracing::info!(tenant_id = %tid, "registered opaque bearer token");
                        map.insert(token, tid);
                    } else {
                        tracing::warn!(
                            entry = pair,
                            "TOKEN_MAP entry missing ':' separator — skipped"
                        );
                    }
                }
                map
            }
            _ => {
                tracing::warn!(
                    "TOKEN_MAP not set — bearer token equals tenant_id (insecure fallback mode); \
                     set TOKEN_MAP=<opaque_token>:<tenant_id>,... to harden"
                );
                HashMap::new()
            }
        };

        TenantRegistry { tenants, tokens }
    }

    /// Validate a Bearer token and return the matching TenantConfig.
    ///
    /// When TOKEN_MAP is set, the bearer is an opaque key mapped to a tenant_id.
    /// When TOKEN_MAP is absent (insecure fallback), the bearer IS the tenant_id.
    pub fn authenticate<'a>(&'a self, bearer: &str) -> Option<&'a TenantConfig> {
        if self.tokens.is_empty() {
            // Insecure fallback: bearer == tenant_id
            self.tenants.get(bearer)
        } else {
            // Opaque token lookup: resolve token → tenant_id → TenantConfig
            let tenant_id = self.tokens.get(bearer)?;
            self.tenants.get(tenant_id)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tenants.is_empty()
    }
}

/// Extract the bearer token from an Authorization header value.
/// Accepts "Bearer <token>" or just "<token>".
pub fn extract_bearer(auth_header: &str) -> &str {
    auth_header
        .strip_prefix("Bearer ")
        .unwrap_or(auth_header)
        .trim()
}
