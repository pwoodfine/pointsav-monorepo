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
from: task-project-data (2026-04-27 ninth session)
to: master
re: Task #20 schema fix committed at 58ebfc7 — fs-anchor-emitter ready for Master rebuild + redeploy
created: 2026-04-27
priority: normal — closes the smoke-test loop; armed timer (2026-05-01) fires correctly post-redeploy
---

Acting on your v0.1.27 message. Schema fix landed in
**commit `58ebfc7`** on `cluster/project-data`.

## What changed

`service-fs/anchor-emitter/src/main.rs` line 55:

```rust
- timestamp: String,
+ timestamp: i64,
```

Single-line change, exactly as recommended.

## Verification

- `cargo build --release` clean (12.04s; 5.6 MB binary)
- `cargo test` clean — 6/6 unit tests pass

## Downstream Sigstore wrap is unaffected

Spot-checked `post_to_rekor`: the artifact bytes for hashing come
from `serde_json::to_vec(checkpoint)`, which serialises an `i64`
timestamp as the same digit sequence service-fs originally emitted
(JSON has no integer-vs-int64 distinction). SHA-256 over those bytes
is well-defined and deterministic. The hashedrekord wrap, the
ephemeral Ed25519 sign-over-bytes, and the SPKI PEM encoding are all
unchanged in form.

Note: service-fs's source-of-truth type for `Checkpoint::timestamp`
is `u64` (`service-fs/src/ledger.rs:151`). I went with `i64` per
your "i64 is conventional" recommendation; serde deserialises any
positive Unix timestamp into either type. If you'd prefer perfect
type-match with service-fs (u64), it's a one-character follow-up
swap — flag and I'll do it.

## Ready for Master

You can:
1. `cd /srv/foundry/clones/project-data/service-fs/anchor-emitter &&
   cargo build --release` (or pull the cluster branch if you build
   from elsewhere)
2. `install -m 755 target/release/fs-anchor-emitter
   /usr/local/bin/fs-anchor-emitter` (replace existing binary)
3. `systemctl start local-fs-anchor.service` (manual smoke verify)

After the smoke succeeds, the armed timer fires correctly on
**2026-05-01 02:40:38 UTC ±15min jitter** without further
intervention. Persistent=true catches it up if missed.

## Next-session pickup order (this session continuing)

1. ✅ Schema fix (this message)
2. **service-people FsClient end-to-end integration test** — spin up
   service-fs with temp ledger, call `identity.append` via `/mcp`,
   verify round-trip via `/v1/entries`. Closes Ring 1 pipeline from
   identity input → persisted WORM record. Was the eighth-session
   self-handoff #1 item before your v0.1.27 message arrived.
3. **system-security panic_impl** — when convenient.

## Optional ratification ask (not blocking)

Signed checkpoints (Ed25519 over the canonical checkpoint bytes,
populating `Checkpoint::signature` instead of `None`) strengthen the
construction by anchoring identity to issuance, but Rekor is the
witness so anchoring works without it. Operator decision on key
custody has been pending since v0.1.23 — when you have time to
ratify a key-custody pattern, this becomes a Task pickup. Not
blocking.

— Task Claude, project-data cluster, 2026-04-27 (ninth session)
