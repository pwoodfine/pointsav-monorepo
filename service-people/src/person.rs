// SPDX-License-Identifier: Apache-2.0 OR MIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Canonical identity record for a person in the Ring 1 Identity Ledger.
///
/// The `id` field is derived deterministically from `primary_email` as
/// UUIDv5(NAMESPACE_DNS, email.to_lowercase()), matching the convention
/// used in `people-acs-engine/` so anchor records from both sources key
/// on the same stable identifier.
///
/// Persisted through `service-fs` via MCP append — never written to disk
/// directly (WORM invariant). ADR-07: no AI in any construction path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Person {
    /// Stable identifier — UUIDv5(NAMESPACE_DNS, primary_email).
    pub id: Uuid,
    /// Display name (e.g. "Jennifer Woodfine").
    pub name: String,
    /// Primary SMTP address; lowercase-normalised.
    pub primary_email: String,
    /// Additional SMTP addresses (aliases, previous addresses); lowercase-normalised.
    pub email_aliases: Vec<String>,
    /// Organisation affiliation, if known.
    pub organisation: Option<String>,
    /// When this record was first created in the ledger.
    pub created_at: DateTime<Utc>,
    /// When this record was last updated.
    pub updated_at: DateTime<Utc>,
}

impl Person {
    /// Construct a new `Person`, deriving the stable ID from `primary_email`.
    pub fn new(name: impl Into<String>, primary_email: impl Into<String>) -> Self {
        let primary_email = primary_email.into().to_lowercase();
        let id = Uuid::new_v5(&Uuid::NAMESPACE_DNS, primary_email.as_bytes());
        let now = Utc::now();
        Self {
            id,
            name: name.into(),
            primary_email,
            email_aliases: Vec::new(),
            organisation: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add an email alias (normalised to lowercase).
    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.email_aliases.push(alias.into().to_lowercase());
        self
    }

    /// Set the organisation affiliation.
    pub fn with_organisation(mut self, org: impl Into<String>) -> Self {
        self.organisation = Some(org.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_derives_deterministic_id_from_email() {
        let a = Person::new("Alice", "Alice@Example.com");
        let b = Person::new("Alice (copy)", "alice@example.com");
        assert_eq!(a.id, b.id, "UUIDv5 must be case-insensitive (email normalised to lowercase)");
    }

    #[test]
    fn new_normalises_email_to_lowercase() {
        let p = Person::new("Bob", "Bob@EXAMPLE.COM");
        assert_eq!(p.primary_email, "bob@example.com");
    }

    #[test]
    fn id_matches_acs_engine_convention() {
        // The people-acs-engine derives: Uuid::new_v5(&Uuid::NAMESPACE_DNS, email.as_bytes())
        // where email is already lowercase. Verify we produce the same value.
        let email = "jennifer@woodfine.ca";
        let expected = Uuid::new_v5(&Uuid::NAMESPACE_DNS, email.as_bytes());
        let p = Person::new("Jennifer Woodfine", email);
        assert_eq!(p.id, expected);
    }

    #[test]
    fn with_alias_appends_lowercase() {
        let p = Person::new("Carol", "carol@example.com")
            .with_alias("Carol@OldDomain.COM")
            .with_alias("c.smith@example.com");
        assert_eq!(p.email_aliases, vec!["carol@olddomain.com", "c.smith@example.com"]);
    }

    #[test]
    fn with_organisation_sets_field() {
        let p = Person::new("Dave", "dave@corp.com").with_organisation("PointSav Digital Systems");
        assert_eq!(p.organisation.as_deref(), Some("PointSav Digital Systems"));
    }

    #[test]
    fn json_round_trip_preserves_all_fields() {
        let original = Person::new("Eve", "eve@example.com")
            .with_alias("eve.backup@example.com")
            .with_organisation("Acme Inc.");

        let json = serde_json::to_string(&original).expect("serialise must succeed");
        let decoded: Person = serde_json::from_str(&json).expect("deserialise must succeed");

        assert_eq!(decoded.id, original.id);
        assert_eq!(decoded.name, original.name);
        assert_eq!(decoded.primary_email, original.primary_email);
        assert_eq!(decoded.email_aliases, original.email_aliases);
        assert_eq!(decoded.organisation, original.organisation);
        assert_eq!(decoded.created_at, original.created_at);
        assert_eq!(decoded.updated_at, original.updated_at);
    }

    #[test]
    fn json_contains_expected_keys() {
        let p = Person::new("Frank", "frank@example.com");
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"id\""));
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"primary_email\""));
        assert!(json.contains("\"email_aliases\""));
        assert!(json.contains("\"organisation\""));
        assert!(json.contains("\"created_at\""));
        assert!(json.contains("\"updated_at\""));
    }

    #[test]
    fn organisation_absent_serialises_as_null() {
        let p = Person::new("Grace", "grace@example.com");
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"organisation\":null"));
    }
}
