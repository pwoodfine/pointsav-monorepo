---
schema: foundry-draft-v1
state: draft-pending-language-pass
originating_cluster: project-data
target_repo: woodfine-fleet-deployment
target_path: vault-privategit-source/
target_filename: GUIDE-fs-anchor-emitter.md
audience: vendor-internal
bcsc_class: current-fact
language_protocol: PROSE-GUIDE
authored: 2026-04-28T00:00:00Z
authored_by: task-project-data (session e509c13609b4b632)
authored_with: opus-4-7
references:
  - ~/Foundry/clones/project-data/service-fs/anchor-emitter/src/main.rs
  - ~/Foundry/infrastructure/local-fs-anchoring/  # Master-shipped IaC
  - ~/Foundry/conventions/worm-ledger-design.md
  - ~/Foundry/DOCTRINE.md  # §II.7 Invention #7
  - https://blog.sigstore.dev/rekor-v2-ga/
  - https://github.com/sigstore/rekor-tiles
notes_for_editor: |
  Operational runbook for the fs-anchor-emitter binary (Doctrine
  Invention #7 monthly Sigstore Rekor anchoring). Master shipped
  the IaC at workspace v0.1.27 (binary at /usr/local/bin/fs-anchor-
  emitter, system user local-fs-anchor, systemd unit + monthly
  timer ARMED). This GUIDE belongs in the vault-privategit-source/
  catalog subfolder of woodfine-fleet-deployment because the
  workspace VM IS vault-privategit-source-1 (per Foundry
  MANIFEST.md), so this is the operator-runbook for the PointSav-
  tenant instance.

  GUIDEs are English-only per CLAUDE.md §14 (operational; not for
  public-facing distribution). No bilingual pair needed.

  Status note: at draft-authored time (2026-04-28), the binary
  installed at /usr/local/bin is at commit 6262d10 — pre-fix.
  Two follow-up commits are pending (one staged, one unstaged):
    - fc03e57 — REKOR_URL configurable (committed in cluster repo;
      Master pending rebuild + redeploy)
    - PD.1 body-shape upgrade v0.0.1 → v0.0.2 (pre-authored
      unstaged in cluster repo at draft time; ships when chmod
      blocker clears)
  After both ship, the May 1 02:40:38 UTC monthly fire will
  succeed against log2025-1.rekor.sigstore.dev. This GUIDE
  documents the post-fix steady state; today's binary cannot
  successfully anchor (returns exit 3 — Rekor 404 / 400).
  Project-language should pare the version-specific notes; the
  steady-state operator interface is what the runbook owner reads.

  Three required sections per PROSE-GUIDE genre template (per
  language-protocol-substrate §2.2):
    1. What it does + when it runs (current state)
    2. How to operate it (commands, env vars, exit codes)
    3. How to recover from each failure mode

  Bilingual pair: NONE. GUIDEs are English-only operational; do
  not generate .es.md per CLAUDE.md §14.

  Citations to resolve to citation-substrate IDs:
    - Sigstore Rekor v2 → sigstore-rekor-v2
    - Sigstore Rekor v2 GA blog → sigstore-rekor-v2-ga-blog
    - DOCTRINE Invention #7 → doctrine-invention-7
    - rekor-tiles CLIENTS.md → rekor-tiles-clients-md
---

# GUIDE — fs-anchor-emitter operator runbook

`fs-anchor-emitter` is the small Rust binary that posts the workspace's monthly per-tenant `service-fs` checkpoint to Sigstore Rekor v2 and writes the returned tlog entry back into the same tenant's WORM ledger. It implements Doctrine Invention #7 (Sigstore Rekor monthly anchoring) — the public-verifiability layer above the private per-tenant ledger.

This document is the operator runbook: what it does, how to invoke it, how to read its exit codes, how to recover from each failure mode, and how to swap the active Rekor v2 shard URL when the year-shard rotates.

## 1. What it does and when it runs

Once per month, on the 1st at 02:30 UTC plus a randomized 0-15 minute jitter:

1. The systemd timer `local-fs-anchor.timer` fires.
2. `local-fs-anchor.service` runs the binary as the unprivileged system user `local-fs-anchor`.
3. The binary GETs `${FS_ENDPOINT}/v1/checkpoint` from the workspace's `service-fs` instance, fetching the latest signed checkpoint for tenant `${FS_MODULE_ID}`.
4. The binary wraps the checkpoint JSON as a Sigstore `hashedRekordRequestV002` entry: SHA-256 of the canonical checkpoint bytes, signed with an ephemeral Ed25519 keypair generated for this run. The keypair is single-use; the value being anchored is the Rekor inclusion proof + timestamp, not the key identity, so ephemeral is correct.
5. The binary POSTs to `${REKOR_URL}` (default `https://log2025-1.rekor.sigstore.dev/api/v2/log/entries`).
6. The binary writes the returned tlog entry back into `service-fs` via `POST ${FS_ENDPOINT}/v1/append` with `payload_id: anchor-rekor-<unix-ts>`. The anchor itself becomes part of the per-tenant ledger.
7. The binary exits.

Steady state: one anchor per tenant per month. The workspace anchor stream is independent per tenant — Customer Toteboxes get their own `local-fs-anchor.service` instance pinned at their `FS_LEDGER_ROOT`, anchoring on the same monthly cadence with the same binary.

## 2. Configuration surface

Three environment variables. Configured in the systemd unit's `[Service]` section.

| Variable | Required | Default | Purpose |
|---|---|---|---|
| `FS_ENDPOINT` | Yes | — | Base URL of the local `service-fs` daemon. On the workspace VM: `http://127.0.0.1:9100`. |
| `FS_MODULE_ID` | Yes | — | Per-tenant moduleId. Workspace VM: `foundry-workspace`. Customer Totebox: the tenant's assigned module ID (per the deployment's `MANIFEST.md`). |
| `REKOR_URL` | No | `https://log2025-1.rekor.sigstore.dev/api/v2/log/entries` | Active Rekor v2 shard endpoint. Override here when the annual shard rotation lands (see §5). |

