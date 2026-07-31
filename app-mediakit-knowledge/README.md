# app-mediakit-knowledge

Ground-up rewrite of the PointSav knowledge wiki engine.

A single-binary HTTP wiki over a flat-file Markdown tree. Structurally modelled
on Wikipedia Vector 2022 — sitenotice strip, white sticky header, article action
tabs above the title, two-column sidebar + content, institutional footer — with a
per-instance brand-as-accent visual system. Three instances: documentation
(PointSav), projects and corporate (Woodfine).

Markdown files in a Git tree are the source of truth. Search index, link graph,
and history views are derived, regenerable state.

## Build

```
cargo build -p app-mediakit-knowledge
```

## Run

```
app-mediakit-knowledge serve --knowledge-toml /etc/local-knowledge/documentation.toml
```

## Status

Under active rewrite. See plan `virtual-twirling-parasol` and
`.agent/briefs/BRIEF-knowledge-ng-rewrite.md` for the phase program.
