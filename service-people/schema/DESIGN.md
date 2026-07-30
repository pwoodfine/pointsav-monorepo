# Identity Ledger Schema Design

> Last updated: 2026-04-27  
> Version: 0.1.0  
> Status: Ring 1 canonical definition

---

## Overview

The Identity Ledger is a JSONL-based, append-only record of canonical person identities in the Foundry ecosystem. Designed per the Compounding Substrate pattern (Three-Ring Architecture §1 — Boundary Ingest), service-people publishes this schema so that Ring 2 and Ring 3 services can depend on stable field names and deterministic identity resolution.

---

## Design Principles

### 1. UUIDv5 as Canonical Key

The `identity_id` is derived deterministically from the primary email address using UUIDv5 with DNS namespace:

```rust
Uuid::new_v5(&Uuid::NAMESPACE_DNS, email.as_bytes())
```

**Rationale:**
- Deterministic: same email always produces the same UUID on any node
- No AI, no probabilistic matching (ADR-07 compliance)
- Email is the universal identifier in the founding cohort (PointSav + Woodfine)
- UUIDv5 is stable and reproducible; v4 would require state synchronization

**Consequence:** Canonical identity is anchored to email. If an identity's primary email changes, the UUID changes — a different identity. Role/relationship attributes track the change via role-claims ledger (future work, not in this schema).

### 2. Two-Ledger Pattern (Anchor + Claim)

Inherited from `people-acs-engine/`:

- **Anchor records** (not in this schema, but in sibling `anchors.jsonl`): One immutable record per unique email address, recording UUIDv5 derivation and source.
- **Identity records** (this schema, in `ledger_personnel.jsonl`): Canonical person record with all known addresses, roles, and attributes.

**Rationale:**
- Anchors preserve audit trail of UUID derivation
- Identity records hold current state, enabling reads without parsing anchors
- Append-only discipline ensures no data loss

### 3. Deterministic Entity Resolution (Ring 1 Zero-AI)

Per ADR-07, Ring 1 must not route structured data through AI. Entity resolution is deterministic:

- Email extraction: regex only (no NLP, no fuzzy matching)
- Email normalization: lowercase, deduplicated
- Unknown/ambiguous identities: surfaced to the operator (no silent merges)

This constraint is reflected in the schema:
- `addresses.emails`: all known emails for this identity, deduplicated
- `primary_email`: single canonical email used to derive `identity_id`
- `metadata.verification_level`: records how confident we are in this assignment

### 4. Multi-Endpoint Communication Addresses

Identity records hold all known communication endpoints:

| Channel | Format | Examples | Notes |
|---------|--------|----------|-------|
| Emails | RFC 5322 (case-insensitive) | `jane@example.com` | Primary + alternates |
| Phones | E.164 (if known) | `+1-555-0123` | No normalization; extracted as-is |
| Endpoints | Typed objects | Slack user_id, Teams handle, Signal contact | Extensible enum |

**Rationale:** Totebox (Ring 2 / Ring 3) may need to reach an identity via multiple channels. Having all known addresses in one record avoids cross-service lookups.

### 5. Roles as Temporal Snapshots

The `roles` array holds role assignments at a point in time. Each role is immutable once written:

- `role_id`: UUIDv4, unique within this identity's role history
- `role_type`: enum (employee, contractor, vendor, customer, partner, other)
- `effective_at` / `expires_at`: temporal scope
- `attributes`: role-specific metadata (title, department, permissions, etc.)

**Rationale:**
- Roles change over time (Jennifer moves from contractor to employee; Peter leaves)
- Immutable snapshots enable audit trail
- Append-only ledger (future `role-claims.jsonl`) will track state transitions

### 6. Module ID Isolation (Futureproof)

Currently, this schema is single-tenant (all PointSav + Woodfine identities in one ledger). Future work may add multi-tenant isolation via `moduleId` prefixing (per Compounding Substrate) or a `tenant_id` field in metadata. This schema does not pre-allocate a field for it — the MCP interface layer will handle multi-tenant routing.

