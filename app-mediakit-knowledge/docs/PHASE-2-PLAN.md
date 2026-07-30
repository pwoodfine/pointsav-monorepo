---
schema: foundry-doc-v1
document_version: 0.1.0
component: app-mediakit-knowledge
status: plan — drafted session-3 2026-04-26 for operator review at BP1 before implementation begins
last_updated: 2026-04-26
session: 3
companion_docs:
  - ARCHITECTURE.md
  - docs/UX-DESIGN.md
  - docs/INVENTIONS.md
upstream_doctrine:
  - DOCTRINE.md claim #29 Substrate Substitution (v0.0.5)
  - DOCTRINE.md claim #31 Constrained-Constitutional Authoring (v0.0.6)
  - conventions/disclosure-substrate.md (v0.1.14)
  - conventions/zero-container-runtime.md
---

# Phase 2 — execution plan

The operator-reviewable artefact at BP1. Once this plan is committed
and operator clears the breakpoint, implementation begins step-by-step
per §1 ordering, with `cargo check` and an explicit commit at every
step boundary.

Strategic context lives in `ARCHITECTURE.md` §3 Phase 2 and
`docs/UX-DESIGN.md` §5. This plan is *how* — file paths, build
ordering, vendoring strategy, test plan, decision log, deferred
items.

## 0. Goal in one line

Ship Phase 2: edit endpoint + JSON-LD baseline + Substrate-Aware
Authoring (SAA) editor surface — additive over Phase 1, no
behaviour change to existing routes.

## 1. Implementation order

Each step is independently buildable, lands as one commit on
`cluster/project-knowledge`, runs `cargo check` from inside
`app-mediakit-knowledge/` (per cleanup-log.md 2026-04-18 caveat —
do not rely on workspace-root `cargo check`), and triggers an
L1 trajectory capture.

### Step 1 — JSON-LD baseline

Smallest scope, no JS deps, lands first to de-risk render-pipeline
modification.

- **New module**: `src/jsonld.rs`
  - `pub fn jsonld_for_topic(meta: &Frontmatter, body_text: &str) -> String`
  - Emits `<script type="application/ld+json">…</script>` block
  - Schema.org profile selection:
    - `TechArticle` for general TOPICs (default)
    - `DefinedTerm` if frontmatter has `disclosure_class: glossary` (introduces this enum value)
- **Modify**: `src/render.rs` to invoke `jsonld_for_topic` and inject the script tag inside `<head>`
- **Tests** (added to `tests/`):
  - Golden file: render `topic-hello.md`, assert `<script type="application/ld+json">` present, JSON parses, `@type` == "TechArticle"
  - Empty-frontmatter case: still emits a minimal valid TechArticle block

Acceptance: `cargo test` in crate root passes, all Phase 1 tests still pass.

### Step 2 — Edit endpoint server-side (pure Rust, no JS yet)

Pure server-side capability. Editor UI lands in Step 3; Step 2
ships the routes, the atomic write, and the path-traversal
hardening. Manually testable via `curl`.

- **New module**: `src/edit.rs`
  - `pub async fn get_edit(Path(slug): Path<String>) -> impl IntoResponse` — returns the editor HTML page (Step 3 fills in actual content; Step 2 serves a minimal placeholder)
  - `pub async fn post_edit(Path(slug): Path<String>, body: String) -> Result<impl IntoResponse, AppError>` — atomic write via `tempfile::NamedTempFile::persist` (POSIX-atomic temp+rename)
  - `pub async fn post_create(Json(req): Json<CreateRequest>) -> Result<impl IntoResponse, AppError>` — new TOPIC from `{title, slug?}`; SAA opens immediately
  - `fn validate_slug(slug: &str) -> Result<&str, AppError>` — extends Phase 1's read-side hardening: rejects `..`, leading `/`, `\`, NUL, any path-separator that resolves outside `<content_dir>`
- **Modify**: `src/server.rs` to mount three routes:
  - `GET  /edit/{slug}`
  - `POST /edit/{slug}`
  - `POST /create`
- **Modify**: `src/error.rs` to add `AppError::SlugInvalid`, `AppError::WriteFailed`, `AppError::PathTraversal`
- **Tests**:
  - Path-traversal denial: `POST /edit/../etc/passwd` → 400
  - Atomic write: write a TOPIC, kill the process between the temp write and the rename via signal — assert original file is intact (use `tempdir` test repo)
  - Roundtrip: create TOPIC, read it back, assert content matches
  - Slug normalisation: `Foo Bar` → `foo-bar` rejection or normalisation per Phase-6 rules (Phase 2 is conservative — reject any slug not matching `^[a-z0-9._-]+$`; full normalisation in Phase 6)

