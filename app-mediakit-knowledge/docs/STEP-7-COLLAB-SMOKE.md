---
schema: foundry-doc-v1
document_version: 0.1.0
title: "Step 7 collab — manual two-client smoke + production enable runbook"
authors:
  - task-project-knowledge
last_revised: 2026-04-27
audience: operator + master
status: pending-operator-smoke
upstream_doc: docs/PHASE-2-PLAN.md §1 Step 7
companion_docs:
  - ARCHITECTURE.md
  - UX-DESIGN.md
  - ~/Foundry/infrastructure/local-knowledge/local-knowledge.service
---

# Step 7 collab — smoke + production enable runbook

> **Purpose.** Capture the manual two-client smoke procedure for
> Phase 2 Step 7 (yjs collab via passthrough relay) and pre-stage
> the systemd unit edit + rollback so Master can ship to production
> in minutes after operator confirms the smoke passed.
>
> **Scope.** PK.4 in the SLM Operationalization Plan §4 (workspace
> v0.1.42). ~1 hour total: ~30 min operator smoke + ~10 min Master
> deploy + ~20 min verification.
>
> **Status.** Step 7 shipped as default-off behind `--enable-collab`
> (commit `05f1dab`); production binary at v0.1.29 was built from
> cluster HEAD but the systemd unit's `ExecStart` does not pass
> `--enable-collab`. The route `/ws/collab/{slug}` and the JS
> bundle `cm-collab.bundle.js` are unreachable in production until
> the unit edit lands.

---

## §1 Why a manual smoke

The collab implementation has unit + integration test coverage
proving the framework is wired correctly: WebSocket route accepts
connections, `tokio::sync::broadcast` per-slug rooms multiplex
correctly, the 256-message lag buffer drains without panic, the
client-side bundle loads when the template's
`window.WIKI_COLLAB_ENABLED` flag is set.

What the unit + integration tests do not cover is **the visual
property** of two browsers seeing each other's cursors render in
the CodeMirror gutter. Cursor presence is awkward to assert
programmatically (it requires a headless-browser harness with
two coordinated WebDriver sessions plus DOM-tree diffing), and
the Phase 2 Step 7 cost-benefit didn't justify that test
infrastructure. A manual smoke covers the gap and ratifies
production readiness.

The smoke is run **before** the production unit edit lands.
Until smoke passes, the collab JS bundle should not load on the
public URL.

---

## §2 Pre-smoke setup

The smoke runs against a **temporary** instance of the binary
that is collab-enabled, on a port that is **not** the production
port. Production stays on `127.0.0.1:9090`; smoke runs on
`127.0.0.1:9099` (or any free port).

### §2.1 Build the collab-enabled debug binary

From inside the cluster sub-clone:

```bash
cd /srv/foundry/clones/project-knowledge/pointsav-monorepo/app-mediakit-knowledge
cargo build   # debug build is fine for smoke; faster than --release
```

Build duration is approximately 2–4 minutes for an incremental
build, longer if the `target/` directory is cold.

### §2.2 Run the smoke binary on a non-production port

```bash
./target/debug/app-mediakit-knowledge serve \
  --content-dir /srv/foundry/clones/project-knowledge/content-wiki-documentation/launch-placeholder \
  --citations-yaml /srv/foundry/citations.yaml \
  --state-dir /tmp/local-knowledge-smoke-state \
  --bind 127.0.0.1:9099 \
  --enable-collab
```

`--state-dir` points at a tmpfs path, so smoke artefacts don't
mix with production state at `/var/lib/local-knowledge/state`.

### §2.3 Open SSH tunnel from operator's laptop

```bash
gcloud compute ssh foundry-workspace --zone=us-central1-a -- \
  -L 9099:localhost:9099 -N
```

This forwards `localhost:9099` on the operator's laptop to the
VM's `127.0.0.1:9099`. Two browsers on the laptop both connect
to `http://localhost:9099`.