Setting any of `FS_ENDPOINT` / `FS_MODULE_ID` to a wrong value will surface as exit code 1 (config error) or exit code 2 (checkpoint fetch failed — 403 if moduleId mismatches the daemon's `FS_MODULE_ID`).

## 3. Manual invocation (smoke test)

The systemd unit fires monthly automatically. Manual invocation is for smoke testing after a deploy or after changing `REKOR_URL`.

```
sudo systemctl start local-fs-anchor.service
sudo journalctl -u local-fs-anchor.service -e --no-pager | tail -40
```

A successful run writes the tlog entry, prints `anchor emitted successfully` to stdout, and exits 0. The entry is now visible in the tenant's `service-fs` ledger:

```
curl -s -H "X-Foundry-Module-ID: foundry-workspace" \
  http://127.0.0.1:9100/v1/entries?since=0 \
  | jq '.entries[] | select(.payload_id | startswith("anchor-rekor-"))'
```

The Rekor inclusion proof in that entry is independently verifiable via Sigstore's verification clients against the public log shard.

## 4. Exit codes and recovery

Five exit codes. Each maps to one observable failure surface; the recovery path differs per failure.

### Exit 0 — success

Normal path. Tlog entry written; anchor record present in the per-tenant ledger. No action required.

### Exit 1 — config error

`FS_ENDPOINT` or `FS_MODULE_ID` missing from the environment. The binary writes the missing variable name to stderr.

Recovery: inspect `local-fs-anchor.service` `[Service]` section for `Environment=` lines. Ensure both required variables are set. `systemctl daemon-reload && systemctl start local-fs-anchor.service` to retry.

### Exit 2 — checkpoint fetch failed

The GET to `${FS_ENDPOINT}/v1/checkpoint` failed. Three subcases visible in stderr:

- **Connection refused** — `service-fs` is not running at `FS_ENDPOINT`. Check `systemctl status local-fs.service`.
- **403 Forbidden** — `FS_MODULE_ID` does not match the daemon's `FS_MODULE_ID`. The X-Foundry-Module-ID header check failed. Verify both env vars name the same tenant.
- **Other HTTP status** — daemon is reachable but returning errors. Inspect `journalctl -u local-fs.service` for the daemon's view.

Recovery: fix the underlying issue (start the daemon, correct the moduleId, etc.), then `systemctl start local-fs-anchor.service` to retry. If the timer-driven monthly fire failed and the cause is fixed within the month, `Persistent=true` in the timer unit catches up automatically on the next start.

### Exit 3 — Rekor submission failed

The POST to `${REKOR_URL}` failed. Two distinct subcases:

- **404 Not Found from `rekor.sigstore.dev`** — the legacy public host serves only Rekor v1 at `/api/v1/log/entries`. v2 endpoints return 404 here. This is a configuration drift; the active Rekor v2 shard URL should be `https://log2025-1.rekor.sigstore.dev/api/v2/log/entries` (or the current shard per §5). Set `REKOR_URL` correctly in the unit.
- **400 Bad Request from a v2 shard** — the request body shape does not match `hashedRekordRequestV002`. The binary should be at the post-PD.1 version (body shape upgraded from v0.0.1 to v0.0.2). If this surfaces, the deployed binary is stale; rebuild from `cluster/project-data` HEAD and reinstall.
- **Network error** — Sigstore unreachable. Defer the anchor; the next monthly fire (or manual retry after the network heals) catches up.

Recovery: see §5 for shard URL rotation; for stale binary, `cd /srv/foundry/clones/project-data/service-fs/anchor-emitter && cargo build --release && sudo install -o root -g root -m 0755 target/release/fs-anchor-emitter /usr/local/bin/`.

### Exit 4 — service-fs append of anchor record failed

Rekor returned a tlog entry successfully but the write-back to `${FS_ENDPOINT}/v1/append` failed. Same recovery surface as exit 2.

This is the unhappy outcome where the anchor exists in the public Rekor log but is not recorded in the tenant's local ledger. The anchor remains independently verifiable via Rekor (the inclusion proof is publicly fetchable by checkpoint hash); the local copy is missing. Manual fix: query Rekor for the entry by checkpoint hash, format as `{"payload_id": "anchor-rekor-<unix-ts>", "payload": <tlog_entry>}`, POST to `service-fs /v1/append` directly. Or, simpler: rerun the anchor emitter for the same checkpoint — it generates a fresh ephemeral keypair, produces a second Rekor entry, and writes both anchor records into the tenant ledger. Two anchors for the same checkpoint is harmless (both attest to the same root hash + tree size).

## 5. Annual Rekor shard rotation

Sigstore deploys Rekor v2 on year-sharded hosts: `logYEAR-rev.rekor.sigstore.dev`. The current shard is `log2025-1.rekor.sigstore.dev`, live since 2025-10-06 per the [Rekor v2 GA blog post](https://blog.sigstore.dev/rekor-v2-ga/). When Sigstore deploys `log2026-1` (announcement expected late 2025 / early 2026 per the same blog post), the 2025 shard will be sunsetted on a published timeline.

When the rotation happens:

1. Verify the new shard host is live: `curl -s -o /dev/null -w "%{http_code}\n" https://log2026-1.rekor.sigstore.dev/` (expect 200).
2. Edit the `local-fs-anchor.service` unit's `[Service]` block, set:
   ```
   Environment=REKOR_URL=https://log2026-1.rekor.sigstore.dev/api/v2/log/entries
   ```
3. `sudo systemctl daemon-reload`
4. Optional smoke: `sudo systemctl start local-fs-anchor.service` and verify `journalctl -u local-fs-anchor.service -e` shows exit 0.

No binary rebuild is required for the URL swap; the binary reads `REKOR_URL` from the environment at every invocation.

The long-term-correct pattern per Sigstore documentation is TUF-based discovery of the active shard URL via Sigstore's Update Framework repository (the shard URL is distributed via SigningConfig). That requires adding a TUF client to the binary (`tough` crate or equivalent) and resolving a TUF trust-root bootstrap problem. Tracked as a Foundry follow-up item conditional on the operator's key-custody decision per `apprenticeship-substrate.md` §6.

## 6. Provenance and disposal

The binary is built reproducibly from `cluster/project-data` HEAD in the project-data cluster. Each release bumps the cluster `Version:` tag per CLAUDE.md §7. The deployed binary at `/usr/local/bin/fs-anchor-emitter` is replaced (not amended) on each redeploy; the prior binary is overwritten by `install`.

The systemd state directory at `/var/lib/local-fs-anchor/` holds no persistent state today (the binary writes its anchor record directly to the tenant's `service-fs` ledger and exits). The directory exists for future expansion (per-run logs, key state if signed-checkpoint custody lands, etc.).

To decommission the anchor stream for a tenant:

1. `sudo systemctl disable --now local-fs-anchor.timer`
2. `sudo systemctl stop local-fs-anchor.service` (no-op if not currently running; oneshot units don't linger)
3. Optionally remove the unit files from `/etc/systemd/system/` and `daemon-reload`.

Already-anchored entries remain valid in Rekor and in the tenant's ledger. Decommissioning stops new anchors only; it does not invalidate prior ones.

## 7. References

- `~/Foundry/clones/project-data/service-fs/anchor-emitter/src/main.rs` — binary source.
- `~/Foundry/infrastructure/local-fs-anchoring/` — Master-shipped systemd unit + bootstrap.sh.
- `~/Foundry/conventions/worm-ledger-design.md` §5 step 6 (anchoring) — design convention.
- `~/Foundry/DOCTRINE.md` §II.7 — Doctrine Invention #7.
- Sigstore Rekor v2 GA: https://blog.sigstore.dev/rekor-v2-ga/
- rekor-tiles CLIENTS.md: https://github.com/sigstore/rekor-tiles/blob/main/CLIENTS.md
- TOPIC: Foundry's WORM Ledger Substrate (companion architectural overview; refines as `topic-worm-ledger-architecture.md`).
