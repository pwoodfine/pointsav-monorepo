#!/usr/bin/env bash
# build-vendor-js.sh — build the SAA editor JS bundle.
#
# Wraps `npm ci && node build.mjs` so operators have one entry point.
# Output: static/vendor/cm-saa.bundle.js
#
# Run from anywhere; the script cds into vendor-js/.

set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$CRATE_DIR/vendor-js"

if [ ! -d node_modules ]; then
    echo "==> npm ci"
    npm ci || npm install
fi

echo "==> node build.mjs"
node build.mjs

OUT="$CRATE_DIR/static/vendor/cm-saa.bundle.js"
if [ -f "$OUT" ]; then
    SIZE=$(stat -c%s "$OUT" 2>/dev/null || stat -f%z "$OUT" 2>/dev/null)
    echo "==> built: $OUT ($SIZE bytes)"
else
    echo "==> build did not produce $OUT" >&2
    exit 1
fi
