# Inputs — project-orgcharts cluster

This folder is where you (Jennifer) drop source files for chart and
diagram work. Task Claude reads everything here and uses it as the
starting material for drafts in `working/` and final renders in
`outputs/`.

This is a private folder. It lives in the deployment instance
`cluster-totebox-corporate-1`, which is gitignored at the workspace
root and never travels to GitHub. The corporate content stays inside
Woodfine; only the design-system components extracted from chart
authoring are public.

---

## File-naming convention (authoritative)

Every file in this folder, in `working/`, and in `outputs/` follows
this pattern:

```
<DEPARTMENT>_<ENTITY>_<YYYY-MM-DD>_<chart-slug>_JW<n>_AI<n>_<STATUS>.<ext>
```

Seven parts, separated by underscores. The chart-slug uses internal
hyphens (kebab-case). Spaces are allowed only inside the
`<DEPARTMENT>` field.

### What each part means

| Part | Format | Example | Notes |
|---|---|---|---|
| `<DEPARTMENT>` | UPPERCASE; spaces allowed | `INVESTOR RELATIONS`, `COMPLIANCE`, `OPERATIONS` | Functional area inside the entity. Same department label appears across all files for the same audience. |
| `<ENTITY>` | short code, mixed case | `MCorp`, `WCPI`, `PointSav` | Which corporate entity the chart belongs to. `MCorp` = Woodfine Management Corp. Add new short codes as needed. |
| `<YYYY-MM-DD>` | ISO 8601 with hyphens | `2026-04-25` | The date this version was created. Never the edit date — once a file is named, the date is fixed. ISO format sorts naturally in any tool. |
| `<chart-slug>` | kebab-case (lowercase, hyphens) | `spv-arrangements`, `exec-team`, `corporate-structure` | One slug per chart subject. Reuse across versions of the same chart. **Internal HTML references (titles, IDs, class names) use the same kebab-case form** — no `Title_Case_With_Underscores` inside files. |
| `JW<n>` | `JW` + integer | `JW1`, `JW2`, `JW3` | Jennifer's revision counter. Replaces the older `v1, v2` numbering. Bumps every time you upload a new version from your side. |
| `AI<n>` | `AI` + integer | `AI1`, `AI2`, `AI3` | Independent AI iteration counter. Bumps every time Task Claude produces a new version off the current `JW<n>`. The first SOURCE upload is `AI1` because PowerPoint/source-tool export counts as the first AI step. |
| `<STATUS>` | one of five words (UPPERCASE) | `SOURCE` | See status vocabulary below. |
| `<ext>` | original file extension | `.html`, `.txt`, `.csv`, `.png`, `.pdf`, `.svg` | Whatever the file actually is. |

### How `JW<n>` and `AI<n>` move together

The two counters are independent. A complete chart's filename
sequence over its life looks like this:

| Step | Filename suffix | Who wrote it | What happened |
|---|---|---|---|
| 1 | `_JW1_AI1_SOURCE.html` | Jennifer | First upload (PPT-to-HTML export = AI1) |
| 2 | `_JW1_AI2_DRAFT.html` | Task Claude | First draft off Jennifer's source |
| 3 | `_JW1_AI3_DRAFT.html` | Task Claude | Revision after chat feedback |
| 4 | `_JW1_AI4_REVIEW.html` | Task Claude | Ready for Jennifer to review |
| 5 | `_JW2_AI1_SOURCE.html` | Jennifer | Jennifer revises the source after review (JW bumps, AI resets to 1) |
| 6 | `_JW2_AI2_DRAFT.html` | Task Claude | First draft off Jennifer's revised source |
| 7 | `_JW2_AI3_FINAL.html` | Task Claude | Approved |

Rule of thumb: `JW` bumps when **Jennifer** uploads a new source.
`AI` resets to 1 when JW bumps, then increments with every Task
Claude iteration.

### Status vocabulary (only these five words)

| Status | Who writes it | What it means |
|---|---|---|
| `SOURCE` | **Jennifer** | Raw source material — your input. Lives in `inputs/`. |
| `DRAFT` | Task Claude | Work-in-progress. Lives in `working/`. |
| `REVIEW` | Task Claude | Draft Task believes is ready for your review. Lives in `working/`. |
| `FINAL` | Task Claude | Approved by Jennifer. Lives in `outputs/`. |
| `ARCHIVE` | Either | Superseded version kept for history. |

### Worked example — current SPV chart

You uploaded:

```
inputs/INVESTOR RELATIONS_MCorp_2026-04-25_spv-arrangements_JW1_AI1_SOURCE.html
```

