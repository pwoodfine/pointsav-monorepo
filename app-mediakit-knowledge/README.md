# app-mediakit-knowledge

Wikipedia-pattern HTTP knowledge wiki for `os-mediakit`. Serves the
`content-wiki-documentation` repository as a fully navigable wiki at
`documentation.pointsav.com`. Built in Rust. No database. No runtime
dependencies beyond the compiled binary.

<div align="center">

[ 🇪🇸 Leer este documento en Español ](./README.es.md)

</div>

The wiki engine for the PointSav knowledge platform. A single Rust
binary that serves a directory of CommonMark-with-wikilinks files
as a Wikipedia-shaped read-and-edit surface.

## Status

Phases 1 through 5 core shipped and running in production at
`documentation.pointsav.com`. See [`ARCHITECTURE.md`](./ARCHITECTURE.md)
for the full phase plan.

## Design principle

**Markdown files in a Git tree are the source of truth.** Every
database, index, and cache is derived state, rebuildable from
`git checkout && reindex`. Page identity is a path; revision
history is `git log`; metadata is YAML frontmatter; multi-content
slots are sibling files. There is no schema migration ladder
because there is no canonical schema in the database — the
database is a regenerable index of the file tree.

## Run

```
cargo run -- serve --content-dir <path-to-content-wiki-documentation>
```

The server binds `127.0.0.1:9090` by default. Override with
`--bind` or `WIKI_BIND`. Build the release binary with
`cargo build --release` inside this directory (not from the monorepo
root — workspace coupling with `service-content` requires crate-local
build).

## Build phases

| Phase | Adds | Status |
|---|---|---|
| 1 | render — GET /wiki/{slug}, /static/, /healthz | shipped |
| 1.1 | Wikipedia chrome — tabs, TOC, hatnote, language switcher | shipped |
| 2 | edit — CodeMirror 6, JSON-LD, atomic save, SAA squiggles, citation autocomplete | shipped |
| 3 | search + feeds — Tantivy BM25, Atom, JSON Feed, sitemap, llms.txt | shipped |
| 4 | Git sync + MCP — git2, history/blame/diff, redb wikilink graph, blake3, native MCP JSON-RPC 2.0, OpenAPI 3.1 | shipped |
| 5 core | auth + edit review — cookie sessions, argon2id, edit review queue | shipped |
| 5.1 | bilingual /es/ routing — Spanish home + articles, EN fallback, Accept-Language redirect, hreflang | shipped |
| 5.2+ | per-page ACLs, OIDC SSO, webhook subscriptions, AsyncAPI 3.1 | planned — gated on BP5 |
| 6 | wikilink resolution + portable identity | planned |
| 7 | federation seam (blake3 content addressing + ActivityPub) | planned |
| 8 | disclosure mode + cryptographic timestamping | planned |

Phase 8 is the intended moat — Markdown-native authorship, structured-data
extraction for regulator-required financial-statement blocks, cryptographic
timestamping, and per-jurisdiction export adapters. Forward-looking; subject
to material assumptions and operator decisions.

## Cluster context

This crate is part of the `project-knowledge` cluster
(per `~/Foundry/PROJECT-CLONES.md`), alongside:

- [`content-wiki-documentation`](../../../content-wiki-documentation/) —
  TOPIC content the engine renders
- [`pointsav-fleet-deployment/media-knowledge-documentation/`](../../../pointsav-fleet-deployment/) —
  catalog runbooks for deployment

## Conventions

- Single-binary deployment, systemd unit, no Docker
  (per `conventions/zero-container-runtime.md`)
- Bilingual READMEs (per workspace `CLAUDE.md` §6)
- Citation discipline (per `conventions/citation-substrate.md`)
- BCSC continuous-disclosure posture for content
  (per `conventions/bcsc-disclosure-posture.md`)

## Licence

Apache-2.0.