Acceptance: `cargo test` passes; manual `curl -X POST -d '...' /edit/test-topic` writes the file.

### Step 3 — Vendor CodeMirror 6 bundle + base editor wiring

JS dependencies enter the build for the first time. **Vendoring strategy** in §2.

- **New tree**: `vendor-js/`
  - `vendor-js/package.json` — pinned exact-version deps (CodeMirror 6 core, lang-markdown, codemirror-rich-markdoc OR ixora, lint, autocomplete; NO yjs yet)
  - `vendor-js/build.mjs` — esbuild config producing `static/vendor/cm-saa.bundle.js`
  - `vendor-js/.gitignore` — `node_modules/`, `package-lock.json` (we keep `package.json` pinned; lockfile is build-local)
  - `vendor-js/README.md` — how to rebuild the bundle when deps change
- **New script**: `scripts/build-vendor-js.sh`
  - Bash wrapper around `cd vendor-js && npm ci && node build.mjs`
  - Idempotent; writes `static/vendor/cm-saa.bundle.js` (committed to Git)
  - Documents the one-time `apt install nodejs npm` requirement
- **New file**: `static/vendor/cm-saa.bundle.js` (committed binary blob, ~300KB tree-shaken)
- **New file**: `static/saa-init.js` — first-party glue: instantiate CodeMirror, mount in editor page, wire save button to `POST /edit/{slug}`
- **New template** (in `src/edit.rs` via maud, no separate file): editor page chrome (article view + editor toggle; Wikipedia muscle-memory tabs from Phase 1.1 wire up to it)
- **Modify**: `src/assets.rs` to embed `static/vendor/` (extends rust-embed coverage)
- **Tests**: smoke only at this step
  - HTTP test: `GET /edit/test-topic` returns HTML containing `<script src="/static/saa-init.js">`
  - Manual smoke: load editor in browser, type, click save, assert file changes on disk

Acceptance: editor loads, types, saves; bundle committed; `cargo check` clean.

### Step 4 — SAA squiggle framework (deterministic rules)

The lint surface — Grammarly-pattern coloured squiggles per
UX-DESIGN.md §5.3. Phase 2 ships **deterministic** rules only;
LLM-grounded checks (Phase 9 CCA) defer.

- **New module**: `src/squiggle.rs`
  - `pub struct SquiggleRule { id, severity, pattern, message, citation }`
  - `pub fn deterministic_rule_set() -> Vec<SquiggleRule>` — compile-time constant
    - **Red rules** (commit-blocking): SYS-ADR-07 / SYS-ADR-10 / SYS-ADR-19 violations; "current equity holder" / "active auditor" framings of Sovereign Data Foundation per CLAUDE.md §6 BCSC rule 2
    - **Amber rules** (warning): paragraph contains a factual assertion but no `[citation-id]` (heuristic: regex on confident-claim verbs without bracket-syntax in the same paragraph)
    - **Blue rules** (FLI): forward-looking verb patterns in body of a TOPIC whose frontmatter does NOT have `forward_looking: true`
    - **Gray rules** (style): "Do Not Use" terms per `POINTSAV-Project-Instructions.md` §5
  - `pub async fn get_squiggle_rules() -> Json<Vec<SquiggleRule>>` — exposes the rule set as JSON
- **Modify**: `src/server.rs` mount `GET /api/squiggle-rules`
- **Modify**: `static/saa-init.js` to install a `@codemirror/lint` extension that fetches the rule set, runs each rule on every doc change, returns CodeMirror `Diagnostic[]` with severities mapped to red/amber/blue/gray
- **Tests**:
  - Server: `GET /api/squiggle-rules` returns JSON, each entry parses
  - Rule unit tests: synthetic input strings, assert rule fires (e.g., text "Sovereign Data Foundation is the active auditor" → red)

Acceptance: squiggle visible in editor for synthetic FLI text; tooltip cites the rule ID.

### Step 5 — SAA citation autocomplete

The other half of the editor's substrate-awareness: typing `[`
surfaces the citation registry.

- **New module**: `src/citations.rs`
  - `pub fn load_registry(path: &Path) -> Result<CitationRegistry, AppError>` — parses `~/Foundry/citations.yaml`
  - `pub async fn get_citations() -> Json<Vec<CitationEntry>>` — exposes registry as JSON
