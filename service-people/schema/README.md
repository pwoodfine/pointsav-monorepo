# Identity Ledger Schema

Canonical schema definitions for the service-people identity ledger. This directory is the source of truth for Ring 1 boundary ingest contract.

## Files

- **`identity-record.schema.json`** — JSON Schema (draft-07) defining the canonical person record format. JSONL serialization; one record per identity keyed by UUIDv5.
- **`DESIGN.md`** — Design rationale, patterns, and constraints. Read this to understand why the schema is shaped the way it is.

## Quick Start

An identity record looks like:

```json
{
  "identity_id": "a1b2c3d4-e5f6-5789-a0b1-c2d3e4f5a6b7",
  "primary_email": "jane@example.com",
  "addresses": {
    "emails": ["jane@example.com", "jane.doe@personal.com"],
    "phones": ["+1-555-0123"],
    "endpoints": [{"type": "slack", "value": "U123456789"}]
  },
  "roles": [
    {
      "role_id": "r-uuid4",
      "role_type": "employee",
      "organization": "PointSav",
      "effective_at": "2025-01-01T00:00:00Z",
      "expires_at": null,
      "attributes": {"title": "Engineer", "department": "Core"}
    }
  ],
  "canonical_name": "Jane Doe",
  "metadata": {
    "source_id": "user-import-batch-20250101",
    "verification_level": "manually_reviewed"
  },
  "created_at": "2026-04-25T14:30:00Z",
  "updated_at": "2026-04-26T09:15:00Z"
}
```

## Key Constraints

- **UUIDv5 canonical key:** deterministic, no AI, no fuzzy matching (ADR-07)
- **Append-only:** immutable records, audit trail preserved
- **No silent merges:** ambiguity surfaces to operator (ADR-10)
- **Email-anchored identity:** primary email derives the identity_id and cannot change without creating a new identity (future role-continuity links planned)

## Downstream Consumers

Ring 2 and Ring 3 services depend on this schema:

- **service-extraction** (Ring 2): reads identities via MCP, enriches with entity resolution results
- **service-content** (Ring 2): reads identities to associate documents with people
- **service-totebox-egress** (Ring 2+3): reads identities to route completions to the right person

## Versioning

Current version: **0.1.0**

Breaking changes are major bumps only (v0.x → v1.0). Minor additions (new enum values, new optional fields) are within v0.x.

See `DESIGN.md` for future version roadmap.
