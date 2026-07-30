---
schema: foundry-doc-v1
document_version: 0.1.0
component: app-mediakit-knowledge
status: thinking — UX synthesis from session-2 research; Phase 1 chrome additions + Phase 2 editor stack pending implementation
last_updated: 2026-04-26
session: 2
authoring_context: synthesised from four parallel research-agent reports commissioned 2026-04-26 in response to operator's "we want an original copy of Wikipedia, capture the muscle memory, find the leapfrog 2030 solution" framing
---

# UX design — leapfrog 2030 wiki engine

The reading and editing surfaces for `app-mediakit-knowledge`. Two
inventions that ride on the engineering substrate documented in
[`ARCHITECTURE.md`](../ARCHITECTURE.md) and the strategic substrate
inventions in [`INVENTIONS.md`](INVENTIONS.md):

- **IVC** — Inline Verifiable Citations (reading)
- **SAA** — Substrate-Aware Authoring (editing)

Both honour a single contract: **Vector 2022's design rule applies
— every upgrade ships as additive, never removal.** The Wikipedia
muscle-memory inventory in §1 is sacred; the 5% headroom in §3 is
where the leapfrog happens.

## 0. The verdict in one line

**Wikipedia-grade reading + IDE-grade editing + Perplexity-grade
citation transparency, with all three properties stored as
first-class graph data rather than rendering tricks.** No surveyed
incumbent ships this combination.

## 1. Wikipedia muscle-memory inventory — the 18 sacred patterns