- **Modify**: `src/server.rs` mount `GET /api/citations`
- **Modify**: `src/main.rs` to add `--citations-yaml` CLI flag (default `~/Foundry/citations.yaml`; tests use a fixture path)
- **Modify**: `static/saa-init.js` to install `@codemirror/autocomplete` source on `[` trigger
- **Tests**:
  - Server: `GET /api/citations` parses, returns expected entries from a fixture YAML
  - JS smoke: type `[ni`, assert autocomplete dropdown contains `ni-51-102`

Acceptance: `[ni-…` autocompletes from `citations.yaml`.

### Step 6 — Three-keystroke ladder (affordances only — Doorman stubs)

Surface the affordances now so the editor's UX is complete; the
machinery wires up in Phase 4 when the Doorman MCP server lands.

- **Modify**: `src/server.rs` mount two stub routes
  - `POST /api/doorman/complete` → returns `501 Not Implemented` with body `{"phase": 4, "reason": "Doorman MCP integration deferred"}`
  - `POST /api/doorman/instruct` → same
- **Modify**: `static/saa-init.js`
  - **Tab key handler**: trigger ghost-text completion request to `/api/doorman/complete`; on 501, render a one-time toast "Tab completion lights up in Phase 4 (Doorman integration)" and don't repeat
  - **Cmd-K handler**: open instruction box (CodeMirror panel), POST to `/api/doorman/instruct`; on 501, same toast pattern
  - **Composer**: omitted entirely (Phase 4 — depends on multi-file change pipeline)
- **Tests**: server stubs return 501 with expected JSON shape

Acceptance: editor surfaces Tab and Cmd-K affordances; 501 path is graceful.

### Step 7 — Real-time collab opt-in (`--enable-collab`)

Optional, additive, gated behind a CLI flag. If the flag is not
set, no collab code paths execute.

- **New module**: `src/collab.rs`
  - WebSocket route `GET /ws/collab/{slug}` via axum WS upgrade
  - In-memory `HashMap<String, YDocSession>` keyed by slug; per-slug Y.Doc on the server side via `yrs` (Rust port of Yjs)
  - On client disconnect: if last client, snapshot Y.Text → file via the same atomic-write path from Step 2
- **Modify**: `Cargo.toml` add deps (only when this step lands):
  - `yrs = "0.21"`
  - `axum` `[ws]` feature
- **Modify**: `src/main.rs` add `--enable-collab` flag; CLI struct exposes it
- **New file**: `vendor-js/build.mjs` extended to produce `static/vendor/cm-collab.bundle.js` (yjs + y-codemirror.next + y-websocket-client; ~100KB additional)
- **Modify**: `src/server.rs` to template `<script>window.WIKI_COLLAB_ENABLED = true;</script>` into the editor page only when `--enable-collab` is set
- **Modify**: `static/saa-init.js` to lazy-load `cm-collab.bundle.js` only if `WIKI_COLLAB_ENABLED`
- **Tests**:
  - Server: `--enable-collab` set → editor page contains `WIKI_COLLAB_ENABLED = true`
  - Server: `--enable-collab` unset → no collab script reference
  - Manual smoke: two clients, edit, see each other's cursor (Playwright optional; may defer to a follow-up)

Acceptance: collab works behind the flag; default-off path has zero collab JS loaded.

## 2. Vendoring strategy (CodeMirror + Yjs)

**Approach**: pre-build JS bundles out-of-tree, commit bundle artefacts to `static/vendor/`. No NPM in the Rust build path.

```
vendor-js/                        ← build inputs (committed)
├── package.json                  exact-pinned versions
├── build.mjs                     esbuild config
├── README.md                     rebuild instructions
└── .gitignore                    excludes node_modules/

static/vendor/                    ← build outputs (committed binaries)
├── cm-saa.bundle.js              ~300KB tree-shaken (Step 3)
├── cm-saa.bundle.js.map          sourcemap (optional; omit for v1)
└── cm-collab.bundle.js           ~100KB additional (Step 7, optional)
```

**Why commit the bundles**:
- Single-binary deployment per `conventions/zero-container-runtime.md`
- Consumers get one tarball; no `npm` required to run
- Reproducible: bundle hash printed in `cargo build` output via build.rs
- Bundle is small enough (<500KB total) that Git history bloat is acceptable