---

## §3 Smoke procedure

Two browsers (any combination — Firefox + Chrome, two Firefox
windows in different profiles, etc.) editing the same TOPIC.

### §3.1 Open the editor in browser A

Navigate to `http://localhost:9099/edit/welcome`. The CodeMirror
editor loads. Type a few characters at the top of the document.
The cursor renders normally (single cursor, no presence indicator
yet because no other client is in the room).

### §3.2 Open the same editor in browser B

In a second browser (or a different profile), navigate to the
same URL: `http://localhost:9099/edit/welcome`.

**Expected**: within ~500 ms of the second browser's WebSocket
opening, both browsers display the *other* browser's cursor as
a coloured block in the gutter. Cursor colour is deterministic
per-client-id; two clients see two distinct colours.

### §3.3 Type in browser A; observe browser B

Type a sentence at the top of browser A's editor. Browser B
should display each character as it lands, with browser A's
remote cursor rendering inline. Latency from keystroke to
remote-render should be under 200 ms on a local SSH tunnel.

### §3.4 Type in browser B; observe browser A

Same but reversed. Browser A should display browser B's edits
inline.

### §3.5 Save from one browser

Click the save button in browser A. The atomic-write path runs
through `POST /edit/{slug}`. The disk file at
`<content-dir>/welcome.md` updates.

**Expected**: browser B continues to see the in-memory CRDT
state (no disruption to the collab session) but does NOT need
to also save — the disk write happened once. The CRDT remains
canonical for in-session edits; git commits one snapshot per
save.

### §3.6 Close one browser; observe the other

Close browser B. Browser A should remove the remote cursor
indicator within ~5 seconds (the WebSocket connection-close
event fires the presence-removal).

### §3.7 Smoke pass criteria

All seven steps complete without:

- WebSocket disconnects requiring a manual page reload
- Cursor flicker, ghost cursors, or stale presence indicators
  after a client closes
- Type-ahead latency above ~500 ms on a local SSH tunnel
  (network-bound; not a substrate concern)
- JavaScript console errors in either browser
- Server-side panics or 500s in the smoke-binary logs

If all pass, smoke is ratified. Move to §4 production enable.
If any fail, log the failure mode in the cluster cleanup-log
and surface in outbox; production enable holds.

---

## §4 Production enable — pre-staged systemd unit edit

The current production unit at
`~/Foundry/infrastructure/local-knowledge/local-knowledge.service`
runs without `--enable-collab`. The edit below adds the flag to
the `ExecStart` line.

### §4.1 Unified diff (Master applies)

```diff
--- a/infrastructure/local-knowledge/local-knowledge.service
+++ b/infrastructure/local-knowledge/local-knowledge.service
@@ -<line> +<line> @@
 ExecStart=/usr/local/bin/app-mediakit-knowledge serve \
   --content-dir /srv/foundry/clones/project-knowledge/content-wiki-documentation/launch-placeholder \
   --citations-yaml /srv/foundry/citations.yaml \
   --state-dir /var/lib/local-knowledge/state \
+  --enable-collab \
   --bind 127.0.0.1:9090
```

The exact line numbers depend on the current file state; Master
locates the `ExecStart=` block and inserts the `--enable-collab \`
line preserving the trailing-backslash continuation pattern.

### §4.2 Apply the edit

```bash
# Master scope per CLAUDE.md §11 — infrastructure/ is workspace-tier
cd ~/Foundry
$EDITOR infrastructure/local-knowledge/local-knowledge.service
# add the --enable-collab \ line as in §4.1

# Sync IaC to live unit (the workspace's IaC convention is symlink
# or copy; check the existing pattern before deciding)
sudo cp infrastructure/local-knowledge/local-knowledge.service \
  /etc/systemd/system/local-knowledge.service