Decoded:
- Department: `INVESTOR RELATIONS`
- Entity: `MCorp` (Woodfine Management Corp)
- Date: 2026-04-25
- Subject: `spv-arrangements`
- Jennifer's first revision: `JW1`
- AI iteration: `AI1` (PowerPoint HTML export)
- Status: `SOURCE`

My first draft will be:

```
working/INVESTOR RELATIONS_MCorp_2026-04-25_spv-arrangements_JW1_AI2_DRAFT.html
```

If you then revise the source after seeing my draft, you upload:

```
inputs/INVESTOR RELATIONS_MCorp_2026-04-29_spv-arrangements_JW2_AI1_SOURCE.html
```

(Note: date can change to reflect the new revision date; chart-slug
stays the same.)

### Hard rules

1. **Never overwrite a file.** Always create a new file with the
   appropriate counter increment. If `JW1_AI3_DRAFT.html` exists,
   the next version is `JW1_AI4_DRAFT.html` (or
   `JW1_AI4_REVIEW.html`), not a re-save of `JW1_AI3`.
2. **Never delete an old version while the chart is active.** If
   something is truly obsolete, rename its status to `ARCHIVE`. Bulk
   archive cleanup happens at the end of a chart's life.
3. **Never reuse a chart-slug for a different chart.** If the
   subject is different, pick a new slug. `spv-arrangements` and
   `exec-team` are separate subjects — separate slugs, separate
   counter chains.
4. **One chart per file.** Don't combine two unrelated charts into
   one source file. Upload them separately with separate slugs.
5. **Internal references match the filename casing.** When Task
   Claude works on a file, internal HTML `<title>`, IDs, class
   names, and references use the same kebab-case slug
   (`spv-arrangements`, not `SPV_Arrangements`). Department label
   stays `INVESTOR RELATIONS` in both filename and any internal
   metadata.

---

## What to put in a SOURCE file

Task Claude can work from many input shapes. The more structured
your input, the more deterministic the output. In rough order of
ease:

### Best — an existing rendered file

A `.html`, `.svg`, `.pdf`, or `.pptx` exported as `.html` showing
the chart you want re-rendered or refined. Task Claude reads the
geometry and content directly. PowerPoint HTML exports (like the
SPV file) work fine — Task Claude will translate from absolute
positioning to a clean design-system layout in the DRAFT.

### Good — a structured table

A `.csv` file with explicit columns. For an org chart:

```
name,title,reports_to,department,status
Jane Doe,CEO,,Executive,active
John Smith,CFO,Jane Doe,Finance,active
```

For a structural chart (like SPV arrangements), columns vary by
subject — describe them in a companion `.md` notes file with the
same chart-slug.

### Acceptable — a hierarchical text outline

A `.txt` or `.md` file with one item per line, indented to show
structure (two spaces per level):

```
Level 1 — Investment Banking
  GP
    LP 1
    LP 2
    LP 3
```

### Last resort — a description in prose

A `.txt` or `.md` file describing the structure in words. Use this
only if you don't have a rendered chart, table, or outline.

---

## Companion files

If a chart needs supporting material (photos, brand assets, notes),
upload them with the **same chart-slug, JW counter, and AI counter**
as the SOURCE file they belong to:

```
INVESTOR RELATIONS_MCorp_2026-04-25_spv-arrangements_JW1_AI1_SOURCE.html
INVESTOR RELATIONS_MCorp_2026-04-25_spv-arrangements_JW1_AI1_SOURCE_notes.md
INVESTOR RELATIONS_MCorp_2026-04-25_spv-arrangements_JW1_AI1_SOURCE_photo-jane-doe.jpg
```

Suffix after `_SOURCE` describes what kind of file it is. The
filename prefix still ties them all together.

---

## What goes where

| Folder | Who writes | Who reads | Direction with your Mac |
|---|---|---|---|
| `inputs/` | You only | Task Claude | You push (upload) |
| `working/` | Task Claude only | You can pull to peek | Task pushes, you can pull |
| `outputs/` | Task Claude only | You pull (download finals) | Task pushes, you pull |

You never need to write into `working/` or `outputs/` — those are
Task Claude's working space. To change something in a draft, leave
a note (chat with me, or upload a new SOURCE version with the
changes spelled out — that bumps `JW`).

---

## When in doubt

If you're not sure what counter to bump, what slug to pick, or
where a file belongs — ask in chat. Better to ask once than to
break the convention and have to rename files later.

---

*Last updated 2026-04-26. Lives in
`/srv/foundry/deployments/cluster-totebox-corporate-1/inputs/README.md`.*
*Convention authored by Jennifer Woodfine; documented here for
Task Claude to follow.*
