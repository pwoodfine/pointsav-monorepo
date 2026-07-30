---
schema: foundry-doc-v1
document_version: 0.1.0
component: app-mediakit-knowledge
status: plan — drafted session-3 2026-04-27 for operator review at BP1 before implementation begins
last_updated: 2026-04-27
session: 3
companion_docs:
  - ARCHITECTURE.md
  - docs/UX-DESIGN.md
  - docs/INVENTIONS.md
  - docs/PHASE-2-PLAN.md
upstream_doctrine:
  - DOCTRINE.md claim #29 Substrate Substitution (v0.0.5)
  - DOCTRINE.md claim #31 Constrained-Constitutional Authoring (v0.0.6)
  - conventions/disclosure-substrate.md (v0.1.14)
  - conventions/zero-container-runtime.md
---

# Phase 4 — execution plan

The operator-reviewable artefact at BP1. Once this plan is committed
and operator clears the breakpoint, implementation begins per §1
ordering, with `cargo check` and an explicit commit at every step
boundary.

Strategic context lives in `ARCHITECTURE.md` §3 Phase 4. This plan
is *how* — file paths, build ordering, dependency strategy, test
plan, decision log, deferred items.

## 0. Goal in one line

Ship Phase 4: Git history surface + redb-backed link graph + blake3
federation-seam baseline + MCP server + read-only Git remote +
OpenAPI 3.1 spec — additive over Phases 1–3, no behaviour change to
existing routes.

## 1. Implementation order

Each step is independently buildable, lands as one commit on
`cluster/project-knowledge`, runs `cargo check` from inside
`app-mediakit-knowledge/` (per cleanup-log.md 2026-04-18 caveat —
do not rely on workspace-root `cargo check`), and triggers an L1
trajectory capture. Steps 4.1 through 4.4 are the load-bearing core
(git history surface); 4.5–4.8 layer on substrate-specific surfaces.

### Step 4.1 — git2 wiring + commit-on-edit

Smallest scope; lands the foundation that Steps 4.2–4.4 build on.

- **New module**: `src/git.rs`
  - `pub fn open_or_init(content_dir: &Path) -> Result<git2::Repository, WikiError>`
  - `pub fn commit_topic(repo: &Repository, slug: &str, body: &str, author_email: &str, author_name: &str, message: &str) -> Result<git2::Oid, WikiError>` — uses `git2`'s `Index::add_path` + `commit` machinery; default-branches `main`
  - `pub fn ensure_commit_identity_from_env(repo: &Repository) -> Result<(), WikiError>` — fall back to commit-as-next.sh's J/P alternation when no session author is supplied (Phase 5 auth replaces this)
- **Modify**: `src/server.rs` — `AppState` gains `git: Arc<Mutex<Repository>>` (git2 is `Send` but mutating ops need exclusive access; tantivy-style `Arc<Mutex<...>>` pattern)
- **Modify**: `src/edit.rs` — `post_edit` and `post_create` call `git::commit_topic` after `atomic_write` succeeds; failures logged but do NOT roll back disk write (consistent with the existing search-reindex policy)
- **Modify**: `src/main.rs` — `serve()` calls `git::open_or_init(&content_dir)` after `search::build_index`; passes the repo into `AppState`
- **Cargo.toml**: add `git2 = "0.20"` (libgit2-sys is a one-time C dep already noted in ARCHITECTURE.md §2; release build needs `libgit2-dev` system lib alongside the existing `libssl-dev` requirement)
- **Tests** (`tests/git_test.rs`, new): integration test — write TOPIC via `POST /create`; assert `git log` shows the commit; edit via `POST /edit`; assert second commit landed; verify alternating J/P identity preserved through git2 path

Acceptance: `cargo test` passes including the new integration test; manual `git -C <content_dir> log` shows commits per edit.

### Step 4.2 — GET /history/{slug} + GET /blame/{slug}

History surface — the read-only consumers of the git2 layer.

- **New module**: `src/history.rs`
  - `pub fn topic_history(repo: &Repository, slug: &str, limit: usize) -> Result<Vec<HistoryEntry>, WikiError>` — uses `gix` (read-side; faster + safer than git2 for read paths)
  - `pub fn topic_blame(repo: &Repository, slug: &str) -> Result<Vec<BlameLine>, WikiError>` — `gix-blame` or equivalent
  - `HistoryEntry { sha: String, author: String, email: String, timestamp_iso: String, message: String }`
  - `BlameLine { line_number: usize, line_text: String, sha: String, author: String, timestamp_iso: String }`
