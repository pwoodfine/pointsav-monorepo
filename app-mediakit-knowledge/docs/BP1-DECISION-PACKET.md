---
schema: foundry-doc-v1
document_version: 0.1.0
title: "BP1 Decision Packet — Phase 4 implementation clearance"
authors:
  - task-project-knowledge
last_revised: 2026-04-27
audience: operator
status: pending-operator-decision
upstream_doc: docs/PHASE-4-PLAN.md §7
companion_docs:
  - docs/PHASE-4-PLAN.md
  - ARCHITECTURE.md
  - UX-DESIGN.md
---

# BP1 Decision Packet — Phase 4 implementation clearance

> **Purpose.** Surface the seven open questions in
> `docs/PHASE-4-PLAN.md` §7 in a decide-quickly format so BP1
> clearance fits a 15-minute operator review rather than the
> ~1 hour the SLM Operationalization Plan §4 (workspace v0.1.42)
> budgets as PK.1.
>
> **How to use.** Each question carries the plan's recommendation,
> the strongest counter-argument, and the cost of changing the
> decision later. Confirm or override per question; record the
> outcome in §9 below. PK.2 (Phase 4 implementation via Sonnet
> sub-agents) unlocks on completion.
>
> **Time budget.** 15 minutes for confirm-recommendation cases;
> longer if any answer is overridden.

---

## §0 At-a-glance summary

| # | Question | Plan recommendation | Reversible? |
|---|---|---|---|
| 1 | MCP server transport | **HTTP** on `/mcp` route | High (stdio sub-command can land later) |
| 2 | Read-only Git remote protocol | **smart-HTTP** via the same axum server | Medium (git-daemon path adds firewall + systemd) |
| 3 | `--enable-mcp` default | **off** (matches `--enable-collab`) | High (one-line systemd unit edit) |
| 4 | Step 4.6 project-slm coordination | **Open — operator preference** | Low (touches cross-cluster contract) |
| 5 | `gix` vs `git2` split | **mixed** — git2 write side, gix read side | Medium (each side is one-shot rewrite to swap) |
| 6 | `libgit2-dev` install path | **install alongside `libssl-dev`** in same Master pass | High (apt is reversible) |
| 7 | OpenAPI 3.1 hand-author vs codegen | **hand-author** for Phase 4 | Medium (codegen can land as cleanup later) |

Six of the seven carry a clear recommendation; only Q4 is genuinely
open and benefits from operator preference rather than technical
analysis.

---

## §1 Question 1 — MCP server transport

**Question.** HTTP/JSON-RPC on `/mcp` (matches the existing axum
routes) vs stdio (more typical for desktop MCP integrations like
Claude Desktop).

**Plan recommendation.** HTTP.

**Why.** The substrate is server-shaped — the wiki engine is an
axum HTTP service serving the public web. An MCP server route
piggybacks on the same TLS termination, the same nginx vhost,
the same systemd unit, and the same audit path. A reader fetching
`/wiki/{slug}` and an agent fetching `/mcp` look identical to the
operator from a logs perspective.

**Counter-argument.** Desktop MCP integrations (Claude Desktop,
some IDE plugins) prefer stdio because they spawn subprocesses
locally. If the wiki engine is also expected to be a desktop tool,
stdio is the friendlier default.

**Cost of changing later.** Low. A future
`app-mediakit-knowledge mcp` subcommand could expose stdio
without touching the HTTP path. Both transports could coexist.

**Decision:** _________________

---

## §2 Question 2 — Read-only Git remote protocol

**Question.** smart-HTTP via the same axum server (recommendation;
piggybacks on existing TLS) vs git daemon on a separate TCP port
(more standard but adds firewall + systemd complexity).

**Plan recommendation.** smart-HTTP.

**Why.** The wiki content is already served over the existing TLS
endpoint; adding the smart-HTTP Git protocol on the same axum
router lets `git clone https://documentation.pointsav.com/<slug>.git`
work without opening another port, adding another systemd unit, or
maintaining another firewall rule. The implementation is a route
handler set, not a separate process.

**Counter-argument.** git-daemon is the standard Git server. Tools
that authenticate against `git://` URLs or expect git-daemon
behaviour need that path.

**Cost of changing later.** Medium. Adding git-daemon means a
separate systemd unit, a port reservation (typically 9418/tcp),
ufw + GCP firewall changes, and a separate cert path if TLS is
desired. Not destructive but a half-day of operational work.