**When bundles rebuild**:
- Bumping CodeMirror or Yjs versions (manual operator action)
- Build script runs `npm ci` (deterministic install from `package.json`); does NOT use `package-lock.json` (intentionally regenerated)
- Bundle hash printed in `vendor-js/README.md` after each rebuild for human verification

**Pinned versions** (exact, not range):
- `@codemirror/state` `6.x.x` (latest at impl time)
- `@codemirror/view` `6.x.x`
- `@codemirror/commands` `6.x.x`
- `@codemirror/lang-markdown` `6.x.x`
- `@codemirror/lint` `6.x.x`
- `@codemirror/autocomplete` `6.x.x`
- `codemirror-rich-markdoc` OR `ixora` (decision in Step 3 implementation; benchmark both)
- Step 7 only: `yjs`, `y-codemirror.next`, `y-websocket`

## 3. File map (full diff at end of Phase 2)

**New files** (15 — counts source + tests + vendor + scripts):

```
src/edit.rs                               (Step 2)
src/jsonld.rs                             (Step 1)
src/squiggle.rs                           (Step 4)
src/citations.rs                          (Step 5)
src/collab.rs                             (Step 7, optional in Phase 2)
static/saa-init.js                        (Step 3, extended in 4/5/6/7)
static/vendor/cm-saa.bundle.js            (Step 3)
static/vendor/cm-collab.bundle.js         (Step 7)
vendor-js/package.json                    (Step 3)
vendor-js/build.mjs                       (Step 3)
vendor-js/README.md                       (Step 3)
vendor-js/.gitignore                      (Step 3)
scripts/build-vendor-js.sh                (Step 3)
tests/edit_test.rs                        (Step 2)
tests/jsonld_test.rs                      (Step 1)
tests/squiggle_test.rs                    (Step 4)
tests/citations_test.rs                   (Step 5)
tests/collab_test.rs                      (Step 7)
```

**Modified files** (4):

```
src/main.rs                               --citations-yaml, --enable-collab flags
src/server.rs                             mount Phase 2 routes
src/render.rs                             call jsonld_for_topic
src/error.rs                              add Phase 2 AppError variants
src/assets.rs                             embed static/vendor/
src/lib.rs                                pub mod edit, jsonld, squiggle, citations, collab
Cargo.toml                                tempfile (dev → prod), serde_json, optionally yrs + axum[ws]
```

## 4. Routes added (Phase 2 surface)

| Route | Method | Purpose | Step |
|---|---|---|---|
| `/edit/{slug}` | GET | Editor HTML page | 2 (placeholder), 3 (real) |
| `/edit/{slug}` | POST | Atomic write of edited TOPIC | 2 |
| `/create` | POST | New TOPIC from `{title, slug?}` | 2 |
| `/api/squiggle-rules` | GET | Deterministic rule set as JSON | 4 |
| `/api/citations` | GET | Citation registry as JSON | 5 |
| `/api/doorman/complete` | POST | Tab completion stub (501 in Phase 2) | 6 |
| `/api/doorman/instruct` | POST | Cmd-K instruction stub (501 in Phase 2) | 6 |
| `/ws/collab/{slug}` | WS | y-codemirror collab transport | 7 (opt-in) |

Phase 1 routes (`/`, `/wiki/{slug}`, `/static/{*path}`, `/healthz`) unchanged.

## 5. CLI flags added

```
--citations-yaml <path>   default ~/Foundry/citations.yaml; reads at startup, exposed via /api/citations
--enable-collab           default off; gates Step 7 collab paths
```

Existing flags (`--content-dir`, `--bind`, `--state-dir`) unchanged.

## 6. What's deferred (explicit non-goals for Phase 2)

- **Composer / multi-file edit** → Phase 4 (depends on Doorman MCP)
- **Doorman integration for Tab/Cmd-K** → Phase 4
- **IVC reading-side machinery** (squiggles in *read* view) → Phase 4–7
- **CCA constrained decoding** → Phase 9 / project-disclosure cluster
- **Authentication for `/edit` / `/create`** → Phase 5; Phase 2 commits as the alternating Jennifer/Peter identity via `bin/commit-as-next.sh` (this is the same identity-pattern Phase 1 implicitly used for Git operations)
- **Tantivy search rebuild on edit** → Phase 3 (search lands in Phase 3, then Phase 4 wires edit→reindex)
- **Live diff against working tree (bottom-strip)** → Phase 4 (depends on `git2` integration)
- **Side rail — related TOPICs** → Phase 4.x (depends on Doorman embedding API)

## 7. Test plan summary

