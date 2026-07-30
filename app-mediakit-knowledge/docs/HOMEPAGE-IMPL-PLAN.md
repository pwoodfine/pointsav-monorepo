# HOMEPAGE-IMPL-PLAN — documentation.pointsav.com home page (iteration 1)

> Implementation scoping for the Master 2026-04-28T22:40:00Z engine-spec
> relayed from project-language. Operator framing: "ship it and look at
> it." Iteration 1 is directionally right + shippable, not perfect.
>
> Two open questions answered in this Task's outbox 2026-04-28T23:50:00Z:
> Q1 = `index.md` (per content-contract.md §1, §2, §7);
> Q2 = `featured-topic.yaml` at content-wiki-documentation repo root.
>
> This document is the implementation scoping for the engine work that
> follows. First implementation pass scheduled for the next Task session.

Last updated: 2026-04-28.

---

## 1. What changes

The `index()` handler at `src/server.rs:203` currently renders a flat
file-listing chrome. Iteration 1 replaces it with a Wikipedia-Main-Page-
shaped home: lede, by-category 3×3 panel grid, featured-TOPIC slot
above the grid, and a recent-additions feed sorted by `last_edited:`.

If `index.md` is absent from `<content-dir>` the handler falls back to
the current placeholder chrome so a fresh content directory does not
500.

## 2. File touches

| File | Change | Lines (estimate) |
|---|---|---|
| `src/server.rs` | Replace `index()` body; add `read_featured_topic`, `bucket_topics_by_category`, `recent_topics_by_last_edited`; render new chrome (panel grid, featured slot, recent feed) | ~250 added |
| `src/render.rs` | Promote `category:` and `last_edited:` from `extra` to first-class `Frontmatter` fields (cleaner than `extra` lookup at every render); keep backwards compat — existing TOPICs without these fields default to `category: None`, `last_edited: None` | ~20 modified |
| `src/lib.rs` | (none — module map unchanged) | 0 |
| `static/style.css` | Add `.wiki-home-grid`, `.wiki-home-featured`, `.wiki-home-recent` styling; 3-col CSS grid with mobile breakpoint to 2-col → 1-col | ~120 added |
| `tests/integration_test.rs` (new or extended) | Fixture `index.md` + `featured-topic.yaml` + 9 fixture TOPICs across 3 categories; assert grid renders, featured panel renders, recent feed sorts correctly | ~150 added |

Single commit unit; no Phase 4 dependencies pulled forward.

## 3. Frontmatter struct change

```rust
#[derive(Debug, Default, Deserialize)]
pub struct Frontmatter {
    // ... existing fields ...

    /// Category for home-page bucketing per
    /// content-wiki-documentation/.claude/rules/content-contract.md §4.
    /// `root` reserved for index.md; one of 9 ratified categories per
    /// naming-convention.md §10 Q5-A for everything else.
    #[serde(default)]
    pub category: Option<String>,

    /// Last meaningful edit. YYYY-MM-DD. Drives recent-additions feed
    /// on the home page; fallback is git-commit-date via shell-out
    /// when absent.
    #[serde(default)]
    pub last_edited: Option<String>,

    // ... extra: BTreeMap stays for forward-compat ...
}
```

Backwards compatible. Existing TOPICs without these fields render
identically; only the home-page rendering uses the new fields.

## 4. Routing

```rust
async fn index(State(state): State<Arc<AppState>>) -> Result<Markup, WikiError> {
    let home_path = state.content_dir.join("index.md");
    if !home_path.exists() {
        return placeholder_index(&state).await;  // current behaviour
    }
    let home_text = fs::read_to_string(&home_path).await?;
    let home_parsed = parse_page(&home_text)?;
    let buckets = bucket_topics_by_category(&state.content_dir).await?;
    let featured = read_featured_topic(&state.content_dir, &buckets).await;
    let recent = recent_topics_by_last_edited(&buckets, 5);
    let home_html = render_html_raw(&home_parsed.body_md);
    Ok(home_chrome(&home_parsed.frontmatter, &home_html, featured, &buckets, &recent))
}
```

Placeholder `placeholder_index` is the current `index()` body extracted
verbatim — preserved for the absent-`index.md` case.

## 5. Category set (operator-ratified 2026-04-28)

Nine categories per `naming-convention.md` §10 Q5-A:

```
architecture
services
systems
applications
governance
infrastructure
company
reference
help
```

Plus `root` for `index.md` itself (not rendered as a panel; suppressed
from bucketing).

Engine renders all 9 panels at launch. Categories with zero articles
render with the placeholder text "0 articles — in preparation" rather
than being suppressed (per spec §"Iteration 1 MUST features").

## 6. Featured TOPIC pin

`featured-topic.yaml` at content-wiki-documentation repo root. Schema:

```yaml
slug: <topic-slug>      # required; must resolve to an existing TOPIC
since: <YYYY-MM-DD>     # optional; for engine-side rotation telemetry
note: <one-line>        # optional; engine ignores; human-readable cue
```

Engine reads once per request (no cache for iteration 1; revisit if
home-page latency becomes a concern).

