---
schema: foundry-doc-v1
document_version: 0.1.0
title: Hello, Substrate
slug: topic-hello
language: en
forward_looking: false
disclosure_class: narrative
hatnote: "For the deployment guide, see the ARCHITECTURE.md document in the crate root."
translations:
  es: topic-hello.es
categories:
  - Phase 1 fixtures
  - Wiki engine
---

# Hello

This is a Phase 1 fixture page used to validate the wiki engine
end-to-end. It exercises:

- A frontmatter block with the canonical schema
- A heading
- A paragraph with a [[Other Topic]] wikilink
- A GFM table

## Wikilinks

The renderer treats `[[Page Name]]` as an internal link. The link
target is the slug; resolution rules and red-link styling for
unknown targets land in Phase 6.

## A small table

| Phase | Adds |
|---|---|
| 1 | render |
| 2 | edit |
| 3 | search |
| 4 | git sync |
| 5 | auth |
| 6 | wikilinks resolution |
| 7 | federation seam |
| 8 | disclosure mode |

See `ARCHITECTURE.md` in the crate root for the full plan.