- **Modify**: `src/server.rs` mount routes:
  - `GET /history/{slug}` — HTML page; renders the entries as a table; each row links to `/diff/{slug}?b=<sha>&a=<sha>~`
  - `GET /blame/{slug}` — HTML page; renders source with per-line author + sha annotation (similar UX to GitHub blame)
- **Cargo.toml**: add `gix = "0.66"` (read-side); `gix-blame = "0.16"` (or current)
- **Tests** (`tests/history_test.rs`, new): seed TOPIC with N edits; assert `/history/{slug}` returns N entries; `/blame/{slug}` annotates each line

Acceptance: `cargo test` passes; manual smoke renders both pages cleanly.

### Step 4.3 — GET /diff/{slug}?a=&b=

Per-revision unified-diff rendering.

- **New module**: `src/diff.rs`
  - `pub fn topic_diff(repo: &Repository, slug: &str, sha_a: &str, sha_b: &str) -> Result<String, WikiError>` — uses `gix-diff` to produce a unified-diff string; render as HTML with syntax-highlighted +/- lines
- **Modify**: `src/server.rs` mount `GET /diff/{slug}?a=&b=` — query params are commit shas (or short-shas; resolve to full); returns HTML page
- **Tests** (`tests/diff_test.rs`, new): seed TOPIC with two revisions; assert `/diff/{slug}?a=<sha1>&b=<sha2>` returns HTML containing `+` and `-` lines reflecting the change

Acceptance: `cargo test` passes; manual diff viewing works.

### Step 4.4 — redb link-graph index + GET /backlinks/{slug}

The wikilink-graph foundation. Rebuilt on every commit (Phase 4.1 wires
the trigger).

- **New module**: `src/links.rs`
  - `LinkGraph` struct backed by `redb` keyed by `(from_slug, to_slug)` → `()`
  - `pub fn open_or_create(state_dir: &Path) -> Result<LinkGraph, WikiError>` — opens `<state_dir>/links.redb`
  - `pub fn rebuild_for_slug(graph: &LinkGraph, slug: &str, body: &str) -> Result<(), WikiError>` — parses wikilinks via the existing `comrak` extension; updates the table
  - `pub fn backlinks(graph: &LinkGraph, slug: &str) -> Result<Vec<String>, WikiError>` — query inverted (range over from-slugs whose to-slug equals input)
- **Modify**: `src/edit.rs` — `post_edit` + `post_create` call `links::rebuild_for_slug` after the disk write
- **Modify**: `src/server.rs` mount `GET /backlinks/{slug}` — JSON list of slugs that link to this one (and a chrome-friendly HTML page on `/wiki/{slug}` that includes a backlinks panel)
- **Modify**: `src/main.rs` build the LinkGraph on startup; pass into AppState
- **Cargo.toml**: add `redb = "4.1"`
- **Tests** (`tests/links_test.rs`, new): seed two TOPICs where A links to B; assert `/backlinks/B` returns `[A]`; edit A to remove the link; assert `/backlinks/B` returns `[]`

Acceptance: `cargo test` passes; backlinks panel surface visible in browser.

### Step 4.5 — blake3 hash storage (federation-seam baseline)

The cryptographic content-address that Phase 7 federation will key on.
Phase 4 ships the baseline: every commit records a blake3 hash of the
TOPIC body in `redb` keyed by `(slug, revision_sha)`. Phase 7 lights
up content-addressed retrieval (`GET /api/v1/page/{hash}`).

- **Modify**: `src/links.rs` (or new `src/hashing.rs` if we want strict separation):
  - Add a second redb table `hashes`: `(slug, revision_sha)` → `[u8; 32]` (blake3 digest)
  - `pub fn record_hash(graph: &LinkGraph, slug: &str, revision_sha: &str, body: &[u8]) -> Result<(), WikiError>`
  - `pub fn lookup_by_hash(graph: &LinkGraph, hash: &[u8; 32]) -> Result<Option<(String, String)>, WikiError>` — reverse lookup; powers Phase 7's content-addressed read endpoint as a no-op stub here
- **Modify**: `src/edit.rs` — `post_edit` + `post_create` call `record_hash` after `git::commit_topic` returns the new sha
- **Cargo.toml**: add `blake3 = "1.8"`
- **Tests**: extend `tests/links_test.rs` — assert the hash table has an entry per commit

