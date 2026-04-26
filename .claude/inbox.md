---
mailbox: inbox
owner: task-project-data
location: ~/Foundry/clones/project-data/.claude/
schema: foundry-mailbox-v1
---

# Inbox — Task Claude on project-data cluster

Messages addressed to whoever opens the next Task Claude session in
this cluster. Read top to bottom at session start. Act on items in
order; archive to `inbox-archive.md` after acting.

If this inbox accumulates more than 5 pending items, post a NOTAM
(per Doctrine §VI) and flag in Master's inbox.

---

## 2026-04-26 — from task-project-data (fifth session) to next session — clean-exit orientation note

priority: medium — read first at session start to orient cold

You are opening the project-data cluster after the fifth session
of 2026-04-26 closed clean. This note exists to orient you in
under two minutes; archive to `inbox-archive.md` once you've
read it.

### State of the branch

- Branch: `cluster/project-data` (verify with
  `git branch --show-current`).
- HEAD at exit: `9f9b824` — service-input PdfParser via
  oxidize-pdf 2.x.
- Recent commit chain (newest first):
  - `9f9b824` service-input PdfParser
  - `10a7dd0` service-fs L1 PosixTileLedger backend (worm-ledger-
    design.md §5 step 2)
  - `8c077c6` fourth-session-end docs
  - `ada358d` service-input parser-dispatcher initial scaffold
  - `1e86047` service-fs L2 LedgerBackend trait extraction
  - `886342f` admin cleanup (mailbox archive + doc-status upgrade)
  - `7d287d3` service-fs SECURITY.md + ARCHITECTURE.md +
    bilingual READMEs
  - `4bfa564` service-fs/RESEARCH.md (~600-line synthesis)
- Working tree: clean (verify with `git status`).
- Push state: nothing pushed. Per the v0.0.10 auto-mode safety
  brief, push is operator-authorised; do not push without
  explicit instruction.

### State of the two Active Ring 1 projects in scope

**service-fs** — Ring 1 WORM Immutable Ledger
- Tokio + axum hosted daemon (Envelope A); per worm-ledger-
  design.md §5 the next steps are 3 (checkpoint signing), 4
  (audit-log sub-ledger), 5 (MCP-server interface layer).
- L2 trait surface (`LedgerBackend`): six methods declared in
  the convention; five implemented (open + append + read_since
  + root + checkpoint + verify_inclusion + verify_consistency).
  The sixth (signing-key parameter on `open`) lands with step 3.
- Backends: InMemoryLedger (test fallback) + PosixTileLedger
  (production; persistent log.jsonl + D4 atomic-write +
  reload-on-open with chain-tamper detection). Both implement
  the full trait.
- 18 unit tests pass (`cargo check + cargo test` standalone).
- service-fs/NEXT.md Right-now: **step 3 checkpoint signing** —
  Ed25519 + signed-note signature population; add
  `FS_SIGNING_KEY` env var; `signing_key` parameter on
  `LedgerBackend::open`; external verification path for
  Customer-side audit per Doctrine claim #28.

**service-input** — Ring 1 generic document ingest
- Cargo crate scaffold (Format/ParsedDocument/Parser
  trait/Dispatcher/detect_format) + first format-specific
  parser (PdfParser via oxidize-pdf 2.x).
- 13 unit tests pass.
- service-input/NEXT.md Right-now: **wire MarkdownParser via
  pulldown-cmark** — pure-text input, no temp-file shim, full
  happy-path testing trivial; proves out the multi-parser
  Dispatcher case before the more complex DOCX + XLSX parsers.

### Where to look first

1. Read `service-fs/NEXT.md` and `service-input/NEXT.md` for the
   per-project pickup items.
2. If anything is unclear about the L2 trait surface or the
   storage design, the authoritative references are:
   - `~/Foundry/conventions/worm-ledger-design.md` (workspace
     v0.1.7 / Doctrine v0.0.3+; ratified 2026-04-26 by Master)
   - `service-fs/RESEARCH.md` (input draft for the convention;
     ~600 lines with full alternatives, sources, and ten
     decisions D1–D10)
   - `service-fs/ARCHITECTURE.md` (per-project four-layer
     overview)
   - `service-fs/SECURITY.md` (per-project compliance posture
     citing SEC 17a-4(f) + eIDAS qualified preservation +
     SOC 2 TSC)
3. Cluster manifest at `.claude/manifest.md` — Master backfilled
   a `triad:` section per Doctrine v0.0.4 with three "leg-pending"
   forward-looking items (Customer GUIDEs, systemd unit at
   `infrastructure/local-fs/`); none of them are immediate Task
   asks.
4. Cluster mailbox at `.claude/outbox.md` — has the fifth-session
   summary to Master (sent 05:15Z) and the fourth-session
   summary still pending Master pickup (sent 11:45Z prior day).
   Both are informational; no asks.

### Pickup order recommendation

The fifth-session outbox to Master flagged three valid options for
the next Task pickup. My recommendation:

1. **service-fs step 3 checkpoint signing** (customer-first
   ordering — sets up the Customer's evidentiary verification
   path; required for the Master-operated monthly Sigstore Rekor
   anchoring per DOCTRINE Invention #7).
2. **service-input MarkdownParser** (low friction; gets the
   multi-parser Dispatcher case proven out).
3. **service-fs step 4 ADR-07 audit-log sub-ledger** (depends on
   step 3 for signed audit checkpoints).

Both 1 and 2 are unblocked. Operator's call on order.

### Hard constraints (still in force)

- ADR-07 zero-AI in Ring 1 (no LLM inference, no embedding,
  no AI-assisted normalisation in any of these crates).
- Per-tenant moduleId isolation (header check today; capability
  isolation in Envelope B / seL4 long-term).
- Append-only invariant at the L2 trait surface and at the L1
  filesystem layer.
- Push only to staging-tier remotes (`origin-staging-j` /
  `origin-staging-p`); never to `origin` (canonical) per
  v0.0.10 auto-mode safety brief — applies even if you are
  operating in auto mode.
- Workspace `[members]` re-add for service-fs + service-input
  is BLOCKED on a pre-existing `openssl-sys` Layer 1 audit
  issue in a sibling member; this is repo-tier work, not
  Task-tier. Do not try to fix it inside this cluster.

After acting on this orientation note, archive it to
`inbox-archive.md` per the mailbox protocol.

---
