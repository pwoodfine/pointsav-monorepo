// SPDX-License-Identifier: Apache-2.0 OR MIT

//! ACS (Anchor-Claim-Source) engine — deterministic email identity extraction.
//!
//! Scans raw text for email addresses and produces immutable Anchor + append-only
//! Claim records per address found. Both record types are written to the WORM
//! ledger via FsClient. ADR-07: zero AI — extraction is regex-only.

use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Immutable anchor — records that `target_uuid` was observed from `anchor_source`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anchor {
    pub target_uuid: String,
    pub anchor_source: String,
    pub timestamp: String,
}

/// Append-only observation — a single attribute claim about a target identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub claim_id: String,
    pub target_uuid: String,
    pub attribute: String,
    pub value: String,
    pub confidence_score: f32,
    pub source_id: String,
    pub timestamp: String,
}

/// Scan `text` for email addresses and return one `(Anchor, Claim)` pair per
/// address found. `source_id` is recorded as the provenance of each claim
/// (e.g. a document name, message-id, or pipeline stage identifier).
///
/// The same email appearing multiple times in `text` produces multiple pairs —
/// callers that want deduplication should deduplicate by `Anchor::target_uuid`.
pub fn scan_text(text: &str, source_id: &str) -> Vec<(Anchor, Claim)> {
    let email_re = Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}").unwrap();
    let mut results = Vec::new();

    for cap in email_re.captures_iter(text) {
        let raw_email = cap[0].to_lowercase();
        let target_uuid = Uuid::new_v5(&Uuid::NAMESPACE_DNS, raw_email.as_bytes()).to_string();
        let timestamp = Utc::now().to_rfc3339();

        let anchor = Anchor {
            target_uuid: target_uuid.clone(),
            anchor_source: raw_email.clone(),
            timestamp: timestamp.clone(),
        };

        let claim = Claim {
            claim_id: Uuid::new_v4().to_string(),
            target_uuid: target_uuid.clone(),
            attribute: "email".to_string(),
            value: raw_email,
            confidence_score: 1.0,
            source_id: source_id.to_string(),
            timestamp,
        };

        results.push((anchor, claim));
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_text_finds_single_email() {
        let results = scan_text("Contact us at hello@example.com for details.", "test-doc");
        assert_eq!(results.len(), 1);

        let (anchor, claim) = &results[0];
        assert_eq!(claim.value, "hello@example.com");
        assert_eq!(claim.attribute, "email");
        assert_eq!(claim.confidence_score, 1.0);
        assert_eq!(claim.source_id, "test-doc");
        assert_eq!(anchor.anchor_source, "hello@example.com");
        assert!(!anchor.target_uuid.is_empty());
        assert_eq!(anchor.target_uuid, claim.target_uuid);
    }

    #[test]
    fn scan_text_finds_multiple_emails() {
        let results = scan_text(
            "From alice@corp.com, copied to bob@corp.com and charlie@other.org",
            "email-thread",
        );
        assert_eq!(results.len(), 3);

        let values: Vec<&str> = results.iter().map(|(_, c)| c.value.as_str()).collect();
        assert!(values.contains(&"alice@corp.com"));
        assert!(values.contains(&"bob@corp.com"));
        assert!(values.contains(&"charlie@other.org"));
    }

    #[test]
    fn scan_text_no_emails_returns_empty() {
        let results = scan_text("No contact information here.", "empty-doc");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn scan_text_uuid_is_deterministic() {
        let r1 = scan_text("reach me at stable@example.com", "src-a");
        let r2 = scan_text("also at stable@example.com please", "src-b");
        assert_eq!(r1[0].0.target_uuid, r2[0].0.target_uuid);
    }

    #[test]
    fn scan_text_normalises_email_to_lowercase() {
        let results = scan_text("Email: UPPER@EXAMPLE.COM", "src");
        assert_eq!(results[0].1.value, "upper@example.com");
    }

    #[test]
    fn scan_text_claim_ids_are_unique_across_calls() {
        let r1 = scan_text("a@b.com", "s");
        let r2 = scan_text("a@b.com", "s");
        assert_ne!(r1[0].1.claim_id, r2[0].1.claim_id);
    }
}