---

## Record Lifecycle

### Creation

When an identity is first discovered (via email extraction, user import, manual onboarding):

1. **Compute** `identity_id` = `Uuid::new_v5(&Uuid::NAMESPACE_DNS, email.as_bytes())`
2. **Create** an anchor record in `anchors.jsonl`
3. **Create** an identity record in `ledger_personnel.jsonl` with minimal fields (primary email, addresses, roles)
4. **Record** `created_at` timestamp and `source_id` (where the identity came from)

### Updates

When new information about an identity arrives (new email, new role, new phone number):

- **Append** a new identity record with updated fields and new `updated_at` timestamp
- Ledger reader must take the latest record per `identity_id` (WORM ledger semantics)
- Future: append to `role-claims.jsonl` for role transitions (not in this schema version)

### Archiving

When an identity must be marked as inactive (employee leaves; spam/invalid email):

- Append a new record with roles marked as `expires_at = now`
- The identity remains queryable but flagged as inactive
- No deletion (append-only discipline)

---

## MCP Interface

`service-people` will expose this schema via MCP (Model Context Protocol):

### Resources

- `identity:///<identity_id>` → Read the canonical record for an identity
- `identity:///by-email/<email>` → Lookup identity by email address (computes UUIDv5, reads record)

### Tools

- `append_identity` (for Ring 1 inbound: Totebox → service-people)
- `resolve_identity` (for Ring 2 / Ring 3: service-content, service-extraction lookup)

MCP tools will handle multi-tenant routing via `moduleId` header (TBD).

---

## Field Validation and Error Cases

### Canonical Key Derivation Errors

If `primary_email` is invalid (malformed email, missing, null):

- **Validation:** reject at append time (HTTP 400 / MCP tool error)
- **Recovery:** operator must correct the record before retry

### Duplicate Identity Detection

If two records arrive with the same `identity_id` but different `primary_email`:

- **Detection:** WORM ledger reader detects anomaly
- **Handling:** operator review required (data corruption? name change? social engineering?)
- **No silent merge:** per ADR-10, ambiguity surfaces to operator

### Multi-Email Handling

If an identity has multiple emails and the `primary_email` changes:

- **Current approach:** append new record with new primary email
- **UUIDv5 consequence:** new primary email → different `identity_id`
- **Future refinement:** role-claims ledger will track "same person, different email" via the identity-continuity mechanism (Doctrine claim #34, not in this version)

---

## Extensibility

The schema is intentionally open in `roles[].attributes` and `metadata`:

```json
"attributes": {
  "type": "object",
  "additionalProperties": true
}
```

This allows:
- Organization-specific fields (Woodfine: `{office_location}`, PointSav: `{github_username}`)
- Future fields without schema migration
- Graceful degradation (consumers ignore unknown keys)

Closed fields (`role_type`, `addresses.endpoints[].type`) use enums for stability.

---

## Schema Versioning

This is `identity-record.schema.json` v0.1.0. Future versions:

- v0.2.0: add `tenant_id` or MCP module routing metadata
- v0.3.0: add `identity_continuity` links (for name/email changes)
- v1.0.0: stabilized and published (no breaking changes within 1.x)

Breaking changes (e.g., removing a required field) are major bumps only.

---

## References

- **Compounding Substrate:** `conventions/compounding-substrate.md` — Three-Ring Architecture, Ring 1 constraints
- **ADR-07:** Structured data never routes through AI (ADR-07 enforcement in Ring 1)
- **ADR-10:** F12 mandatory checkpoint, no bypass (human-in-loop for ambiguity)
- **UUIDv5 RFC:** RFC 4122 — UUIDs and GUIDs
- **E.164 Phone Format:** ITU-T Recommendation E.164
- **JSONL Format:** JSON Lines (http://jsonlines.org/)
