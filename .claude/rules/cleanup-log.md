# Cleanup Log — pointsav-monorepo

Living record of in-flight cleanup work, open questions, and decisions made during active development. This file is read at session start and updated at session end when meaningful cleanup occurs. Maintained in-repo so the history travels with the code.

---

## How this file is maintained

- **Read at session start.** Claude Code reads this file at the start of every session (per the instruction in `CLAUDE.md`). The tables below reflect the current state of in-flight work. Apply the guidance before touching any related files.
- **Update at session end.** When a session includes meaningful cleanup — renames across multiple files, deprecated code removal, resolving an open question, surfacing a new one — append a dated entry to the top of the **Session entries** section at the bottom of this file.
- **Do not log trivial edits.** Single-file typo fixes, comment tweaks, or routine formatting changes do not belong here. This log is a record of decisions, not of every keystroke.
- **Commit each update with the code changes it describes.** The log and the work it documents travel together through git history.

---

## Interpreting build signals during cleanup

Until the workspace `Cargo.toml` is unified (see Layer 1 audit findings), `cargo build --workspace` and `cargo check` at the repo root only exercise the 8 declared members. The other ~70 crates are not covered by workspace-level commands. When making changes to any crate outside the declared members, run `cargo check` inside that crate's directory specifically. Do not rely on workspace-root build signals to confirm correctness across the full repo. This caveat lifts when the workspace is unified.

---

## Active legacy-to-canonical renames

These substitutions are known and in progress. Canonical names are from the Nomenclature Matrix. When the last occurrence of a legacy name is removed from the repo, move the row to the **Completed migrations** section with the date of completion.

| Legacy | Canonical | Status | Notes |
|---|---|---|---|
| `service-llm` | `service-slm` | Documentation-only inconsistency | Code references are correct. Legacy appearances in docs should be read as `service-slm`. |
| `cluster-totebox-real-property` | `cluster-totebox-property` | In flight | Appears in older deployment manifests and doc references. |
| `os-interface`, `os-integration` | `os-orchestration` | In flight | Legacy names predate the current three-layer stack nomenclature. |
| `RealPropertyArchive` | `PropertyArchive` | In flight | Appears in older archive-type documentation and possibly in legacy code comments. |

---

## Deprecations — flag and remove

Names no longer in use. Any occurrence in the repo should be flagged and removed. If a removal blocks something active, surface it — do not leave the legacy name in place silently.

| Name | Status | Notes |
|---|---|---|
| `fleet-command-authority` | Deprecated — remove | Node no longer in use. Should not appear in any current deployment manifest, build script, or documentation. |

---

## Intentional exceptions — do not migrate

Items that may look like candidates for cleanup but are intentionally preserved as-is. Do not "fix" these without confirmation.

| Item | Rationale |
|---|---|
| `cluster-totebox-personnel-1` and other numbered personnel instances | Exist locally but intentionally absent from GitHub and the MEMO. Not a naming error. Do not flag as legacy. |
| Two ConsoleOS operating patterns (multi-service `node-console-operator` and single-service nodes) | Both patterns are valid. The MEMO documents `node-console-operator` only, by design, to keep official documentation clean. Do not flag the single-service pattern as an inconsistency. |

---

## Open questions

Pending confirmations that affect how Claude should describe or reason about parts of the system. Do not invent values for these. If a task requires an answer, stop and surface the question.

