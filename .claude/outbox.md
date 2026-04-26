---
mailbox: outbox
owner: task-project-data
location: ~/Foundry/clones/project-data/.claude/
schema: foundry-mailbox-v1
---

# Outbox — Task Claude on project-data cluster

Messages this Task Claude session sends to other roles or to itself
in a later session. Append at session end, before yielding.

Multiple messages separated by `---`. Append-only during session;
move to `outbox-archive.md` after the recipient has acted.

---

---
from: task/project-data (sixth session, Sonnet 4.6, 2026-04-26)
to: master
re: sixth-session completion — all 13 tasks done; next-session recommendations
created: 2026-04-26T00:00:00Z
---

All 13 items from the sixth-session inbox task list landed in commits
on `cluster/project-data`. Working tree clean. Summary by service:

**service-fs** — steps 3–5 of worm-ledger-design.md §5 complete:
- Step 3: Ed25519 checkpoint signing (`ed25519-dalek`; `FS_SIGNING_KEY_HEX`
  env var; signed Checkpoint struct; `/v1/checkpoint` returns signature).
- Step 4: ADR-07 audit-log sub-ledger (second PosixTileLedger at
  `<root>/_audit/log.jsonl`; records timestamp/module_id/cursor/sha256
  per append; no payload content in audit record — ADR-07 clean).
- Round-trip integration test: `tests/ledger_roundtrip.rs` with
  `tempfile` dep; restart-boundary verification.
- MCP interface: `src/mcp.rs`; `/mcp/tools/list` + `/mcp/tools/call`;
  `append_record` + `read_records` tools; JSON-schema descriptors.

**service-input** — full Ring 1 parser stack complete:
- MarkdownParser (pulldown-cmark), DocxParser (docx-rust), XlsxParser
  (calamine); FsClient (reqwest rustls-tls) calling service-fs
  `/v1/append` + `/v1/entries`; MCP interface (`parse_document` tool
  → Dispatcher → FsClient); happy-path PDF fixture
  (`tests/fixtures/minimal.pdf` — 614-byte hand-crafted PDF 1.4 with
  Python-computed xref; oxidize-pdf extracts non-empty text). 30 tests
  pass.

**service-people** — pre-framework inventory complete. Decisions:
`sovereign-acs-engine/` keep + Cargo name rename queued; `spatial-ledger/`
keep; `spatial-crm/` retire-pending; `substrate/` gitignored;
`tools/extract-people-ledger.sh` → `scripts/` (repo-layout.md). Next:
schema definition for canonical person record.

**service-email** — inventory + EWS auth rebase both complete. Rebase
replaces inline OAuth2 + Graph REST with env-var token consumption +
EWS SOAP (FindItem / GetItem-IncludeMimeContent / UpdateItem-IsRead);
`src/graph_client.rs` renamed → `src/ews_client.rs`; Cargo uses
reqwest rustls-tls (avoids openssl-sys); 6 unit tests pass.

**Recommendations for next Task session (Sonnet 4.6 appropriate):**

1. `sovereign-acs-engine/` Cargo `name` rename → `people-acs-engine`
   (single-file edit + cargo check; 10 min).
2. `sovereign-splinter/` rename → `email-splitter` + update
   `scripts/spool-daemon.sh` binary path reference.
3. `ingress-harvester/` + `master-harvester-rs/` formal retirement
   (add CLAUDE.md archive headers; update registry rows).
4. service-people canonical person-record schema design (Rust struct
   in `src/person.rs`; feeds identity-resolution surface).

**Master-tier asks (cannot action from Task layer):**

- `service-fs` systemd unit file at `infrastructure/local-fs/`
  (Master provision item, per triad deployment leg).
- Sigstore Rekor monthly anchoring integration (Invention #7 from
  DOCTRINE §IX; connects service-fs checkpoint output to the Rekor
  anchor path).
- workspace `[members]` re-add for service-fs + service-input
  (blocked on openssl-sys Layer 1 audit resolution — separate Master
  or Root track).

Working tree clean. Branch `cluster/project-data` ahead of main by
all sixth-session commits; no push requested.