**Decision:** _________________

---

## §3 Question 3 — `--enable-mcp` default

**Question.** off (recommendation; consistent with `--enable-collab`
Phase 2 Step 7 default-off pattern; production deploys opt in) vs
on (one fewer flag to set).

**Plan recommendation.** off.

**Why.** The MCP route exposes agent-callable tooling that bypasses
the human-author paths (CodeMirror editor, squiggle linter at edit
time). Until the Doorman auth + rate-limit policy is wired (Step
4.6), the route should not be reachable in production. Default-off
matches the `--enable-collab` pattern and signals operator intent
when enabled.

**Counter-argument.** Default-off requires every deployment to
explicitly opt in; one more flag to set, one more place to forget.

**Cost of changing later.** High to reverse. A one-line systemd
unit edit + daemon-reload + restart toggles the flag.

**Decision:** _________________

---

## §4 Question 4 — Step 4.6 coordination with project-slm cluster

**Question.** The Doorman (`service-slm/router/`) is the
workspace's own MCP client; per ARCHITECTURE.md §3 Phase 4, the
MCP server should be co-designed with Doorman for auth +
rate-limit policy. Outbox to project-slm Task before Step 4.6
lands? Or implement the server independently and let Doorman wire
to it after?

**Plan recommendation.** Open — operator preference.

**Why this is the genuinely open question.** The two paths have
different risk profiles and the right pick depends on operator
sequencing preference, not on technical analysis.

- **Outbox-first** path: this Task drafts the MCP server's auth
  contract + rate-limit shape, sends to project-slm Task via
  outbox, awaits feedback (~1 session round-trip), iterates,
  then implements. Slower to ship but lower drift risk.
- **Implement-independently** path: this Task picks reasonable
  defaults (Bearer-token auth in headers, simple per-tenant rate
  bucket, MCP version pin), implements, ships behind
  `--enable-mcp` off, and project-slm wires Doorman to whatever
  contract emerged. Faster to ship; project-slm absorbs the
  drift cost on the consumer side.

**Cost of changing later.** Low if outbox-first (the contract is
under co-design before any code lands); medium if
implement-independently (Doorman side may need to adapt to the
contract picked here). The MCP route is `--enable-mcp off` until
it's ready, so neither path puts the substrate at risk.

**Decision:** _________________

---

## §5 Question 5 — `gix` vs `git2` for the read-side

**Question.** `gix` is faster + memory-safer but the API is
younger. `git2` (libgit2-sys) is more mature.

**Plan recommendation.** Mixed — `git2` for the write side
(commit, index) where its API is stable; `gix` for the read side
(history, blame, diff) where its perf wins matter.

**Why.** Both crates are in active use across the Rust ecosystem
and the recommendation matches `gitoxide` maintainer Sebastian
Thiel's published roadmap (`gix` covers reads first, `git2`
remains the conservative write-side choice). The split also
isolates blast radius: a `gix` API change only affects read-side
routes (`/history`, `/blame`, `/diff`).

**Counter-argument.** Two crates means two dependency graphs, two
sets of build requirements. `git2` requires `libgit2-dev`; `gix`
is pure Rust. Picking one would simplify CI.

**Cost of changing later.** Medium. The read-side and write-side
each call distinct functions; swapping one is an isolated
refactor.

**Decision:** _________________

---

## §6 Question 6 — `libgit2-dev` install path

**Question.** `libgit2-dev` is the same class of dependency as
`libssl-dev` (one of the Phase 3 cleanup-log open items). Adds
another `apt install` to the production deployment runbook.

**Plan recommendation.** Install both at the same time when Master
next executes the runbook.

**Why.** PK.3 in the SLM Operationalization Plan (workspace
v0.1.42) names this exact bundle as a single ~10-minute Master
pass. Splitting them across two passes wastes operator coordination
time.

**Counter-argument.** None significant.

**Cost of changing later.** High to reverse (apt remove is fine);
either direction is reversible.

**Decision:** _________________

---

## §7 Question 7 — OpenAPI 3.1 hand-author vs codegen

**Question.** Hand-authoring at the start gives the spec a clean
structure; codegen (e.g. `utoipa`) keeps the spec automatically
in sync with the route handlers but adds a build step.