sudo systemctl daemon-reload
sudo systemctl restart local-knowledge.service
sudo systemctl status local-knowledge.service
```

Service should re-enter `active (running)` within a second or
two. No drain is needed — collab sessions are session-ephemeral
per the substrate's source-of-truth inversion (the disk markdown
is canonical; in-memory CRDT discards on restart).

### §4.3 Production verification

From any external host:

```bash
# Editor route still loads
curl -I https://documentation.pointsav.com/edit/welcome
# expect: HTTP/1.1 200 OK

# WebSocket route now reachable (a 426 Upgrade Required is the
# expected response to a non-Upgrade GET; absence of 404 confirms
# the route is wired)
curl -I https://documentation.pointsav.com/ws/collab/welcome
# expect: HTTP/1.1 426 Upgrade Required (or 101 if curl negotiates upgrade)

# JS bundle loads
curl -I https://documentation.pointsav.com/static/vendor/cm-collab.bundle.js
# expect: HTTP/1.1 200 OK
# expect: Content-Length: ~302000  (the 302 KB bundle)
```

If all three pass, production collab is enabled. The next
operator opening `/edit/{slug}` in two browsers will see the
collab presence indicators on the live URL.

---

## §5 Rollback

If production verification fails, or if a regression surfaces in
the live binary, rollback is a one-line edit:

```bash
cd ~/Foundry
$EDITOR infrastructure/local-knowledge/local-knowledge.service
# remove the --enable-collab \ line; restore the prior shape

sudo cp infrastructure/local-knowledge/local-knowledge.service \
  /etc/systemd/system/local-knowledge.service
sudo systemctl daemon-reload
sudo systemctl restart local-knowledge.service
```

Service returns to the v0.1.29 default-off posture. The collab
JavaScript stops loading on `/edit/{slug}` because the template
`window.WIKI_COLLAB_ENABLED` flag flips back to `false`. The
WebSocket route returns 404. No data loss — collab state is
session-ephemeral.

The rollback is non-destructive at every layer.

---

## §6 What this runbook does NOT change

- The `/edit/{slug}` route's atomic-write semantics (CodeMirror →
  `POST /edit` → atomic file rename) are unchanged whether collab
  is on or off
- The squiggle linter (Phase 2 Step 4), citation autocomplete
  (Phase 2 Step 5), and Doorman ladder stubs (Phase 2 Step 6) are
  unchanged
- Search index, feeds, sitemap, robots.txt, llms.txt, and `/git/`
  routes are unchanged
- All Phase 1 + Phase 1.1 chrome (article rendering, muscle-memory
  layout, language switcher) is unchanged
- The placeholder content tree at
  `content-wiki-documentation/launch-placeholder/` is unchanged

Step 7 collab is **purely additive** to the Phase 2 + Phase 3
surface. Disabling it (rollback) returns the system to the
v0.1.29 default-off state byte-for-byte.

---

## §7 Cross-references

- `docs/PHASE-2-PLAN.md` §1 Step 7 — original Step 7 brief +
  vendoring decisions for `cm-collab.bundle.js`
- `ARCHITECTURE.md` §11 — API surface set including `/ws/collab/{slug}`
- `UX-DESIGN.md` §4.7 — collab UX wireframe + cursor-presence
  pattern
- `~/Foundry/conventions/service-slm-operationalization-plan.md`
  §4 — PK.4 enumeration for this cluster
- `~/Foundry/clones/project-knowledge/.claude/inbox-archive.md` —
  v0.1.29 (HTTPS launch context) and v0.1.42 (PK enumeration)

---

## §8 Operator + Master sign-off

| Step | Signed by | Date | Notes |
|---|---|---|---|
| §3 smoke pass criteria met | _________ | _________ | |
| §4 production enable applied | _________ | _________ | |
| §4.3 verification curls all green | _________ | _________ | |
| §5 rollback NOT required (or applied) | _________ | _________ | |

After sign-off, this runbook flows back to project-knowledge
cluster Task via inbox; PK.4 closes; the cluster cleanup-log
records the production-enable date.