Eighteen specific UX patterns trained ~2 billion monthly readers
across two decades. Violating any of them gives the substrate
substitute the "61st knockoff" problem. Source: Agent-1 deep-dive
on Wikipedia UX 2026, including the Vector 2022 redesign data and
the eye-tracking / click-pattern research synthesised at
[Research:Which parts of an article do readers read](https://meta.wikimedia.org/wiki/Research:Which_parts_of_an_article_do_readers_read).

1. **Article / Talk tab pair** — top-left of title row
2. **Read / Edit / View history tabs** — top-right of title row, in that order
3. **Per-section `[edit]` pencils** — right-floated on every heading
4. **Footnote convention** — bracketed superscript `[n]` → References list with `^` back-arrows
5. **End-of-article ordering** — *See also → Notes → References → Further reading → External links → Categories* (per WP:Manual of Style/Layout)
6. **Hatnote** — italic, indented, top of article, above the infobox in source order
7. **Infobox right-rail** — right-aligned summary card; Agent 1 cites: 4% of in-article links → 18% of all wikilink clicks
8. **Lead first-sentence convention** — bolded subject + copula + one-line definition
9. **"From Wikipedia, the free encyclopedia"-equivalent tagline** — under the title (Vector 2022 moved to a "tagline" element)
10. **Link colours** — blue (unvisited) / purple (visited) / red (missing target)
11. **Body typography** — serif (Charter or equivalent), ~16px, ~960px max-width, 1.6 line-height
12. **Collapsible left-rail TOC** — follows the reader on scroll (Vector 2022's biggest visible change)
13. **Centre-top search** — with autocomplete and image previews
14. **Language switcher** — as a button next to the title, not buried in the sidebar
15. **Footer** — categories → license notice → About / Disclaimers / Contact / Cookie statement
16. **Mobile chrome** — hamburger top-left, sections collapsed by default, infobox stacked under lead, hatnote hoisted above infobox
17. **Talk-page replies** — indent-by-quote-block, signed timestamps, permalink-per-comment
18. **Edit-conflict screen** — diff-based reconciliation surface (the underlying merge can be smarter; the surface stays familiar)

The Vector 2022 redesign — community RfC voted **165 oppose / 153
support** but the close found rough consensus and the deployment
proceeded — preserved every item on this list and added affordances
to it. The MDPI 2025 study found the redesign drove pageviews +1.25%
and internal link clicks +1.06M monthly with no significant external
referral disruption. **Additive succeeds; removal does not.**

## 2. Where existing alternatives fail — the 60% knockoff failure modes

From Agent-2's mapping of the alternatives landscape (Wiki.js,
BookStack, Outline, DokuWiki, XWiki, Foswiki, Wikijump, Notion,
Obsidian, Logseq, Roam, Tana, Confluence, GitBook, mdBook, MDN,
Stack Overflow, etc.). Each knockoff fails in known, repeatable
ways. Substrate substitute MUST avoid:

| Failure mode | Where it bites | Avoid by |
|---|---|---|
| Replacing `<ref>` with bare Markdown links | Loses footnote-with-backlink; readers can't navigate from claim to source and back | Ship a citation primitive richer than wikitext, not poorer (see IVC §4) |
| TOC in slide-out drawer | Obscures structure; readers can't scan section list | Fixed left-rail TOC (Wikipedia pattern) — non-negotiable |
| Notion-block editor for long-form | Fights paragraph flow, citation insertion, references rendering | Source-faithful Markdown editor (see SAA §5) — block editors disqualified |
| JS bundle blocks first paint | Outline / Wiki.js mobile painful | Static-render reading view; JS for interactivity only |
| Mobile as media query | 95% of mobile editors make zero edits on Wikipedia | Mobile-from-the-ground-up; the editor that works on touch IS the substrate's mobile-edit win |
| Search worse than CirrusSearch | Universal across alternatives | Tantivy embedded; equal or beat MediaWiki+CirrusSearch |
| Broken math, tables, code, footnotes | nLab fights this daily | comrak + MathML/KaTeX + GFM tables + syntect — first-class from Phase 1 |
| AI as side-panel chatbot | Microsoft Viva Topics retired Feb 2025 specifically because of this complexity; Amazon Q UX criticised | AI as in-editor contributor (Cursor pattern, see SAA §5.2) — never a separate chat |
| Auth-gate read access | Outline does this; loses Wikipedia's anonymous-read gift | Anonymous read by default; auth only for write |
| Typography worse than Vector 2022 | Almost universal failure mode | 45–75 char line length, ≥17px body, ≥1.5 line height, real serif/sans choice — non-negotiable |

The cautionary tale is VisualEditor itself: even with the WMF
behind it, "experienced editors prefer the wikitext source editor
because it is faster, more precise, more full-featured, and less
buggy" (Wikipedia article on VisualEditor). The lesson — a "modern"
editor that loses fidelity loses adoption.

## 3. The 5% headroom — where the leapfrog happens

What Vector 2022 left room for, and what no surveyed alternative
ships:

- **Per-claim cryptographic verification badges** baked into the
  reading view — IVC (§4)
- **Real-time collaborative editing** via CRDT inside both
  rendering and source editor surfaces, with presence indicators,
  without changing the Edit-tab affordance
- **Citation-graph navigation** — sibling to "What links here":
  "what this cites / what cites this", powered by the substrate's
  content-addressed citation registry
- **Optional AI-assisted citation surfacing** at edit time only —
  never in the read path; brand promise is *deterministic
  encyclopedia*, AI is Ring 3 structurally optional
- **Mobile editor that actually works** — VisualEditor as the
  default with wikitext available behind a power-user toggle. The
  95% mobile-edit drop-off on Wikipedia is the single biggest win
  available to anyone shipping a credible substitute in 2026.
- **Semantic browse** beside category browse — "articles like
  this" / "concepts adjacent to this" surfaced from the citation
  graph, not from an LLM in the read path

Each upgrade ships as additive — readers who don't notice them
still get the Wikipedia experience they expect. Readers who attend
to them get something Wikipedia readers have never had.

## 4. IVC — Inline Verifiable Citations (reading)

The reading-side surface that makes substrate Inventions A, C, and
the citation registry visible to readers without becoming visual
noise.

### 4.1 The TLS padlock lesson — most important calibration

Per Agent-4 research: Chrome dropped the green padlock in May 2023
([Chromium Blog](https://blog.chromium.org/2023/05/an-update-on-lock-icon.html))
because **89% of users misread it as "this site is trustworthy"**
rather than "the connection is encrypted." When verification
becomes universal, the *positive* badge becomes noise; only the
*exception* deserves chrome.

Direct application to IVC: **default state is neutral grey, not
green.** Color reserved for exceptions. Shows all-verified-OK only
via reader toggle (see §4.5).

### 4.2 Glyph + placement

- **Glyph**: small **C2PA "CR" pin** in superscript at clause end,
  next to (not replacing) any existing footnote `[n]`. The C2PA
  Content Credentials mark is shipping on Adobe's Content
  Authenticity app and Pixel 10 Google Photos; literacy is growing;
  we don't invent a new symbol. Source:
  [C2PA official Content Credentials icon post](https://spec.c2pa.org/post/contentcredentials/)
  + [Adobe Content Authenticity app design story](https://adobe.design/stories/process/behind-the-design-adobe-content-authenticity-app).
- **Placement**: end of clause (not after every sentence; one per
  *claim* per the substrate's existing granularity).
- **Default colour**: neutral grey.
- **Exception colours**:
  - 🟡 yellow — source-drifted (cited URL still works but content
    has changed since publish)
  - 🔴 red — citation missing or hash mismatch
  - 🔵 blue — forward-looking-information (cautionary), per BCSC
    posture
- **Filled state** (✓ green) appears only when the reader has
  explicitly toggled "show all verified marks" (§4.5).

### 4.3 Hover preview (Distill.pub pattern)

200ms delay → small preview card pops next to the badge:

- Citation title
- Publisher
- Jurisdiction binding (if any)
- FLI label (if applicable)
- One-line status: *"Verified · anchored 2026-03-04 · source unchanged"*

Pattern lifted from [Distill.pub](https://distill.pub/guide/) —
numbered superscript + on-hover full-card preview. Distill chose
this because long inline citations (`Smith 2024`) clutter dense
prose; the hover-card delivers citation density without visual
collapse.

### 4.4 Click → inline expansion panel (Community Notes pattern)

Click expands a panel **directly below the paragraph** — never a
sidebar overlay (the Hypothes.is sidebar pattern was found to
"compete with the host page's design and get in the way" per
Critchlow 2019). Placement borrowed from [X/Twitter Community
Notes](https://en.wikipedia.org/wiki/Community_Notes), whose
under-the-claim block was extensively iterated for neutrality
(per [Asterisk on the making of Community Notes](https://asteriskmag.com/issues/08/the-making-of-community-notes)).

Panel contents:
- Registry entry ID + link to source URL
- Content hash at publish time (truncated `0xabcd…ef12` + copy button)
- Current-fetch hash + drift diff if any (see §4.6)
- OpenTimestamps Bitcoin anchor (block height + "verify
  independently" deep-link to a public OTS verifier)
- RFC 3161 TSA token if present
- FLI assumptions block if `forward_looking: true`
- Jurisdiction binding (e.g., NI 51-102, FCA Listing Rules)
- W3C VC signature status if Phase 9 CCA is active

Reader stays in the document throughout — no page navigation, no
modal dialog, no full-page redraw.

### 4.5 Page-level masthead band (BBC Verify pattern)

A single horizontal strip at the top of every TOPIC, just below the
title row. One summary line:

> *"23 claims · all verified · last drift-check 2h ago"*

This is where the *positive* signal lives without polluting prose.
Pattern lifted from [BBC Verify](https://en.wikipedia.org/wiki/BBC_Verify)
— branded text label appended to byline, not per-claim badges.
Reader gets the at-a-glance trust signal without 23 green
checkmarks bleeding into the body.

When drift or missing-citation issues exist, the band turns yellow
or red and the count breaks down: *"23 claims · 21 verified · 2
drifted · last drift-check 2h ago"*.

### 4.6 Reader density toggle — the one decision that answers most of the noise question

A single setting in the user preferences:

- **Off** — no IVC marks at all (the pure Wikipedia reading
  experience for readers who don't care)
- **Exceptions only (default)** — neutral grey marks visible at
  default density; coloured marks (yellow/red/blue) prominent
- **All** — shows verified ✓ marks too; for auditors, regulators,
  power-readers

Default is **Exceptions only**. This is the operationalisation of
the TLS padlock lesson at the substrate layer.

### 4.7 Adjacent invention — Diff-since-citation inline view

When a cited source has drifted (cited URL still works but content
has changed since publish), the IVC expansion panel surfaces a
one-line diff inline (publish-time excerpt vs current excerpt).
Borrowed from `git diff` and PolicyMap revision overlays.

Makes drift legible to a non-technical reader without showing them
a hash. Directly serves CLAUDE.md §6 rule 3 (material changes
surfaced).

**Structural advantage over both C2PA (proves provenance, not
currency) and BBC Verify (human-attested, not continuously
re-checked).** No incumbent ships continuous drift detection bound
to inline reading UX.

### 4.8 Adjacent invention — `verify://` URL scheme + side-loaded local verifier

Every IVC mark has an associated `verify://` URI containing the
citation ID + content hash + OTS proof. A reader who installs a
small local verifier (CLI or browser extension) can right-click any
IVC mark and run an *independent* end-to-end verification — registry
fetch, hash recomputation, OpenTimestamps Bitcoin-block check —
without trusting the publishing site at all.

Pattern modelled on [Sigstore's CLI-first verification
posture](https://docs.sigstore.dev/cosign/verifying/verify/) and
the `mailto:` URL scheme. Lesson from Certificate Transparency: **the
most credible verification is the one the reader's own software
performs.**

Existence of the local-verifier path materially raises credibility
of the inline marks even for readers who never install it. This is
how the substrate scales trust beyond its own infrastructure.

## 5. SAA — Substrate-Aware Authoring (editing)

The editing-side surface that makes the citation registry, FLI
discipline, and (Phase 9) CCA usable in practice. Per Agent-3
research, the stack converged decisively.

### 5.1 Editor stack

Per Agent-3's stress-test of CodeMirror 6 vs Monaco vs ProseMirror /
Tiptap / Lexical / BlockNote:

- **[CodeMirror 6](https://codemirror.net/)** — 300 KB tree-shaken
  vs Monaco's 5–10 MB monolith. Mobile and accessibility
  first-class. Adopted by Replit and Chrome DevTools — long-term
  bet is safe.
- **[`@codemirror/lang-markdown`](https://github.com/codemirror/lang-markdown)**
  — Markdown parsing
- **[`codemirror-rich-markdoc`](https://github.com/segphault/codemirror-rich-markdoc)**
  or **[`ixora`](https://codeberg.org/retronav/ixora)** — Obsidian
  Live Preview pattern (tokens hide on blur, reveal on cursor
  entry). The only WYSIWYG-Markdown hybrid that doesn't lie about
  what's on disk.
- **[`@codemirror/lint`](https://github.com/codemirror/lint)** —
  substrate-rule squiggles framework. `Diagnostic.actions[]` adds
  buttons that perform effects; severity levels include "hint";
  `markClass` for custom CSS — exactly what an "FLI-not-labelled"
  or "claim-without-citation" rule needs.
- **[`@codemirror/autocomplete`](https://github.com/codemirror/autocomplete)**
  — citation autocomplete; `[` triggers fuzzy-match against
  `~/Foundry/citations.yaml`
- **[`y-codemirror.next`](https://github.com/yjs/y-codemirror.next)**
  + **[Yjs](https://github.com/yjs/yjs)** — real-time collaborative
  editing
- Self-hosted **[`y-websocket`](https://github.com/yjs/y-websocket)**
  — collab transport (avoid Liveblocks vendor lock for v1)
- Reference patterns: **[`ink-mde`](https://github.com/davidmyersdev/ink-mde)**
  for project structure; Obsidian's Live Preview for hybrid behaviour

Monaco was rejected: bundle size, mobile story, VS Code aesthetic
lock-in. Block-based editors (BlockNote, Lexical, Tiptap default)
were rejected because their Markdown serialization is **lossy**
([BlockNote docs warn explicitly](https://www.blocknotejs.org/docs/features/export/markdown))
— incompatible with the substrate's Git-as-source-of-truth
requirement.

### 5.2 Three-keystroke ladder (Cursor pattern)

The dominant 2026 in-editor AI UX, formalised by Cursor and copied
across Zed, Copilot, etc.:

- **Tab** — passive ghost-text completion (continue what I'm
  typing). Custom completion source talks to the Doorman
  (`service-slm`); Doorman handles model selection per the
  three-tier compute routing per `conventions/compounding-substrate.md`.
- **Cmd-K** — selection + natural-language instruction → diff
  overlay → accept/reject. Diff rendered as a CodeMirror decoration
  with per-hunk accept/reject buttons. Lift from Cursor verbatim
  including the keybinding. Zero learning cost; this binding is
  universal in 2026.
- **Composer / Agent mode** — multi-file changes, surfaced as a
  unified change-set view across files. GitHub's April 2026
  [inline agent mode](https://github.blog/changelog/2026-04-24-inline-agent-mode-in-preview-and-more-in-github-copilot-for-jetbrains-ides/)
  brought agent capability into the inline experience rather than
  the chat panel — explicit acknowledgement that side-panel-as-
  chatroom was wrong even for agent work.

The chat-in-corner pattern (Notion AI v1, Microsoft Copilot in Word
default) is **explicitly rejected**. AI lives where the cursor is;
never in a separate panel.

### 5.3 Substrate squiggles (Grammarly pattern, with cited authority)

[Grammarly's color-coded squiggles](https://www.grammarly.com/blog/product/better-writing-with-grammarly/)
are the canonical inline-validator UX. We adopt the visual pattern
exactly:

- 🔴 **red** — hard substrate violation (commit blocked). Examples:
  ADR-07/19 violations; SYS-ADR-10 violations; structurally
  prohibited claims (e.g., naming Sovereign Data Foundation in
  current-tense per CLAUDE.md §6 BCSC posture)
- 🟠 **amber** — unsourced claim (no `[citation-id]` in the
  paragraph; substrate cannot resolve the assertion)
- 🔵 **blue** — unlabelled FLI (forward-looking statement without
  `forward_looking: true` in frontmatter or without cautionary
  language patterns)
- ⚪ **gray (hint)** — style-guide drift (e.g., "Do Not Use" terms
  per `POINTSAV-Project-Instructions.md` §5; structural-positioning
  slips per CLAUDE.md §6)

Hover any squiggle → tooltip cites the rule (`[ni-51-102 §4A.2]`,
`[osc-sn-51-721]`, `[claude-md-§6-bcsc-rule-1]`). Click → tooltip
with the rule ID, the cited authority, and the proposed rewrite.

**Critical distinction from Grammarly**: Grammarly's squiggle is
opaque ("Grammarly thinks this is wrong"). Ours **cite the rule**
so the author can verify it. The diagnostic itself is grounded;
that's the substrate's discipline applied to its own UI.

The diagnostic source is a CodeMirror lint extension consuming:
- `~/Foundry/citations.yaml` (citation registry)
- `conventions/bcsc-disclosure-posture.md` (FLI label patterns)
- `POINTSAV-Project-Instructions.md` §5 (Do Not Use terms)
- Per-tenant constitution (`<content_dir>/.wiki/constitution.md`,
  Phase 9 CCA)
- Jurisdiction adapter (Phase 9 CCA via Doorman)

Lint API is `EditorView → Diagnostic[]` — fast deterministic checks
first, async LLM-grounded checks second, same surface, different
latency.

### 5.4 Citation autocomplete

`[` triggers `@codemirror/autocomplete` source over
`~/Foundry/citations.yaml`. Fuzzy-matched against citation IDs
(e.g., typing `[ni` surfaces `ni-51-102` first, then `ni-25-101`,
etc.). Selection inserts `[ni-51-102]`. Hover any inserted citation
ID → preview card (same as IVC §4.3 reading-side hover preview;
authoring sees what readers will see).

When a `cites:` frontmatter block exists, autocomplete prioritises
already-cited registry entries.

### 5.5 Side rail — related TOPICs (Smart Connections pattern)

Read-only side rail surfacing embedding-similarity matches against
the content corpus. Pattern lifted from [Smart Connections for
Obsidian](https://github.com/brianpetro/obsidian-smart-connections)
and Logseq Copilot.

- Tiles show: matched TOPIC slug + first sentence snippet +
  similarity score
- Click → opens the matched TOPIC in a split (read-only)
- Never injects content into the editing buffer (separation of
  concerns: the side rail is *information*, the editor is *agency*)
- Embedding model + vector index: handled by the Doorman / service-slm;
  the editor consumes a JSON API

### 5.6 Bottom strip — live diff against working tree

Three-line status bar at the bottom of the editor:

1. File path + branch
2. Commit-readiness status: *"ready to commit"* / *"2 substrate
   violations"* / *"awaiting collab session merge"*
3. Live `git diff --stat` against the working tree, updated on
   every save-point

The IDE pattern — VS Code's status bar — applied to disclosure
prose. Author always sees what they would commit.

### 5.7 Commit gate — explicit, never auto

**Explicit commit button.** Runs full substrate validation:
- All squiggles must be at amber-or-lower severity (red blocks)
- Citation graph resolves (`cites:` entries exist in registry)
- FLI labels match content (claims tagged `forward_looking: true`
  carry cautionary language patterns)
- Phase 9 CCA proof-of-grounding chain (when Phase 9 active)
- Frontmatter schema valid (`schema: foundry-doc-v1`)

Only if all checks pass → invokes `~/Foundry/bin/commit-as-next.sh`
under the editor's authenticated user identity.

**No auto-commit.** ADR-10 / F12 (mandatory human checkpoint) is
honoured by construction. Pattern:
[Cursor's regression where auto-apply was added](https://forum.cursor.com/t/regression-ai-edits-applying-automatically-without-diff-approval-ui/154887)
showed users react instantly when this is taken away — the explicit
gate is load-bearing UX, not friction.

### 5.8 Real-time collaboration — Yjs with Git as source of truth

Per Agent-3 stress-test: [Yjs](https://github.com/yjs/yjs) is the
2026 consensus winner for CRDT-backed real-time collab. The
combination "Yjs live + Git as source of truth" is coherent:

- CRDT doc = editing-session ephemeral state
- Commit = serialization checkpoint (Y.Text → Markdown → file →
  `commit-as-next.sh`)
- [`y-codemirror.next`](https://github.com/yjs/y-codemirror.next)
  binds the CRDT to the editor (shared cursors via Yjs awareness
  protocol — "free" with the binding)

**Git remains authoritative.** This is non-negotiable per CLAUDE.md
§6 (BCSC continuous-disclosure posture requires signed,
non-repudiable Git commits). [BlockSuite](https://block-suite.com/blog/crdt-native-data-flow.html)'s
"CRDT as source of truth" pattern is rejected — it eliminates the
disclosure-grade Git audit trail.

Per the session-2 wildcard research synthesis: CRDT-shaped
disclosure records cannot say "issuer asserted X at time T" with
concurrent edits and no authority decision. CRDTs land for *draft
co-authoring inside a cluster*; never for the disclosure-tier
surface.

### 5.9 In-place vs separate-page editing — Notion's UX, Confluence's commit gate

Wikipedia's Read/Edit page split is a 2001 server-rendered artifact.
2026 best practice is in-place editing — Notion's UX, but with
Confluence's commit gate (per Agent-3 stress-test).

The page never reloads; the editor IS the page; an explicit "commit"
button (§5.7) fires the substrate validation pass and only then
the Git write lands. The Article / Talk / Edit / View history tab
metaphor is preserved (Wikipedia muscle memory item #2), but
clicking "Edit" toggles in-place rather than navigating to a
separate page.

## 6. The killer characterization

Reading the IVC + SAA design end to end:

**The substrate is the compliance witness; the AI is the productive
author; the human is the editor of last resort.**

This is what makes the substrate AI-positive in a way no surveyed
incumbent is. Today AI productivity and compliance posture trade
off — more AI authoring → more risk of hallucination → more human
review → less compliance defensibility. With the IVC + SAA + CCA
combination, more AI authoring **increases** compliance posture
because every AI output is structurally bound to disclosure rules
(SAA §5.3 squiggles + Phase 9 CCA constrained decoding) and
cryptographically witnessed at publish (IVC §4 + Inventions C, D,
E).

The reader gets something Wikipedia readers have never had: the
ability to verify, in seconds, that what they're reading is grounded.

The author gets something Wikipedia editors have never had:
real-time substrate awareness that catches FLI violations and
unsourced claims before commit.

## 7. Phase mapping — which Phase ships which UX element

| Element | Phase | Notes |
|---|---|---|
| Wikipedia muscle memory chrome (items 1–18 in §1) | 1 (shipped) → 1.1 (additive) | Phase 1 ships items 4 (footnotes), 7 (infobox capability), 10 (link colours), 11 (typography), 13 (search placeholder), 16 (mobile chrome). Phase 1.1 adds tabs (items 1, 2), edit pencils (3), end-of-article ordering (5), hatnote (6), lead first-sentence (8), tagline (9), TOC (12), language switcher (14), footer (15) as additive Phase-2 baseline. |
| IVC masthead band placeholder (no logic) | 1.1 | Renders even when no IVC machinery exists; says "verification not yet available" |
| Reader density toggle (preference UI) | 1.1 | Setting persists; no machinery to honour it until Phase 7 |
| SAA editor (CodeMirror 6 base + Live Preview) | 2 | The main Phase 2 deliverable |
| SAA three-keystroke ladder (Tab/Cmd-K/Composer) | 2 | Tab + Cmd-K in Phase 2; Composer in Phase 4 (depends on Doorman MCP) |
| SAA squiggles (deterministic rules) | 2 | Citation-graph + FLI patterns first; jurisdiction adapters in Phase 9 |
| SAA citation autocomplete | 2 | Reads `~/Foundry/citations.yaml` |
| SAA bottom strip (live git diff) | 4 | Depends on Phase 4 git2 integration |
| SAA commit gate | 4 | Depends on Phase 4 commit-on-edit |
| SAA collab (Yjs + y-codemirror.next) | 2.x | Optional; opt-in via flag |
| SAA side rail (related TOPICs) | 4.x | Depends on Doorman embedding API |
| IVC glyphs + neutral grey default | 4 | Depends on Phase 4 citation registry resolution |
| IVC hover preview | 4 | |
| IVC inline expansion panel | 4 | |
| IVC drift detection + diff-since-citation | 7 | Depends on Phase 7 federation seam (content-addressed citations) |
| IVC `verify://` URL scheme | 7 | Depends on Phase 7 hash addressing |
| IVC reader density toggle (functional) | 7 | Depends on Phase 7 verification machinery |
| Phase 9 CCA in editor | 9 (project-disclosure cluster) | SAA squiggles get jurisdictional rule packs from constitutional-layer adapter |

## 8. Anti-patterns — explicitly rejected

From Agent 4's verification UX research and Agent 2's knockoff
analysis:

- **Universal green checkmarks next to every clause.** TLS padlock
  lesson; ubiquity destroys signal.
- **Sidebar overlay for verification info** (Hypothes.is pattern).
  Competes with content; breaks reading rhythm. Use under-the-claim
  inline expansion instead (Community Notes pattern).
- **Raw hashes in body text.** Reader-unintelligible; visually
  heavy. Hashes live in expansion panels behind copy buttons.
- **Accusatory or alarmist framing.** Community Notes deliberately
  rejected "This is misleading"; IVC prefers "Source updated since
  publication — review changes" over "WARNING: drifted."
- **Paid-verification semantics.** No signal in IVC should look
  like X's blue-check (now meaningless). Verification derives from
  cryptographic state, not publisher self-declaration.
- **Modal dialogs for proof viewing.** Always inline expansion.
- **Trust-badge clutter at page chrome** (the Phantom Seals critique).
- **Chat-in-corner as primary AI surface** (Notion AI v1, Microsoft
  Viva Topics — retired Feb 2025 specifically because of this
  pattern).
- **Block-based editor with lossy Markdown serialization** (BlockNote
  default; incompatible with Git-as-source-of-truth).
- **Side-by-side preview** (Typora-legacy; wastes screen).
- **Wikipedia's read/edit page split** (2001 server-rendered artifact).
- **CRDT as source of truth** (BlockSuite; eliminates disclosure-
  grade Git audit trail).
- **Auto-apply AI edits without diff overlay approval** (Cursor
  regression demonstrated this is non-negotiable; also violates
  ADR-10 / F12).
- **Monaco as editor core** (bundle size, mobile, VS Code aesthetic
  lock-in).
- **Liveblocks / hosted vendor lock for v1** (use self-hosted
  `y-websocket`; revisit at multi-tenant scale).
- **Replacing `<ref>` with bare Markdown links** (loses footnote-
  with-backlink discipline).
- **TOC in slide-out drawer** (obscures section structure).
- **Auth-gate read access** (loses Wikipedia's anonymous-read gift).
- **Mobile as media query** (95% of mobile editors make zero edits
  on Wikipedia; the substrate must design from mobile up).

## 9. Sources

### Wikipedia muscle memory (Agent 1)

- [Wikipedia:Vector 2022 — design rationale and scope](https://en.wikipedia.org/wiki/Wikipedia:Vector_2022)
- [The Impact of the 2023 Wikipedia Redesign on User Experience — MDPI Informatics 2025](https://www.mdpi.com/2227-9709/12/3/97)
- [Research:Which parts of an article do readers read — Meta-Wiki](https://meta.wikimedia.org/wiki/Research:Which_parts_of_an_article_do_readers_read)
- [Insights on mobile web editing on Wikipedia in 2025 (Part I) — Diff](https://diff.wikimedia.org/2025/09/26/insights-on-mobile-web-editing-on-wikipedia-in-2025-part-i/)
- [Wikimedia Foundation Annual Plan 2025-2026 Product & Technology OKRs](https://meta.wikimedia.org/wiki/Wikimedia_Foundation_Annual_Plan/2025-2026/Product_&_Technology_OKRs)
- [Skin:Minerva Neue — MediaWiki](https://www.mediawiki.org/wiki/Skin:Minerva_Neue)
- [Wikipedia:Manual of Style/Layout](https://en.wikipedia.org/wiki/Wikipedia:Manual_of_Style)
- [F-Shaped Pattern For Reading Web Content — Nielsen Norman Group](https://www.nngroup.com/articles/f-shaped-pattern-reading-web-content-discovered/)
- [Charter — Butterick's Practical Typography](https://practicaltypography.com/charter.html)

### Knockoff failure modes (Agent 2)

- [Wikipedia: VisualEditor](https://en.wikipedia.org/wiki/VisualEditor)
- [Reworked: Microsoft Retiring Viva Topics](https://www.reworked.co/digital-workplace/microsoft-is-retiring-viva-topics-heres-what-you-can-do/)
- [MassiveGRID: Confluence Data Center EOL 2029 Guide](https://massivegrid.com/blog/confluence-data-center-end-of-life-2029-guide/)
- [Gartner Peer Insights: Amazon Q Developer](https://www.gartner.com/reviews/product/amazon-q-developer)
- [PkgPulse: Best Documentation Frameworks 2026](https://www.pkgpulse.com/blog/best-documentation-frameworks-2026)
- [Skywork: Perplexity Accuracy Tests 2025](https://skywork.ai/blog/news/perplexity-accuracy-tests-2025-sources-citations/)
- [Wikipedia: Citation needed](https://en.wikipedia.org/wiki/Wikipedia:Citation_needed)
- [Hypothes.is](https://web.hypothes.is/)
- [Wikijump Logs Dec 2025](https://www.wikijump.org/2025/12/31/wikijump-logs-december-2025/)
- [CSS-Tricks: Designing for Long-Form Articles](https://css-tricks.com/designing-for-long-form-articles/)

### Editor stack + AI-as-contributor (Agent 3)

- [CodeMirror 6 reference](https://codemirror.net/docs/ref/)
- [PARA Garden: CodeMirror vs Monaco](https://agenthicks.com/research/codemirror-vs-monaco-editor-comparison)
- [`@codemirror/lang-markdown`](https://github.com/codemirror/lang-markdown)
- [`codemirror-rich-markdoc`](https://github.com/segphault/codemirror-rich-markdoc)
- [`ink-mde`](https://github.com/davidmyersdev/ink-mde)
- [Cursor 2.0 changelog](https://cursor.com/blog/2-0)
- [GitHub Copilot inline agent April 2026](https://github.blog/changelog/2026-04-24-inline-agent-mode-in-preview-and-more-in-github-copilot-for-jetbrains-ides/)
- [Zed AI inline assistant](https://zed.dev/docs/ai/inline-assistant)
- [Yjs](https://github.com/yjs/yjs)
- [`y-codemirror.next`](https://github.com/yjs/y-codemirror.next)
- [Velt: best CRDT libraries](https://velt.dev/blog/best-crdt-libraries-real-time-data-sync)
- [Grammarly — color-coded suggestions](https://www.grammarly.com/blog/product/better-writing-with-grammarly/)
- [BlockNote markdown export warning](https://www.blocknotejs.org/docs/features/export/markdown)
- [Cursor diff regression forum](https://forum.cursor.com/t/regression-ai-edits-applying-automatically-without-diff-approval-ui/154887)

### Verification UX (Agent 4)

- [C2PA official Content Credentials icon post](https://spec.c2pa.org/post/contentcredentials/)
- [Adobe Content Authenticity app — design story](https://adobe.design/stories/process/behind-the-design-adobe-content-authenticity-app)
- [Google Security Blog — Pixel 10 + C2PA](https://security.googleblog.com/2025/09/pixel-android-trusted-images-c2pa-content-credentials.html)
- [Chromium Blog — An Update on the Lock Icon (May 2023)](https://blog.chromium.org/2023/05/an-update-on-lock-icon.html)
- [SSL Store — Chrome 117 lock-icon replacement](https://www.thesslstore.com/blog/google-to-replace-the-padlock-icon-in-chrome-version-117/)
- [BBC Verify — Wikipedia](https://en.wikipedia.org/wiki/BBC_Verify)
- [Asterisk Magazine — The Making of Community Notes](https://asteriskmag.com/issues/08/the-making-of-community-notes)
- [Tom Critchlow — Exploring the UX of web-annotations](https://tomcritchlow.com/2019/02/12/annotations/)
- [Distill — How to Create a Distill Article](https://distill.pub/guide/)
- [Etherscan — Viewing Transactions](https://info.etherscan.com/viewing-transactions-on-etherscan/)
- [OpenTimestamps](https://opentimestamps.org/)
- [Sigstore — cosign verify](https://docs.sigstore.dev/cosign/verifying/verify/)
- [Saint Paulo — Phantom Seals critique](https://saintpaulo.com/the-phantom-seals-why-your-trust-badges-verify-nothing/)

## 10. Cross-references

- [`../ARCHITECTURE.md`](../ARCHITECTURE.md) — engineering design;
  Phase plan; API surface set. Phase 1 chrome additions and Phase
  2 editor stack picks are extracted from this doc.
- [`./INVENTIONS.md`](INVENTIONS.md) — strategic inventions A–E;
  IVC is the reading-side surface that makes Inventions A and C
  visible to readers; SAA is the editing-side surface that makes
  Invention E (CCA, ratified as DOCTRINE claim #31 in v0.0.6)
  usable in practice.
- [`~/Foundry/conventions/disclosure-substrate.md`](../../../../../conventions/disclosure-substrate.md)
  — operationalises Inventions A, C, D, E in the wiki engine; §5.1
  (substrate-native API surface set, added in v0.1.14) implies the
  IVC + SAA UX surfaces as user-facing affordances over the
  underlying APIs.
- [`~/Foundry/conventions/bcsc-disclosure-posture.md`](../../../../../conventions/bcsc-disclosure-posture.md)
  — source for the FLI-labelling and structural-positioning rules
  that SAA squiggles enforce.

---

*This doc is a thinking artefact. The Phase 1.1 chrome additions
(masthead band placeholder + reader density toggle) and Phase 2
editor stack are concrete enough to start implementing; the IVC
machinery and Phase 9 CCA editor integration are longer-horizon and
land in their named phases.*
