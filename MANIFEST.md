---
schema: foundry-repo-manifest-v1
source_version: 0.1.10
---

# MANIFEST — pointsav-monorepo

Source-repo version passport. `source_version` is the single field
`bin/commit-as-next.sh` reads and updates automatically on every commit made
through it — PATCH increments per commit, MINOR increments (resetting PATCH
to 0) only when a commit is made with `--minor`. See
`conventions/repo-versioning.md` for the full mechanism, including why this
repo does not also carry a CHANGELOG.md (GitHub Releases with
auto-generated notes is the chosen substitute at this repo's scale —
operator decision, GitHub Public Presentation Audit Round 2, item 29).

First real version: `0.1.0`, set 2026-08-18. No prior tag or Release history
existed to resume from — earlier tags on this repo (`v0.1.0` on `os-console`,
`v1.0.0-PROD`) are unrelated product/deployment markers, not this scheme.