| File state | Engine behaviour |
|---|---|
| Absent | Suppress featured panel (no warning) |
| Present, `slug:` field missing or empty | Suppress + log warning |
| Present, `slug:` doesn't match a TOPIC | Suppress + log warning |
| Present + valid | Render featured panel above the by-category grid |

Featured panel content: title from TOPIC frontmatter, one-line lede
auto-extracted from the first paragraph of body Markdown (or a future
`featured_lede:` frontmatter field on the TOPIC), `→ Read` link.

## 7. Recent-additions feed

Top-5 TOPICs sorted by `last_edited:` descending. Tiebreaker: filename
ascending.

`last_edited:` absent → fall back to `git log -1 --format=%cI -- <path>`
via `std::process::Command`. Iteration 1 uses shell-out (small,
pre-Phase-4-acceptable). Iteration 2+ migrates to the `git2` dependency
that lands in Phase 4 Step 4.1.

If neither `last_edited:` nor git-commit-date is available (fresh
file, never committed) → falls back to filesystem mtime.

## 8. Test plan

Fixture content dir at `tests/fixtures/home/` with:
- `index.md` (frontmatter `category: root`, lede + 9 category-card
  Markdown placeholders + ENGINE comments)
- `featured-topic.yaml` pointing to `topic-architecture-three-layer-stack`
- 9 fixture TOPICs spread across 3 categories (3 × `architecture`,
  3 × `services`, 3 × `governance`); `last_edited:` set for sort-order
  determinism

Tests:
- `home_renders_with_index_md_present` — assert h1, lede, all 9 panels,
  featured slot, top-5 recent
- `home_falls_back_to_placeholder_when_index_md_absent` — fixture without
  index.md
- `featured_topic_yaml_absent_suppresses_panel` — fixture without yaml
- `featured_topic_yaml_unresolvable_slug_suppresses_panel` — yaml with
  bad slug
- `recent_feed_sorts_by_last_edited_desc` — explicit sort order check
- `category_with_zero_articles_renders_placeholder` — bucket has empty
  category that still appears in grid

## 9. Out of scope for iteration 1

Per spec:
- Spanish home routing (`/es` → `index.es.md`) — iteration 2
- Search-box on home — iteration 2
- `/wanted` page — iteration 2
- Featured-TOPIC rotation logic — iteration 2 (single-pin sufficient)
- Date-tagged announcements panel — iteration 2
- Did You Know? / On This Day — never (cut by spec)

## 10. Cross-cluster contract

| project-language commits | project-knowledge implements |
|---|---|
| `category:` frontmatter on all new TOPICs | `category:` parsing + 9-bucket grouping |
| `last_edited:` frontmatter | Recent-additions sort by `last_edited:` |
| `featured-topic.yaml` at repo root with `slug:` | Read featured pin; suppress on absent |
| `index.md` (and `index.es.md`) | Home-file routing + render |
| Wikilinks via `[[slug]]` per content-contract.md §5.1 | (already handled by render pipeline) |

## 11. Sequencing

1. Frontmatter struct extension (small; isolated)
2. `read_featured_topic` + `bucket_topics_by_category` + `recent_topics_by_last_edited` helpers (small; pure)
3. `home_chrome` markup function (medium; HTML structure + maud)
4. `index()` handler swap (small; wires the above)
5. Style additions (medium; CSS grid + mobile breakpoints)
6. Tests (medium; fixture + assertions)
7. Single commit, single PR-shaped unit, on `cluster/project-knowledge`

## 12. After this iteration ships

- Stage `component-home-grid` DESIGN draft to
  `~/Foundry/clones/project-knowledge/.claude/drafts-outbound/` for
  project-design pickup per cluster-design-draft-pipeline.md (claim #38).
- Surface in outbox for project-language: 4 backfill DESIGN drafts
  (article-shell, citation-popover, bilingual-toggle, edit-pencil,
  search-results) — opt-in priority; not blocking.
- Operator + readers look at the live page. Iteration 2 driven by what
  surfaces.

## 13. Risk + mitigation

| Risk | Mitigation |
|---|---|
| `bucket_topics_by_category` walks the entire content dir on every `/` request — O(N) parse cost | Iteration 1 acceptable (49 TOPICs today); add a 60s in-memory cache in iteration 2 if needed |
| `last_edited:` shell-out adds startup cost / per-request cost | Cache git-commit-dates in the same iteration-2 cache |
| Spec change between iterations 1 → 2 (e.g. operator decides Did You Know? after seeing the page) | Acceptable; "ship it and look at it" framing is exactly designed for this |
| `index.md` lands in content-wiki-documentation before this engine pass ships | Fallback to placeholder is preserved; site continues serving — just doesn't render the new home until binary is rebuilt + redeployed |

## 14. Done criteria

- All 6 MUST features render correctly against the fixture
- All 6 unit + integration tests pass
- Cargo build + clippy clean
- Production deployment (separate Master pass) renders the new home at
  documentation.pointsav.com against the live `index.md` once
  project-language commits the refined draft