| Step | Tests added | Coverage |
|---|---|---|
| 1 | jsonld_test.rs | Golden-file render + JSON parse + @type field |
| 2 | edit_test.rs | Path traversal, atomic write under signal, roundtrip |
| 3 | (smoke only — bundle existence + server response) | HTTP + manual browser smoke |
| 4 | squiggle_test.rs | Each rule fires on synthetic input |
| 5 | citations_test.rs | Registry parse + endpoint shape |
| 6 | (server stubs; toast UI manually verified) | 501 response shape |
| 7 | collab_test.rs | Flag-gated script presence |

Total Phase 2 test addition: ~30 unit + integration tests on top of Phase 1's 8.

## 8. Open questions for operator at BP1

1. **Authentication on `/edit` / `/create`**. Phase 2 uses the alternating Jennifer/Peter `bin/commit-as-next.sh` identity for the underlying Git write (Phase 4 lights up; Phase 2 just writes the file, Phase 4 commits it). Is that acceptable for the demo, or should Phase 2 ship a stub auth gate so unauthenticated POSTs are refused? Recommendation: **ship with no auth for Phase 2** (loopback-bound by default, single-tenant per `--content-dir`); production deployment will sit behind a TLS-terminating reverse proxy that enforces network-level auth until Phase 5.

2. **Collab transport: same axum server vs separate y-websocket process.** Recommendation: **same server**, axum WS upgrade — simpler, one binary, no cross-process state. Trade: harder to scale beyond one node, but Phase 2 is single-node demo.

3. **Vendor bundle commit policy.** Recommendation: **commit the bundles** to Git (~500KB total). Trade: history bloat over time. Mitigation: rebuild only on dep bumps (rare); bundle hash in README for human verification of commit-vs-build divergence.

4. **JSON-LD profile selection**. Recommendation: **TechArticle for general TOPICs, DefinedTerm only when frontmatter `disclosure_class: glossary`**. This introduces a new enum variant — should `disclosure_class:` enum stay as `narrative | financial | governance` (per ARCHITECTURE.md §6) and `glossary` be a separate field? Recommendation: **add `glossary` to the enum** — narratively it's a fourth class.

5. **Step 7 (collab) inclusion in Phase 2**. Recommendation: **defer Step 7 to Phase 2.x** unless operator wants live collab in the v0.2.0 demo. Steps 1–6 deliver the SAA editor and citation-grounded squiggles — the load-bearing Phase 2 value. Collab is additive and well-isolated.

6. **CodeMirror-rich-markdoc vs ixora**. Recommendation: **benchmark both during Step 3** (load time, mobile responsiveness, blur/focus smoothness), pick at impl time, document choice in `vendor-js/README.md`. Both are MIT-licensed; both implement the Obsidian Live Preview pattern.

## 9. Trajectory capture

Each step's commit triggers L1 trajectory capture via the `.git/hooks/post-commit` hook, writing a corpus record to `~/Foundry/data/training-corpus/engineering/project-knowledge/<sha>.jsonl` per `conventions/trajectory-substrate.md`. Phase 2 will yield 6–7 corpus records (one per step + this plan commit).

The cluster adapter (`cluster-project-knowledge`, per Doctrine claim #21) trains on these records when L3 lands at v0.5.0+.

## 10. Build-breathing budget

Per operator's "let the build breathe properly" instruction:

- `cargo check` from inside `app-mediakit-knowledge/` after every step (workspace-root check covers only 8 of ~70 monorepo crates — misleading per cleanup-log.md 2026-04-18)
- `cargo test` after every step
- Anchor between commits to allow trajectory-capture hooks to complete
- One step per commit; no batching
- Total estimated wall-time per step (excluding bundle build): 5–15 minutes implementation + check + test + commit

Bundle build (Step 3 vendor JS one-time): ~2 minutes including `npm ci`.

## 11. Cross-references

- `ARCHITECTURE.md` §3 Phase 2 — what
- `docs/UX-DESIGN.md` §5 — how it should feel
- `docs/INVENTIONS.md` §A — substrate-enforced AI grounding (squiggles are the editor-side enforcement surface)
- `~/Foundry/citations.yaml` — Step 5 input
- `~/Foundry/conventions/bcsc-disclosure-posture.md` — Step 4 amber/blue/gray rule sources
- `~/Foundry/conventions/zero-container-runtime.md` — vendor strategy rationale
- `cluster-manifest.md` — L1 trajectory capture target

---

*Plan ready for BP1 review. Implementation does not start until operator clears.*
