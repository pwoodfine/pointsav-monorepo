# Design system — decision record (P2)

Recorded 2026-07-01. Full recon + candidate transcripts are in this session's
agent history; this file is the durable record of what was decided and why.

## Recon (3 parallel `fable`-model agents, browser-in-the-loop)

1. **Current production keep-list** — live Playwright audit of the actual
   public sites (not the local preview — see below). Real computed values:
   canvas `#F7F9FA`, body ink `#111827`, secondary `#6B7280`, accent navy
   `#164679` (accent-only today), section grey `#E6E7E8`, Woodfine hero navy
   (white text), PointSav hero slate `#B4C5D5` (dark text). Fonts loading:
   Nunito Sans (body), Oswald (nav/kickers), Roboto Slab (hero lede). Layout
   DNA: flat, no radius/shadow, navy top-rule card motif (the signature).
   axe-core: 4–5 violations per site (missing `lang`, missing `h1`, footer
   contrast failures, one aria issue). No hamburger nav anywhere. `/es`
   returns 404 on both — bilingual delivery is currently absent in production.
   **Important operational finding, verified independently by curl**: the
   public domains still serve the *old* `__bundler` monolith engine entirely
   — the June cutover only ever reached the local preview ports (9101/9102),
   never production. The keep-list above is genuine current-production truth.
2. **Wiki standard** (`app-mediakit-knowledge-2`, live at 127.0.0.1:9090) —
   white 57px sticky masthead, centered search, `--k-*` tokens, per-tenant
   swap via `[data-instance]` (accent only), 3-column footer + badge row,
   "Powered by MediaKit" badge (white pill, border `#c8ccd1`, radius 3px, SVG
   glyph + stacked "POWERED BY"/"MediaKit" text, muted ink, zero brand color),
   mobile hamburger opens a pre-rendered left slide-in drawer.
3. **Competitive research** — navy+white is the dominant top-PE-firm palette
   (validates the locked Sovereign Editorial direction); serif-display +
   sans-body is a credible "editorial authority" differentiator; real asset
   photography should carry hero weight, not chrome color; footer needs
   on-page jurisdiction-disclosure slots (SEC Marketing Rule "clear and
   prominent" guidance — hyperlinked disclosures don't satisfy it);
   audience-segmented nav IA; Core Web Vitals budgets (LCP<1.5s, INP<200ms,
   CLS<0.05) should shape technical choices (variable fonts, minimal JS
   motion).

## Three independent candidates (judged, not averaged)

- **A — "Prospectus Serif"** (`--mkt-*`): Fraunces + Public Sans + Source
  Code Pro. Serif used pervasively (h1/h2/h3/lede/nav-drawer/badge
  wordmark) — boldest editorial-authority bet.
- **B — "Restrained Modernist"** (`--mx-*`): kept Nunito Sans for body
  continuity, paired with a quiet Source Serif 4 (display/h2/lede only) +
  Source Code Pro. Strongest CWV/performance framing; most explicit footer
  disclosure-slot structure (3-tier: nav columns → disclosure band → badge
  row).
- **C — "Family Consistency"** (`--m-*`): Inter + Source Serif 4 (both
  literally the wiki's own fonts) + Source Code Pro. Deepest integration —
  shares the wiki's exact pixel values (57px masthead, 640/960 breakpoints,
  75/150/300ms motion curves, 720px prose measure, byte-identical badge).
  Chrome polarity (dark masthead/footer vs. the wiki's light chrome) is the
  *only* differentiating axis; everything else shares values, not just
  structure.

## Decision: Candidate C wins as the base

The operator's stated priorities across this program repeatedly emphasize
integration — "full integration between the various websites," streamlined
shared CSS/tokens/bundles, and pulling small details (the MediaKit badge)
backward from the wiki into marketing. Candidate C serves that directly:
reusing the wiki's already-proven, already-self-hosted fonts carries zero new
font-loading risk and the smallest total maintenance surface across the site
family, and its badge treatment is explicitly a certification mark that does
not theme — byte-identical to the wiki's real badge — which is what "work the
MediaKit badge in backwards" calls for, not a marketing-specific
reinterpretation.

**Grafted from B:** the explicit three-tier footer structure (nav/contact
columns → rule-separated on-page disclosure band, mono-caps label + legal
body per jurisdiction slot → badge/trademark base row) — C had the tokens for
this but B's layout description was more concrete; adopted into `tokens.css`
(`--m-slot-*` tokens) and this structure.

**Grafted from A:** the hero-photography scrim gradient
(`--m-hero-scrim`, per-brand) so text stays legible over real asset
photography without hiding it, and the explicit articulation of
`overflow-x: clip` + zero-fixed-width discipline as *the* structural fix for
the audited sub-page horizontal-scroll bug (implemented in `app.css`).

**Not grafted:** A's serif-set badge wordmark ("MediaKit" in Fraunces) —
conflicts with the "byte-identical certification mark" property that made C's
badge the right choice in the first place.

## Radius tension — resolved explicitly (not a compromise)

Keep-list says flat/no-radius (the "prospectus" register); the wiki uses
2–3px. Resolution: content surfaces (cards, hero, sections, buttons) stay
`--m-radius-0` — the keep-list's flatness and the navy top-rule card motif
are marketing's own signature and survive intact. Shared family micro-UI
(the badge, form inputs) adopts the wiki's exact 2/3px, because those are the
artifacts meant to be recognizable *as the same thing* across every site in
the family. A 3px pill inside an otherwise-square layout reads as consistent
craft, not inconsistency.

## Secondary-ink WCAG fix

Production's `#6B7280` fails AA (~4.4:1) on the canvas and worse on the
section-grey band — a real, structural bug, not a style preference. Demoted
to `--m-ink-muted` (light-surface-only use); the wiki's proven `#54595d`
(`--m-ink-secondary`) becomes the workhorse secondary ink everywhere else —
a WCAG fix that happens to *increase* family kinship rather than costing
anything.

## What's still open (lands in P3)

- Actual chrome markup (masthead, hero, card grid, footer, mobile drawer) —
  tokens exist, HTML/maud structure does not yet.
- Font files themselves (`static/fonts/*.woff2`) — `fonts.css` declares the
  contract; asset fetch is a P3 step.
- Search bar in the masthead — the wiki has one because it has a search
  corpus; marketing (6 pages) may not need a real one. Decide in P3/implementation.
- Audience-segmented nav IA (from the competitive research) — worth applying
  once real nav content/pages are being built in P3.