| Question | Current handling |
|---|---|
| Verification Surveyor daily throttle number | Under operational review. Do not cite a specific number. Refer to it as "a system-enforced daily limit" until confirmed in a future MEMO version. **Code reference (2026-04-23):** `app-console-content/scripts/surveyor.py` hard-codes `MAX_DAILY_VERIFICATIONS = 10`; whether this value is authoritative or drift is the pending decision. |
| User Guide language on Sovereign Data Foundation | The User Guide contains language treating the Foundation as a current equity holder and active auditor. Requires a language review pass before any User Guide content is reused in public-facing materials. Flag any passage that describes the Foundation as current or active. |
| `service-search` inclusion in the next MEMO | Confirmed for inclusion in the next MEMO version. Treat as canonical in code; note the doc catch-up is pending. |
| Is the per-crate independent workspace pattern intentional (some crates meant to be extractable and published separately) or accidental drift? | Pending decision — do not act on related findings until answered. |
| Are `app-console-*` and `app-network-*` directories without `Cargo.toml` intentional scaffolding for planned work, or abandoned attempts? | Pending decision — do not act on related findings until answered. |
| Should the doubly-nested `service-email-egress-{ews,imap}` structure be flattened, or does the nesting reflect a real protocol-implementation hierarchy? | Pending decision — do not act on related findings until answered. |
| What is `discovery-queue` — runtime data that should be gitignored, reference data that belongs elsewhere, or a misplaced crate? | Pending decision — do not act on related findings until answered. |
| ~~Does `vendors-maxmind` (containing a GeoLite2 database, not code) belong as a `vendor-*` crate at all, or should it move to a non-workspace data directory?~~ | **Answered 2026-04-23:** non-workspace data directory. Moved to `app-mediakit-telemetry/assets/` (matching the authoritative target path already documented in the vendor's README). `vendor-*` crate framing rejected: the directory contained only data, no code. |

---

## Completed migrations

Migrations fully resolved in the repo. Moved here from **Active legacy-to-canonical renames** when the last occurrence of the legacy name is removed. Empty for now.

| Legacy | Canonical | Closed | Notes |
|---|---|---|---|
| `service-parser` | `service-extraction` | 2026-04-23 | Legacy-era scaffold containing only a README that described an AI-routing architecture since superseded by `service-extraction`'s deterministic Parser-Combinators approach. Zero runtime references, never a workspace member, one commit in history. No code or data to recycle into `service-extraction`; README deleted without migration. |
| `pointsav-pty-bridge` | `service-pty-bridge` | 2026-04-23 | Prefix-violation defect flagged in 2026-04-18 audit (brand prefix `pointsav-` not one of the seven canonical prefixes). Canonical target `service-pty-bridge` fits the daemon runtime role. Working Rust crate with one source file; directory renamed via `git mv`, `Cargo.toml` `name` field updated in the same commit. Not a workspace member, zero external import references, no callers needed updating. |
| `tool-cognitive-forge` + `service-slm/cognitive-forge` | `service-slm/router-trainer/` + `service-slm/router/` | 2026-04-23 | Closes the last rename-series item and removes the "Cognitive Forge" Do-Not-Use term in one commit. The Rust runtime sub-crate at `service-slm/cognitive-forge/` renamed to `service-slm/router/` (Cargo.toml `name` field + `main.rs` usage string updated). The Python distillation workflow at `tool-cognitive-forge/` moved in to `service-slm/router-trainer/`, joining the runtime as producer/consumer pair. Rationale for split naming: the runtime is a router (of messages to service handlers); the trainer distils knowledge to produce the routing model. Inside `router-trainer/`, `distill_knowledge.py` moved from a non-canonical `src/` into `scripts/` alongside `ignite_teacher.sh`. Three binary/log files untracked from Git and covered by new `.gitignore` patterns (still physically present at new paths for the Python workflow): 35 MB `engine/llamafile`, 22 KB `engine/engine.log`, 89 B `llama.log`. The 15 MB `engine/weights/qwen2.5-coder-1.5b.gguf` was already covered by the existing `**/weights/*` + `*.gguf` patterns — no new ignore needed. Git history retains all blobs; shrinking history is separate `git-filter-repo` work. Registry: `tool-cognitive-forge` row removed; Scaffold-coded 54 → 53, Total 98 → 97. `llama.log` surfaced earlier in this session is closed by this commit. |
| `vendors-maxmind` | `app-mediakit-telemetry/assets/` | 2026-04-23 | Not a rename but a reclassification: the `vendors-maxmind` directory was a data container holding `GeoLite2-City.mmdb` + READMEs, no code. The vendor's own README already named `app-mediakit-telemetry/assets/` as the intended location — the monorepo had never realised that path. Moved the `.mmdb` + READMEs into their documented target; deleted the empty `vendors-maxmind/` directory. Monorepo `README.md` line 151 and `USER_GUIDE_2026-03-30_V2.md` line 902 updated to the new path. `repo-layout.md` extended to name `assets/` as a conventional project subfolder. Python script reference in `app-mediakit-telemetry/scripts/generic-omni-matrix-engine.py` left unchanged — it reads a deployment-side path relative to CWD, not the monorepo-side path. Separate `.mmdb` → build-time-fetch task remains open under Structural defects. |

---

## Session entries

Newest on top. Append a dated block when a session includes meaningful cleanup work. Format:

```
## YYYY-MM-DD
- What changed (files touched, counts, rationale)
- What was left pending and why
- New open questions surfaced
```

---

## 2026-04-26 (third session — research-only, no code changes)

- **Research synthesis written for service-fs storage architecture.**
  Operator asked 2026-04-26 for deep cross-industry research with
  leapfrog-2030 framing on the question of `service-fs` long-term
  storage design, given (a) the MEMO §6.3 WORM legal-compliance
  language, (b) the MEMO §7 trajectory toward seL4 unikernel native
  + moonshot-database capability-aware persistence, (c) the
  Linux/BSD wrapper for hosts where seL4 cannot boot natively.
  ~600-line synthesis committed at `service-fs/RESEARCH.md` (this
  commit).
- **What the synthesis proposes.** A four-layer stack — L1 tile
  storage (C2SP tlog-tiles per RFC 9162 v2 / Trillian-Tessera /
  Sigstore Rekor v2), L2 WORM Ledger Rust trait
  (open/append/read_since/checkpoint/verify_*), L3 wire protocol
  (axum HTTP + MCP layered), L4 monthly Sigstore Rekor anchoring
  (already DOCTRINE Invention #7). The cross-cutting design idea:
  same tile format works on POSIX storage today (Linux/BSD daemon)
  and through capability-mediated `moonshot-database` IPC long-term
  (seL4 Microkit unikernel) — wire protocol is identical across
  envelopes, storage primitive survives the seL4 transition.
- **Industry standards surveyed.** SEC Rule 17a-4(f) (2022 amendment
  effective 2023-05-03 — adds Audit-Trail alternative to WORM, but
  WORM path is cleaner for service-fs); eIDAS qualified preservation
  service (EU 2025/1946 in force 2026-01-06 + ETSI EN 319 401
  v3.2.1 + CEN TS 18170:2025 — long-term integrity "irrespective
  of future technological changes" matches Pillar 2); SOC 2 TSC
  CC6/CC7/PI1/PI4 (per-tenant access, change detection, processing
  integrity); DARP confirmed Foundry-internal not regulatory
  (DOCTRINE.md line 462).
- **Verified that key Foundry sources are coherent on this question.**
  MEMO §6.3 calls service-fs WORM-compliant; MEMO §7 lists
  vendor-sel4-kernel as Legacy → moonshot-kernel and Sled →
  moonshot-database (capability-aware) as the long-term substrate;
  conventions/three-ring-architecture.md + zero-container-runtime.md
  fix today's deployment shape; DOCTRINE §IX SOC 2/DARP posture +
  Invention #7 Sigstore Rekor anchoring give the audit-anchoring
  substrate. The seL4 unikernel target is real, not aspirational —
  Microkit 1.3.0 (rewritten in Rust) is the static-system framework,
  rust-sel4 + sel4-microkit are official runtime crates.
- **Key synthesis claim.** The same tile format used internally for
  service-fs's per-tenant ledger IS the same tile format Sigstore
  Rekor v2 uses externally. Foundry's monthly anchor bundle
  (Invention #7) becomes a direct integration rather than a separate
  format conversion. Customer Totebox tile checkpoints flow into the
  same Rekor anchoring path with zero new format work — extends
  Invention #7 from a Vendor audit-posture feature to a Customer
  evidentiary feature at zero marginal complexity.
- **Workspace-tier convention proposed.** Outboxed to Master under
  subject `worm-ledger-design-convention-proposal`. The proposal:
  the design lands at `~/Foundry/conventions/worm-ledger-design.md`
  (Master tier per §11 action matrix) rather than baking into
  service-fs alone, because the same WORM-ledger primitive will be
  useful for any future Ring 1 producer or audit sub-ledger.
- **Ten ratification decisions surfaced (D1–D10).** D1 (adopt C2SP
  tlog-tiles), D2 (adopt C2SP signed-note), D3 (SHA-256 + algorithm-
  agility), D4 (write-rename + 0o444 today, chattr +i later), D5
  (Foundry workspace witnesses every Customer Totebox by default), D6
  (monthly anchoring cadence unchanged), D7 (moonshot-database swap
  when ready, POSIX backend retained as Envelope A fallback), D8
  (per-call audit granularity), D9 (dual anchoring — Customer-key +
  Foundry-key), D10 (workspace re-add deferred separately).
- **service-fs/NEXT.md Right-now updated.** Storage swap is now
  PAUSED pending Master ratification of the design convention.
  Implementation roadmap (5 task-tier commits) sketched in
  RESEARCH.md §12 for the next Task Claude session in this cluster.
- **De facto pattern observation: RESEARCH.md is not in
  repo-layout.md's allowed-files list, but appears at the project
  root of `app-console-bim`, `app-orchestration-bim`,
  `app-workplace-bim`, `service-bim` per the registry.** Adding it
  to the repo-layout allowed-files list would close the de facto
  deviation; that's a Root-tier edit and out of Task scope. Flagging
  here so a future Root Claude in the monorepo can codify.
- **Web research tools used.** WebSearch + WebFetch (one-time loads
  via ToolSearch), 7 search queries + 1 successful WebFetch (against
  blog.sigstore.dev/rekor-v2-ga). transparency.dev was a less
  productive WebFetch — page is high-level, not technical
  specification. All cited sources listed in RESEARCH.md §13.
- **Customer-first ordering preserved.** Per Master's prior
  go-ahead (2026-04-26 07:55 inbox), service-input parser-dispatcher
  is still the next Task pickup; this research session is a
  parallel Master-ratification track that doesn't block service-input
  work.

---

## 2026-04-26 (second session)

- **Master ratification actioned in full.** Master's inbox message
  2026-04-26T07:20Z ratified the three decisions surfaced in the
  prior session's outbox `ring1-scaffold-runtime-model-drift`. All
  three actioned this session in three commits.
- **Decision 2 — seL4 scaffold relocated.** Commit `7519390`.
  Four `git mv` renames preserved history:
  - `service-fs/src/main.rs` → `vendor-sel4-fs/src/main.rs`
  - `service-fs/.cargo/config.toml` →
    `vendor-sel4-fs/.cargo/config.toml`
  - `service-fs/Cargo.toml` → `vendor-sel4-fs/Cargo.toml` (package
    name updated in transit; description rewritten to cite the
    relocation rationale)
  - `service-fs/Cargo.lock` → `vendor-sel4-fs/Cargo.lock`
  Created bilingual READMEs at `vendor-sel4-fs/README.md` +
  `vendor-sel4-fs/README.es.md` per CLAUDE.md §6 (vendor-tier
  bilingual). Added registry row for `vendor-sel4-fs` in the
  Vendor section between `vendor-phi3-mini` and
  `vendor-sel4-kernel` as Reserved-folder. Service-fs registry
  row updated to record the relocation.
- **Decision 1 — service-fs Tokio MCP-server skeleton.** Commit
  `af73232`. New contents:
  - `Cargo.toml` (tokio + axum 0.7 + serde + tracing + anyhow);
    package version reset 1.0.1 → 0.1.0 (the 1.0.1 stream
    belonged to the relocated bare-metal scaffold; this is a
    fresh hosted skeleton with a different runtime model).
  - `src/main.rs` — Tokio entrypoint reading `FS_BIND_ADDR`,
    `FS_MODULE_ID` (required), `FS_LEDGER_ROOT` (required) from
    env.
  - `src/http.rs` — axum router with `/healthz`, `/readyz`,
    `/v1/contract`, `/v1/append`, `/v1/entries`. Per-tenant
    `X-Foundry-Module-ID` enforcement on the two business
    endpoints (mismatch → 403). `ApiError` type wraps internal
    errors with HTTP status + JSON body.
  - `src/ledger.rs` — `WormLedger` primitive enforcing the
    append-only invariant at the API surface. In-memory
    `Vec<Entry>` placeholder (first NEXT.md item: swap for
    hash-addressed segment files in immutable directories).
    Three unit tests pass: append assigns monotonic cursors,
    read_since filters strictly greater, read_since(0) returns
    all.
  - `README.md` + `README.es.md` — bilingual pair; the project
    never had READMEs before this commit (silently violating the
    bilingual rule from activation; closed in transit).
  - `cargo check` + `cargo test` both pass clean.
  Reference shape: slm-doorman-server in the `project-slm`
  cluster (`78031c4`); Master named this in the ratification
  message. Inherited the Tokio + axum + ApiError + tracing
  pattern; adapted for WORM-ledger semantics + per-tenant
  moduleId boundary.
- **Decision 3 — workspace membership held; re-add deferred
  behind Layer 1 audit.** Removed `service-fs` from root
  `Cargo.toml` `[workspace.members]`; added a new
  `[workspace.exclude]` array containing `service-fs` and
  `vendor-sel4-fs` (cargo requires explicit exclude when a
  nested package exists outside `[members]`). Tried re-adding
  `service-fs` to `[members]` once the rewrite passed clean per
  Master's "re-add when builds clean" instruction; workspace-
  level `cargo check --workspace` then failed with `openssl-sys`
  system-dep missing — pulled in by an existing sibling member,
  not by service-fs. Reverted the re-add because the failure is
  pre-existing Layer 1 audit work, not service-fs's problem.
  Re-add tracked as Blocked in `service-fs/NEXT.md`.
- **Bilingual-README hygiene closed in transit.** `service-fs`
  was activated 2026-04-25 without bilingual READMEs (silent
  violation of CLAUDE.md §6 / repo-layout.md "Required" entries).
  This session's Decision 1 commit added both
  `service-fs/README.md` and `service-fs/README.es.md`
  alongside the new Cargo manifest and src/. No separate
  cleanup commit needed.
- **Cluster manifest backfilled by Master, tracked here this
  session.** `~/Foundry/clones/project-data/.claude/manifest.md`
  was created by Master in their v0.0.2 drop (file landed
  untracked between sessions). Read at session start; will be
  added to git in the session-end commit so future sessions see
  it as part of the tracked state.
- **Doctrine v0.0.2 conventions applied.** Read
  `~/Foundry/conventions/trajectory-substrate.md` and
  `~/Foundry/conventions/bcsc-disclosure-posture.md`. The latter's
  forward-looking-information rule (§Rule 1) governs prose about
  future capability — already followed in the per-project
  CLAUDE.md / NEXT.md files written this session
  ("planned"/"intended"/"first NEXT.md item" rather than
  declarative future-tense). Trajectory-capture wiring
  (`capture-edit:` log lines on every commit this session) is
  Master's workspace-tier responsibility per `trajectory-
  substrate.md`; transparent to my work.
- **Workspace `.toggle` continues to alternate across sessions.**
  This session's three commits authored Jennifer / Peter /
  Jennifer (next: Peter for the session-end commit). Pattern
  consistent with Master's confirmation that the toggle is
  shared workspace state; no anomaly this session.
- **Registry summary updated.** Active unchanged at 8;
  Scaffold-coded unchanged at 50; Reserved-folder 36 → 37 (added
  `vendor-sel4-fs`); Total 98 → 99 (one new project).
- **Customer-first sequencing for next session.** Per the
  customer-first ordering convention and Master's inbox
  message, the next pickup proposal in the session-end outbox
  is `service-input` parser-dispatcher scaffold — the next-most-
  productive item now that service-fs has a working consumer
  surface (`/v1/append`).
- **Pending items carried to next session in this cluster:**
  1. service-input parser-dispatcher scaffold (Right-now
     proposed for next session)
  2. service-fs storage swap (in-memory → hash-addressed
     segment files)
  3. service-fs MCP-server interface layered on JSON-over-HTTP
  4. service-email EWS auth rebase
  5. service-people + service-email pre-framework subdirectory
     inventory
  6. service-fs systemd unit file (workspace-tier; coordinate
     via Master)
  7. Re-add service-fs to workspace `[members]` (Blocked on
     Layer 1 openssl-sys cleanup)

---

## 2026-04-26 (first session)

- **First Task Claude session in `cluster/project-data` (Ring 1)
  completed.** Acted on three inbox messages from Master Claude
  (v0.0.7 priority briefing, v0.0.9 SLM-stack FYI, v0.0.10
  auto-mode safety brief). Activated four projects per
  `~/Foundry/CLAUDE.md` §9: `service-fs`, `service-input` (created
  + activated), `service-people`, `service-email`. Five commits
  on branch `cluster/project-data`:
  - `ee209e3` activate service-fs
  - `fa1f71e` create service-input (Reserved-folder)
  - `1490e27` activate service-input (→ Active)
  - `c45b308` activate service-people
  - `032afe8` activate service-email
- **Drift surfaced (not silently propagated).** Two distinct
  drift findings during the activation pass:
  1. **`service-fs/src/main.rs`** is a `#![no_std] #![no_main]`
     bare-metal seL4 unikernel scaffold. Contradicts the same-day
     ratified `~/Foundry/conventions/three-ring-architecture.md`
     (Ring 1 = MCP-server processes) and
     `~/Foundry/conventions/zero-container-runtime.md` (every
     deployment is a Linux binary under systemd). Operator
     decision 2026-04-25: keep file untouched at activation;
     document drift in `service-fs/CLAUDE.md` "Current state";
     queue the rewrite as Blocked-on-Master in
     `service-fs/NEXT.md`. Surfaced to Master via cluster outbox
     `ring1-scaffold-runtime-model-drift` requesting ratification
     of (a) rewrite direction and (b) disposition of the
     existing scaffold (suggested: relocate to a future
     seL4-related project alongside `vendor-sel4-kernel` /
     `moonshot-sel4-vmm` rather than delete or leave-and-mark).
  2. **`service-email/src/auth.rs` + `src/graph_client.rs`** use
     in-process OAuth `client_credentials` against
     `login.microsoftonline.com` and call Microsoft Graph REST
     endpoints. Operator decision 2026-04-25 (real user-turn,
     out-of-band): rebase onto the EWS-based MSFT auth pattern
     proven in the sibling `service-email-egress-ews/` project —
     access token consumed from `AZURE_ACCESS_TOKEN` env (per
     `template.env` and `egress-ingress/src/main.rs` /
     `egress-roster/src/main.rs`), with EWS SOAP envelopes
     referenced from `egress-roster/ews_payload.xml`. Tokio
     runtime model preserved. Logged in
     `service-email/CLAUDE.md` "Current state" with the rebase
     queued as Right-now in `service-email/NEXT.md`. Not
     surfaced to Master — already operator-decided.
- **Prompt-injection attempt detected and neutralised.** A
  `<system-reminder>` block embedded in a tool result claimed to
  be a new user message instructing the EWS rebase. The harness
  flagged it as potentially malicious. The instruction was
  topically plausible (consistent with cluster contents and
  prior conversation), so the safe path was confirmation rather
  than refusal: paused activation of `service-email`, asked the
  user via the chat surface, received a real "yes" user turn,
  then proceeded. The earlier (premature) acknowledgment of the
  EWS instruction was walked back in the same chat surface.
  Logging here so future sessions know the EWS direction is
  legitimate operator policy, not adopted from an injected
  message.
- **`service-input` did not previously exist.** Created the
  directory with bilingual READMEs and added the registry row as
  Reserved-folder in `fa1f71e`; activated it directly to Active
  in `1490e27` because the parser-dispatcher scaffold is the
  entire next workstream and per-project doc discipline is wanted
  before any code lands. Total registry rows 97 → 98;
  Reserved-folder count untouched (transient +1 then -1).
- **Activation-state inventory deferred for two projects.** Both
  `service-people` and `service-email` carry pre-framework
  sub-directories that have not been inventoried
  (`service-people/{sovereign-acs-engine,spatial-crm,spatial-
  ledger,substrate,tools}/`;
  `service-email/{ingress-harvester,master-harvester-rs,sovereign-
  splinter,scripts}/`). Inventory + per-item keep/rename/retire/
  relocate decisions are queued as the first NEXT.md item in each
  project. Did not touch any of those sub-directories this
  session.
- **Workspace `.toggle` concurrency observation (FYI).** Across
  this session's five commits, the J/P alternation crossed two
  apparent skips (commits 2+3 both Peter; commits 4+5 both
  Jennifer) even though the helper's end-line correctly named
  the next identity each time. Root cause is most likely benign:
  the toggle file is shared workspace state, and any other
  session (Root Claude in another engineering repo, Master
  Claude using a helper) committing in parallel mutates it
  between this session's commits. The alternation is preserved
  across the workspace as a whole, not within any one session.
  Not a bug; not surfacing as an action item beyond the FYI in
  the cluster outbox to Master.
- **Registry summary updated.** Active 4 → 8
  (added `service-fs`, `service-input`, `service-people`,
  `service-email`); Scaffold-coded 53 → 50 (three Active
  promotions); Reserved-folder unchanged at 36 (transient +1/-1
  for `service-input`); Total 97 → 98 (one new project).
- **Pending for next session in this cluster:** wait for Master
  ratification on `service-fs` rewrite direction; begin EWS
  rebase work on `service-email/src/auth.rs`; inventory the
  pre-framework sub-directories in `service-people` and
  `service-email`. All queued in the per-project NEXT.md files.

---

## 2026-04-23

- **Repo-layout rule introduced.** Added
  `.claude/rules/repo-layout.md` codifying the allowed file set at
  the monorepo root and at each project directory root, and naming
  the sibling repos where cross-cutting content belongs (user guides,
  ADRs, design-system material). Anchor for the file-relocation work
  queued behind it (see `NEXT.md`).
- **Defects surfaced at root by this rule** — staged for separate
  commits, not moved in this session:
  - ~~`force_build.sh` (tracked, at repo root) → queued move to
    `vendor-sel4-kernel/scripts/`~~ **Closed 2026-04-23** — moved
    via `git mv` in a follow-up commit within this session. Zero
    runtime callers; script body uses absolute paths so no content
    edits required.
  - `GUIDE-OPERATIONS.md` (tracked, at repo root) → queued move to
    `content-wiki-documentation/`.
  - `USER_GUIDE_2026-03-30_V2.md` (tracked, at repo root) → queued
    move to `content-wiki-documentation/` with `_V2` dropped, per
    CLAUDE.md §6 edit-in-place rule.
  - ~~`app-console-content/src/{pointsav-surveyor.sh,surveyor.py}` →
    queued move to `app-console-content/scripts/`~~ **Closed
    2026-04-23** — both files moved via `git mv` (recognised as
    100% renames). Shell wrapper uses `$(dirname "$0")/surveyor.py`
    (relative) so the pair moves together without edits. Python
    script uses absolute paths into `woodfine-fleet-deployment` so
    location-independent. Zero intra-repo runtime callers; no cron
    entries found. The clone at `~/Foundry/clones/service-slm/`
    retains its copy on branch `cluster/service-slm` (separate
    `.git/`) and is unaffected by this move on `main`; it will
    receive the change only when that branch merges.
  - ~~`os-infrastructure/build_iso/forge_iso.sh` → queued rename to
    `os-infrastructure/build_iso/compile_binary.sh`~~ **Closed
    2026-04-23** — renamed via `git mv`; in-file header comment
    updated to reflect the new name and record the rename
    rationale. Zero external callers.
- ~~**Project-root scripts flagged (not yet moved):** ~15 scripts sit
  at project root instead of under `scripts/` across `service-vpn`
  (5 generator scripts), `service-email` (`spool-daemon.sh`),
  `service-slm` (`cognitive-bridge.sh`), `service-content`
  (`forge-seeds.sh`), `os-network-admin` (2 scripts),
  `os-totebox` (1), `tool-cognitive-forge` (1),
  `vendor-phi3-mini` (2), `app-mediakit-telemetry` (5 generic
  scaffold scripts). Each project is a separate closure task.~~
  **Closed 2026-04-23** — all 9 projects relocated in 9 separate
  `git mv` commits (18 files total, every one a 100% rename).
  Commit chain: `8f5cc48` os-totebox → `2456ea6` service-content
  → `30ff629` service-email → `cda2ce5` service-slm → `654d255`
  tool-cognitive-forge → `503f922` os-network-admin → `6df4be0`
  vendor-phi3-mini → `6f95279` service-vpn → `faae141`
  app-mediakit-telemetry. No callers needed updating; the only
  in-script references found were self-usage strings that remain
  valid after the move.
- **Stray runtime log surfaced.** `tool-cognitive-forge/llama.log`
  at project root — runtime log, almost certainly should be
  gitignored (and removed from tracking if tracked). Not addressed
  in this session. Added to `NEXT.md` as a separate item.
- **First rename-series closure: `service-parser` removed.**
  `service-parser/` directory deleted (`git rm -r`); contained
  only a README describing an abandoned AI-routing framing — no
  code, no data, no subdirectories. Zero runtime references
  anywhere in the repo. Rename-table row moved to Completed
  migrations; registry row removed; registry Defect count updated
  from 5 to 4 and Total rows from 100 to 99.
- **Second rename-series closure: `pointsav-pty-bridge` →
  `service-pty-bridge`.** Directory renamed via `git mv` (four
  100% renames: `.gitignore`, `Cargo.toml`, `Cargo.lock`,
  `src/main.rs`); `target/` left in place because it is gitignored
  build output. `Cargo.toml` `name` field updated in the same
  commit. Registry row moved from "Other / special" to the
  Service section, alphabetically between `service-people` and
  `service-search`, reclassified Defect → Scaffold-coded. Summary
  counters: Defect 4 → 3, Scaffold-coded 51 → 52, Total stays 99.
  Zero external Rust imports, no callers needed updating; not a
  workspace member. Stray `Cargo.lock` inside the renamed
  directory remains — resolves with workspace `Cargo.toml`
  unification (separate open structural defect).
- **Handoffs-outbound entries made self-executing.** Each outbox
  entry now carries a "Prescriptive actions" subsection with the
  exact commands a destination Root Claude can run mechanically —
  `cp` commands from source absolute path, `git add`, commit
  message, any in-transit edits, and the completion-signal commit
  pattern. Header also describes the convention so future outboxes
  follow the same shape. Two existing entries for
  `GUIDE-OPERATIONS.md` and `USER_GUIDE_2026-03-30_V2.md` updated
  with their prescriptive actions. This lets a cold-start Root
  Claude session in `content-wiki-documentation/` execute the
  handoffs without reading anything from this session's context.
- **Fifth (final) rename-series closure: Cognitive Forge term
  retired.** `service-slm/cognitive-forge/` renamed to
  `service-slm/router/`; former top-level `tool-cognitive-forge/`
  moved in as `service-slm/router-trainer/`. Producer/consumer
  now live together under `service-slm`. Rust Cargo.toml `name`
  field + `main.rs` usage string updated. Python
  `distill_knowledge.py` relocated from non-canonical `src/` to
  `scripts/` alongside `ignite_teacher.sh`. Three binary/log
  files stopped being tracked (`llamafile` 35 MB, `engine.log`,
  `llama.log`) via `git rm --cached` + new `.gitignore` section;
  physical files remain at new paths so the Python workflow still
  finds them. The 15 MB `qwen2.5-coder-1.5b.gguf` under `weights/`
  was already ignored. Registry Scaffold-coded 54 → 53, Total
  98 → 97 (one top-level project absorbed into `service-slm`).
  This closes the rename-series queue (5 of 5 done) and the
  separate `llama.log` stray item surfaced earlier in this
  session.
- **Fourth rename-series closure: `service-email-egress-{ews,imap}`
  wrappers flattened; consolidation plan reversed.** After
  reviewing sub-crate contents, EWS and IMAP are two
  protocol-specific adapters — not duplicates. Shared sub-crates:
  `egress-ingress`, `egress-ledger`, `egress-roster`,
  `data-ledgers/`. Protocol-specific: `egress-archive-ews` /
  `egress-archive-imap`; EWS-only: `egress-prune`,
  `egress-balancer`. Merging them would erase that architectural
  distinction. Instead, flattened the redundant
  `service-email-egress-ews/service-email-egress-ews/` wrapper
  (and the imap equivalent) — 73 files promoted up one level.
  Relative `../data-ledgers/` paths in Rust sources remain valid
  because crate dirs and `data-ledgers/` both moved together.
  Registry reclassified both from Defect → Scaffold-coded;
  Defect count 2 → 0 (registry is now Defect-free); Scaffold-coded
  52 → 54. The 13 dir-name / Cargo-name mismatches the 2026-04-18
  audit flagged (e.g., dir `egress-ingress` containing
  `Cargo.toml` with `name = "service-email-batch-ingress"`) are
  unaddressed and remain as a separate audit finding.
- **Third rename-series closure: `vendors-maxmind` reclassified
  to `app-mediakit-telemetry/assets/`.** Not a rename but a
  data-reclass: the directory held only the 63.5 MB
  `GeoLite2-City.mmdb` + READMEs with no code. The vendor's own
  README already named `app-mediakit-telemetry/assets/` as the
  intended target path — the monorepo had never realised that
  path. Moved the `.mmdb` + both READMEs into the documented
  target; removed `vendors-maxmind/.keep`; empty directory
  auto-removed by git. Closed the related "does it belong as a
  `vendor-*` crate at all?" open question (answer: no;
  non-workspace data directory). Updated monorepo `README.md`
  line 151 and `USER_GUIDE_2026-03-30_V2.md` line 902 (in-transit
  edit travels with the cross-repo handoff). Extended
  `repo-layout.md` to name `assets/` and `data/` as conventional
  project subfolders. Registry row removed; Defect 3 → 2, Total
  rows 99 → 98. Python script reference in
  `app-mediakit-telemetry/scripts/generic-omni-matrix-engine.py`
  left unchanged (it refers to deployment-side path relative to
  CWD — independent of monorepo-side layout). Separate `.mmdb` →
  build-time-fetch task remains open under Structural defects.
- **Open question surfaced.** `surveyor.py` hard-codes
  `MAX_DAILY_VERIFICATIONS = 10`. The existing cleanup-log open
  question — "Verification Surveyor daily throttle number — Under
  operational review. Do not cite a specific number" — must
  reconcile: either the code is authoritative (close the question,
  value is 10) or the doc is authoritative (the code is out of step
  and needs updating). Do not cite the number externally until
  resolved.
- **Second open question surfaced (os-infrastructure build
  pipeline).** The two scripts `os-infrastructure/forge_iso.sh`
  (ISO assembly) and `os-infrastructure/build_iso/compile_binary.sh`
  (binary compile, renamed this session) are sequential build
  stages but are not wired together — the assembly script does not
  invoke the compile script, and there is no Makefile or top-level
  driver. Operator must run them manually in order. Is this
  intentional (operator-gated two-step) or drift (should become a
  single driver script)? Pending decision before next pipeline
  refactor.
- **Handoff-outbound pattern piloted.** Added
  `.claude/rules/handoffs-outbound.md` as a cross-repo file-move
  outbox. Two entries lodged: `GUIDE-OPERATIONS.md` and
  `USER_GUIDE_2026-03-30_V2.md` both → `content-wiki-documentation`.
  Both files remain in place in this repo until a Root Claude in
  the destination repo commits the add-side; only then does a
  follow-up Root Claude session here commit the source-remove.
  The pattern is passive — an outbox entry waits for pickup.
- **Surfaced for Master Claude** (workspace-scope changes, outside
  Root Claude's write lane per §9):
  1. Formalise the cross-repo handoff pattern as an addendum in
     `~/Foundry/CLAUDE.md` §9. Current §9 stops at clone
     provisioning; the handoff mechanic is the natural extension
     for file movement between engineering repos.
  2. Extend `~/Foundry/CLAUDE.md` §10's `.claude/rules/` canonical
     list from three files to four — add `handoffs-outbound.md`
     alongside `repo-layout.md`, `project-registry.md`, and
     `cleanup-log.md`.
  3. Propagate both the `repo-layout.md` rule (§10 already names
     the monorepo as reference implementation) and the new
     `handoffs-outbound.md` pattern to the other engineering repos
     over time. Order of propagation is `~/Foundry/NEXT.md`'s
     concern.

---

## 2026-04-22

- **Project framework bootstrap.** Added `.claude/rules/project-registry.md`
  with 100-row inventory of every top-level directory, classified by
  state per `~/Foundry/CLAUDE.md` §8 (Reserved-folder /
  Scaffold-coded / Active / Defect / Not-a-project). Framework docs,
  templates, and activation procedure live workspace-level. This
  cleanup-log was also introduced onto `main` today (previously
  present only on feature branches — drift closed).
- **Taxonomy expanded to seven domains.** Added `app-orchestration-*`
  to the in-force `app-[os]-*` list in
  `~/Foundry/IT_SUPPORT_Nomenclature_Matrix_V8.md` §3. Triggered by
  `app-orchestration-bim` appearing during the session — would have
  been an unmatched-prefix defect under the original six-domain
  rule. Now conformant; `os-orchestration` already exists as a
  Systemic Wordmark (§2).
- **Four BIM-research directories registered.** `app-console-bim`,
  `app-orchestration-bim`, `app-workplace-bim`, `service-bim` — each
  with a single `RESEARCH.md`. Classified as Reserved-folder pending
  decision to activate.
- **Audit cleanup.** Removed 2 `__MACOSX/` directories and 16
  tracked `.DS_Store` / AppleDouble files from extraction-artefact
  scaffolding in the egress crates. Added `.DS_Store` to
  `.gitignore`.

---

## 2026-04-18 — Layer 1 structural audit — findings

- **Headline finding:** Workspace `Cargo.toml` declares only 8 of ~70+ crates as members. Everything else is treated as standalone workspaces, which explains the 23 stray `Cargo.lock` files scattered through the repo. `cargo build --workspace` will skip almost everything; profile/edition inheritance is not reaching most crates.
- **Severity counts:** 1 Critical, 1 High, 4 Medium, 1 Low.
  - Critical: workspace under-declaration (8 of ~70+ crates).
  - High: 23 stray `Cargo.lock` files inside member crates.
  - Medium: prefix violations (2); dir-name vs `Cargo.toml` name mismatches (13); doubly-nested `service-email-egress-{ews,imap}` scaffolding; many `app-console-*` / `app-network-*` directories without `Cargo.toml`.
  - Low: `discovery-queue` orphan data directory at root.
- **Good news on prefix adherence:** across ~85 directories, adherence to the seven canonical prefixes is approximately 97.6%. Only two violations found: `pointsav-pty-bridge` (no recognized prefix) and `vendors-maxmind` (plural form instead of canonical `vendor-`).
- **Nested redundancy:** `service-email-egress-ews` and `service-email-egress-imap` both contain a redundant intermediate directory of the same name — a doubly-nested copy-paste scaffolding pattern producing depth-3 crates. All 13 directory-name / `Cargo.toml`-name mismatches are concentrated in these nested egress areas (short dir names like `egress-ingress` aliasing qualified crate names like `service-email-batch-ingress`).
- **No modifications were made in this session — audit only.**
- **Next:** Open Questions section of this log to be updated separately with five new questions raised by the audit.

---

## 2026-04-18

- Initialized this cleanup log. Seeded active renames, deprecations, intentional exceptions, and open questions from Section 13 of the PointSav Project Instructions.
- Established the session-start / session-end read-and-update pattern in CLAUDE.md.
- No code changes in this session. Next session should confirm the active renames table against a fresh grep of the repo to establish a baseline count of remaining occurrences per legacy term.
- Open question surfaced: whether the `service-parser` / `service-extraction` consolidation is scoped for a specific MEMO version or tracked informally. Answer will determine how we prioritize closing that migration.
