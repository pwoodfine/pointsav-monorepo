# app-mediakit-marketing-2

Marketing platform engine — ground-up rewrite. Temporary `-2` name; renamed to
`app-mediakit-marketing` at cutover once parity is reached and the operator
signs off (plan phase P8).

## Why a rewrite

The retired `app-mediakit-marketing` + `app-mediakit-shell` remain live at
home.woodfinegroup.com / home.pointsav.com and are not modified by this crate.
They serve as a read-only contract reference and anti-pattern catalogue — no
code is reused, only the third-party dependencies and the external contract
(routes, `SERVICE_MARKETING_*` env vars, content-manifest schema) so the
eventual swap-in requires zero/minimal deployment change.

## Phase program

P0 scaffold → P1 content pipeline → P2 design system → P3 core pages →
P4 SEO/discovery → P5 MCP + review queue → P6 test suite → P7 parity + shadow
deploy → P8 swap + retire old crate (Command Session scope).

## Run

```bash
cargo run -p app-mediakit-marketing-2 -- serve \
  --content-dir ../app-mediakit-marketing/content \
  --module-id woodfine \
  --bind 127.0.0.1:9202
```

`cargo run -p app-mediakit-marketing-2 -- check --content-dir <dir>` validates
config/content without serving.

## Status

P0 — scaffold. Builds green; `/healthz` + `/static/*path` served; CLI/env
contract matches the retired engine.