Acceptance: `cargo test` passes; redb file at `<state_dir>/links.redb` shows two tables.

### Step 4.6 — MCP server (rmcp) — first-class agent surface

Per ARCHITECTURE.md §3 Phase 4 + DOCTRINE Pillar 2. The substrate
exposes itself as an MCP server so agents (Claude/GPT/Gemini and
the workspace's own Doorman) can interact through the standard
2026 protocol rather than scraping HTML.

- **New module**: `src/mcp.rs`
  - Resources: `wiki://topic/{slug}` per TOPIC (per-page resource URI)
  - Tools:
    - `search_topics(query: string, limit: int) -> Vec<{slug, title, score}>`
    - `get_revision(slug: string, revision_sha?: string) -> {body, frontmatter, sha, timestamp}`
    - `create_topic(title: string, body: string, slug?: string) -> {slug}`
    - `propose_edit(slug: string, body: string, message: string) -> {revision_sha}`
    - `link_citation(slug: string, citation_id: string) -> {ok}` — adds to frontmatter `cites:`
    - `list_backlinks(slug: string) -> [string]`
  - Prompts:
    - `/cite-this-page` — summarises the current TOPIC + suggests citations from `~/Foundry/citations.yaml`
    - `/summarize-topic` — short-form summary of a TOPIC
    - `/draft-related-topic` — generates a draft TOPIC referencing the current one
- **Cargo.toml**: add `rmcp = "0.x"` (or current Anthropic Rust MCP SDK release)
- **Modify**: `src/server.rs` mount `POST /mcp` — single endpoint serves the MCP JSON-RPC handshake + tool calls + resource reads (per MCP spec)
- **Modify**: `src/main.rs` add `--enable-mcp` CLI flag (default off; Phase 5 auth wires session-based access; Phase 4 demo mode is open)
- **Coordination with project-slm cluster**: the Doorman (`service-slm/router/`) is the workspace's own MCP client; co-design auth + rate-limit policy per ARCHITECTURE.md §3 Phase 4 final bullet. Outbox to project-slm Task before this step lands.
- **Tests** (`tests/mcp_test.rs`, new): MCP handshake (initialize); tool call round-trips for `search_topics`, `get_revision`, `create_topic`, `propose_edit`; resource read for `wiki://topic/{slug}`

Acceptance: `cargo test` passes; manual smoke with an MCP client (Claude Desktop config or `mcp inspect` CLI) shows the tools listed.

### Step 4.7 — read-only Git remote

The substrate exposes itself as `git://wiki.example.com/{tenant}.git`
for `git clone` style consumption. Pure read; writes go through the
auth'd HTTP surface (Phase 5).

- **New module**: `src/git_protocol.rs`
  - Implements the git protocol v2 server-side handshake using `git2` or `gix`'s server primitives
  - Single endpoint or path prefix (`/git-server/{tenant}.git`); handles the upload-pack flow
- **Modify**: `src/server.rs` mount `GET /git-server/{tenant}/info/refs` + `POST /git-server/{tenant}/git-upload-pack` (the git smart-HTTP protocol surface — easier to deploy than git daemon over a TCP port; rides existing nginx/TLS termination)
- **Modify**: `src/main.rs` add `--git-tenant <name>` flag (default `pointsav`); the path prefix uses this
- **Tests** (`tests/git_protocol_test.rs`, new): clone the served repo to a temp dir; assert the cloned content matches what's on disk

Acceptance: `cargo test` passes; manual `git clone http://localhost:9090/git-server/pointsav.git /tmp/clone` works.

### Step 4.8 — OpenAPI 3.1 spec

The substrate's REST surface formalised. Source of truth for client SDK
generation + (per Agent-1 research, arxiv 2507.16044) MCP tool
definition derivation.

- **New file**: `openapi.yaml` (hand-authored, at crate root)
  - Documents every Phase 1 + 1.1 + 2 + 3 + 4 endpoint
  - JSON Schema for every payload and frontmatter shape
  - Tags by phase (phase-1, phase-2-saa, phase-3-search, phase-3-feeds, phase-4-history, phase-4-mcp)
- **Modify**: `src/server.rs` mount `GET /openapi.yaml` — serves the spec as a static asset (use `rust-embed` to bake it into the binary)
- **Tests** (`tests/openapi_test.rs`, new): fetch the YAML; parse with `serde_yaml`; assert at least the well-known routes are present in `paths:`

Acceptance: `cargo test` passes; spec validates against an external OpenAPI 3.1 validator (manual check).

## 2. File map (full diff at end of Phase 4)

**New files** (~10):

```
src/git.rs                       (Step 4.1)
src/history.rs                   (Step 4.2)
src/diff.rs                      (Step 4.3)
src/links.rs                     (Step 4.4 + 4.5)
src/mcp.rs                       (Step 4.6)
src/git_protocol.rs              (Step 4.7)
openapi.yaml                     (Step 4.8)
tests/git_test.rs                (Step 4.1)
tests/history_test.rs            (Step 4.2)
tests/diff_test.rs               (Step 4.3)
tests/links_test.rs              (Step 4.4 + 4.5)
tests/mcp_test.rs                (Step 4.6)
tests/git_protocol_test.rs       (Step 4.7)
tests/openapi_test.rs            (Step 4.8)
```

**Modified files** (~5):

```
src/main.rs            (--enable-mcp, --git-tenant flags; build + pass new state)
src/server.rs          (mount Phase 4 routes; AppState gains git + links + mcp fields)
src/edit.rs            (post_edit + post_create call git + links + hashing on success)
src/lib.rs             (pub mod git, history, diff, links, mcp, git_protocol)
src/assets.rs          (embed openapi.yaml via rust-embed)
Cargo.toml             (git2, gix, gix-blame, gix-diff, redb, blake3, rmcp)
```

## 3. Routes added (Phase 4 surface)

| Route | Method | Purpose | Step |
|---|---|---|---|
| `/history/{slug}` | GET | Revision list (HTML) | 4.2 |
| `/blame/{slug}` | GET | Per-line blame (HTML) | 4.2 |
| `/diff/{slug}?a=&b=` | GET | Unified diff (HTML) | 4.3 |
| `/backlinks/{slug}` | GET | JSON list of slugs that link to this one | 4.4 |
| `/api/v1/page/{hash}` | GET | Content-addressed read (Phase 7 lights up; Phase 4 stubs) | 4.5 |
| `/mcp` | POST | MCP JSON-RPC | 4.6 |
| `/git-server/{tenant}/info/refs` | GET | git smart-HTTP protocol | 4.7 |
| `/git-server/{tenant}/git-upload-pack` | POST | git fetch (read-only) | 4.7 |
| `/openapi.yaml` | GET | OpenAPI 3.1 spec | 4.8 |

## 4. CLI flags added

```
--enable-mcp           default off; mounts /mcp; Phase 5 auth gates per-session
--git-tenant <name>    default pointsav; used in /git-server/{tenant}/... paths
```

Existing flags (`--content-dir`, `--state-dir`, `--bind`, `--citations-yaml`, `--enable-collab`) unchanged.

## 5. What's deferred (explicit non-goals for Phase 4)

- **Auth on `/edit`, `/create`, `/mcp`** — Phase 5 (`tower-sessions`,
  `axum-login`, `argon2id`, OIDC). Phase 4's commit-on-edit uses the
  alternating Jennifer/Peter identity from `bin/commit-as-next.sh`
  (same pattern Phase 2 established). MCP demo mode is open in Phase 4.
- **Content-addressed retrieval at `/api/v1/page/{hash}`** — Phase 7
  (federation seam). Phase 4 records the blake3 hashes; Phase 7 wires
  the lookup endpoint.
- **ActivityPub Inbox/Outbox** — Phase 7 (federation).
- **iroh content discovery** — Phase 7 (federation).
- **Webhook subscriptions** — Phase 5 (auth + AsyncAPI 3.1).
- **Tantivy reindex on commit** — already wired in Phase 3 Step 3.2;
  Phase 4 commits trigger reindex via the existing `post_edit` path.
- **Search snippet upgrade to query-aware** (`tantivy::SnippetGenerator`)
  — Phase 4.x or later; not blocking.

## 6. Test plan summary

| Step | Tests added | Coverage |
|---|---|---|
| 4.1 | `git_test.rs` | Commit per edit; J/P identity round-trip |
| 4.2 | `history_test.rs` | History list + per-line blame |
| 4.3 | `diff_test.rs` | Unified diff between two shas |
| 4.4 + 4.5 | `links_test.rs` | Backlinks add/remove on edit; blake3 hash records |
| 4.6 | `mcp_test.rs` | Handshake + 6 tool round-trips + resource read |
| 4.7 | `git_protocol_test.rs` | `git clone` round-trip |
| 4.8 | `openapi_test.rs` | Parse spec; well-known routes present |

Total Phase 4 test addition: ~25 unit + integration tests on top of
Phase 3's 97.

## 7. Open questions for operator at BP1

1. **MCP server transport**: HTTP/JSON-RPC on `/mcp` (matches the
   existing axum routes) vs stdio (more typical for desktop MCP
   integrations like Claude Desktop). Recommendation: **HTTP** — the
   substrate is server-shaped; stdio is for local tooling. A separate
   `app-mediakit-knowledge mcp` subcommand could expose stdio later
   if desktop integration becomes a concrete need.

2. **Read-only Git remote protocol**: smart-HTTP via the same axum
   server (recommendation; piggybacks on existing TLS) vs git daemon
   on a separate TCP port (more standard but adds firewall + systemd
   complexity).

3. **`--enable-mcp` default**: off (recommendation; consistent with
   `--enable-collab` Phase 2 Step 7 default-off pattern; production
   deploys opt in) vs on (one fewer flag to set).

4. **Step 4.6 coordination with project-slm cluster**: the Doorman
   (`service-slm/router/`) is the workspace's own MCP client; per
   ARCHITECTURE.md §3 Phase 4 the MCP server should be co-designed
   with Doorman for auth + rate-limit policy. Outbox to project-slm
   Task before Step 4.6 lands? Or implement the server independently
   and let Doorman wire to it after?

5. **`gix` vs `git2` for the read-side**: `gix` is faster + memory-safer
   but the API is younger. `git2` (libgit2-sys) is more mature.
   Recommendation: **mixed** — `git2` for the write side (commit, index)
   where its API is stable; `gix` for the read side (history, blame,
   diff) where its perf wins matter. Both are already in use across
   the Rust ecosystem; this is the conservative path the gix
   maintainer's published roadmap supports.

6. **`libgit2-dev` system-lib install on the deploy host**: the same
   class of dependency as `libssl-dev` (one of the Phase 3 cleanup-log
   open items). Adds another `apt install` to the production
   deployment runbook. Recommendation: install both at the same time
   when Master next executes the runbook.

7. **OpenAPI 3.1 spec hand-author vs codegen**: hand-authoring at the
   start gives the spec a clean structure; codegen (e.g. `utoipa`)
   keeps the spec automatically in sync with the route handlers but
   adds a build step. Recommendation: **hand-author** for Phase 4;
   revisit `utoipa` as a cleanup later if spec drift becomes a real
   problem.

## 8. Trajectory capture

Each step's commit triggers L1 trajectory capture via the
`.git/hooks/post-commit` hook, writing a corpus record to
`~/Foundry/data/training-corpus/engineering/project-knowledge/<sha>.jsonl`
per `conventions/trajectory-substrate.md`. Phase 4 will yield
8 corpus records (one per step + this plan commit).

The cluster adapter (`cluster-project-knowledge`, per Doctrine claim
#21) trains on these records when L3 lands at v0.5.0+.

## 9. Build-breathing budget

Per the Phase 2/3 pattern:

- `cargo check` from inside `app-mediakit-knowledge/` after every
  step (workspace-root check pulls service-content's openssl-sys
  drag — open question carried from Phase 3)
- `cargo test` after every step
- One step per commit; no batching
- Total estimated wall-time per step: 30-60 minutes implementation
  + check + test + commit; total Phase 4 implementation effort
  ~4-8 hours
- New system deps surfacing this phase: `libgit2-dev` (alongside the
  Phase 3 `libssl-dev` open item)

## 10. Cross-references

- `ARCHITECTURE.md` §3 Phase 4 — what
- `docs/UX-DESIGN.md` §5.6 (bottom strip — live diff against working
  tree) + §5.7 (commit gate) — UX surfaces that consume Phase 4 git
  state
- `~/Foundry/conventions/trajectory-substrate.md` — every commit feeds
  the cluster adapter
- `~/Foundry/conventions/compounding-substrate.md` — Doorman lives in
  `service-slm` (project-slm cluster); Step 4.6 coordination
- `cluster-manifest.md` — L1 trajectory capture target

---

*Plan ready for BP1 review. Implementation does not start until
operator clears.*
