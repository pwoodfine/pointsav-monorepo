# vendor-js — SAA editor bundle build

Out-of-tree NPM build that produces `../static/vendor/cm-saa.bundle.js`.
The bundle is CodeMirror 6 + the extensions the SAA editor uses, wrapped
in an IIFE that exposes `window.CMSAA`. The Rust build never invokes NPM;
the Rust binary embeds the pre-built bundle via `rust-embed`.

## When to rebuild

- Bumping CodeMirror, esbuild, or any dep in `package.json`
- After a fresh checkout if `../static/vendor/cm-saa.bundle.js` is missing
  (e.g. shallow clone, or someone deleted it for cleanliness)

## How to rebuild

Requires a recent Node + npm. From this directory:

```
npm ci
node build.mjs
```

Outputs:

```
../static/vendor/cm-saa.bundle.js     committed to Git
```

`node_modules/` and `package-lock.json` are gitignored — the build is
deterministic enough for our purposes against the `^X.Y.0` ranges in
`package.json`. If you want a fully reproducible build chain, switch
the ranges to exact pins after the first successful build and commit
the lockfile.

## Bundle contents

Per `entry.js`:

- `@codemirror/state` — editor state primitive
- `@codemirror/view` — DOM rendering
- `@codemirror/commands` — keymap + history
- `@codemirror/language` — language framework
- `@codemirror/lang-markdown` — Markdown parsing + highlighting
- `@codemirror/lint` — diagnostic / squiggle framework (Phase 2 Step 4)
- `@codemirror/autocomplete` — completion (Phase 2 Step 5)

The bundle exposes them as `window.CMSAA.state`, `window.CMSAA.view`,
`window.CMSAA.commands`, `window.CMSAA.language`, `window.CMSAA.langMarkdown`,
`window.CMSAA.lint`, `window.CMSAA.autocomplete`.

`static/saa-init.js` (first-party) consumes this bundle to initialise the
editor when `/edit/{slug}` loads.

## Why not pull from a CDN?

`conventions/zero-container-runtime.md` + the single-binary deployment
constraint — operators install one Rust binary; the binary serves
everything, including the editor JS. No CDN dependency, no third-party
load, no rate limits at deploy time.

## Why commit the built artefact?

So a fresh clone of the monorepo can `cargo build --release` without
needing Node installed. The trade is ~300-400 KB of repo growth per
bundle revision; revisions are rare (only on dep bumps).