**Plan recommendation.** Hand-author for Phase 4; revisit `utoipa`
as a cleanup later if spec drift becomes a real problem.

**Why.** The Phase 4 route surface is small enough (~12 new
routes) that hand-authoring produces a cleaner artefact than
codegen output. `utoipa` introduces proc-macros across every
handler signature and changes the build profile. Reasonable to
defer until the route count or spec-drift incidents justify it.

**Counter-argument.** Hand-authored specs drift. Every route
addition risks the spec falling out of sync; the operational
remedy (CI lint) is more work than `utoipa`'s opt-in.

**Cost of changing later.** Medium. A `utoipa` introduction is a
one-week cleanup that touches every handler. Reversible but not
trivial.

**Decision:** _________________

---

## §8 Implementation effort estimate

The plan estimates ~2 weeks of Task time for the full 8-step
Phase 4 implementation, dispatched as Sonnet sub-agents per
v0.1.30 model-tier-discipline. Each sub-step is a bounded
brief (one task / one result / file paths / response cap)
ratified into `~/Foundry/.claude/sub-agent-queue.md`.

The seven decisions above shape eight briefs:

| Step | Brief shape | Affected by Q# |
|---|---|---|
| 4.1 git2 wiring + commit-on-edit | mechanical implementation per spec | 5, 6 |
| 4.2 GET /history + GET /blame via gix | mechanical implementation per spec | 5, 6 |
| 4.3 GET /diff (unified between two shas) | mechanical implementation per spec | 5 |
| 4.4 redb wikilink graph + GET /backlinks | mechanical implementation per spec | — |
| 4.5 blake3 federation seam | mechanical implementation per spec | — |
| 4.6 MCP server via rmcp | depends on coordination path | 1, 3, 4 |
| 4.7 read-only Git remote (smart-HTTP) | mechanical implementation per spec | 2 |
| 4.8 OpenAPI 3.1 spec | hand-authored or codegen depending on Q7 | 7 |

---

## §9 Operator decision record

Cleared 2026-04-28 by operator via Master Claude (workspace pass v0.1.54).

| # | Question | Decision | Notes |
|---|---|---|---|
| 1 | MCP server transport | **HTTP on `/mcp`** | Per plan recommendation; substrate is server-shaped |
| 2 | Read-only Git remote protocol | **smart-HTTP via the same axum server** | Per plan recommendation; piggybacks on existing TLS |
| 3 | `--enable-mcp` default | **off** | Per plan recommendation; matches `--enable-collab` Phase 2 Step 7 default-off pattern |
| 4 | Step 4.6 project-slm coordination | **outbox-first** | Draft auth + rate-limit contract; outbox to project-slm Task; ~1 session round-trip; iterate; implement. Aligns with v0.1.52 "everything routes through service-slm" intent (claim #32 Apprenticeship Substrate composition) |
| 5 | `gix` vs `git2` split | **mixed — `git2` for write side, `gix` for read side** | Per plan recommendation; matches gitoxide maintainer's published roadmap |
| 6 | `libgit2-dev` install path | **bundle with `libssl-dev` in PK.3 single Master pass** | Per plan recommendation; PK.3 in SLM Operationalization Plan §4 names this exact bundle |
| 7 | OpenAPI 3.1 hand-author vs codegen | **hand-author for Phase 4** | Per plan recommendation; revisit `utoipa` as cleanup later if spec drift becomes real |

After completing the table, this packet flows back to
project-knowledge cluster Task via inbox; that Task drafts the
8 sub-agent briefs reflecting the decisions and proposes them to
`~/Foundry/.claude/sub-agent-queue.md` for Master ratification.
PK.2 implementation begins after Master ratifies the queue.

---

## §10 Cross-references

- `docs/PHASE-4-PLAN.md` — full implementation plan with file map,
  route table, CLI flag additions
- `ARCHITECTURE.md` §3 Phase 4 — sequenced steps overview
- `~/Foundry/conventions/service-slm-operationalization-plan.md`
  §4 — PK.1 through PK.5 enumeration for this cluster
- `~/Foundry/conventions/model-tier-discipline.md` §1A — six rules
  for sub-agent dispatch (bounded brief, foreground+serial when
  writing, ≥80% confidence gate, layer scope preserved, anti-slop,
  parent never delegates commit decision)
- `~/Foundry/clones/project-knowledge/.claude/inbox-archive.md` —
  full v0.1.42 message text from Master
