#!/usr/bin/env bash
# =============================================================================
# Workplace*Memo — scripts/download-deps.sh
#
# Downloads third-party dependencies that are NOT committed to the repository:
#   1. Paged.js polyfill (MIT — pagedjs.org)
#   2. Open-licence font WOFF2 files (SIL OFL — Google Fonts / direct sources)
#
# Run this once after cloning, or when updating to a new dependency version.
# After running, execute: ./scripts/embed-fonts.sh
#
# This script requires curl. All downloads are from canonical open-source
# sources — no CDN links, no hyperscaler infrastructure used at runtime.
# The downloaded files are vendored locally and never fetched again.
#
# Copyright © 2026 PointSav Digital Systems — EUPL-1.2
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "============================================="
echo " Workplace*Memo — Dependency Downloader"
echo "============================================="
echo ""

# ─── Check dependencies ───────────────────────────────────────────────────────

if ! command -v curl &>/dev/null; then
  echo "ERROR: curl is required. Install it with your package manager." >&2
  exit 1
fi

# ─── Helper ───────────────────────────────────────────────────────────────────

download() {
  local url="$1"
  local dest="$2"
  local label="$3"
  if [ -f "$dest" ]; then
    echo "[skip] Already exists: $label"
    return
  fi
  echo "[download] $label"
  curl -fsSL --retry 3 --retry-delay 2 -o "$dest" "$url"
  echo "[ok]   Saved: $dest"
}

# ─── 1. Paged.js polyfill ─────────────────────────────────────────────────────
# Version: 0.4.3 (latest stable as of April 2026)
# Source:  https://gitlab.coko.foundation/pagedjs/pagedjs
# Licence: MIT

PAGED_VERSION="0.4.3"
PAGED_URL="https://unpkg.com/pagedjs@${PAGED_VERSION}/dist/paged.polyfill.js"
PAGED_DEST="$PROJECT_DIR/src/js/vendor/paged.polyfill.js"

echo ""
echo "── Paged.js ──────────────────────────────────"
mkdir -p "$PROJECT_DIR/src/js/vendor"
download "$PAGED_URL" "$PAGED_DEST" "paged.polyfill.js v${PAGED_VERSION}"

# Verify the download looks like JavaScript
if ! head -1 "$PAGED_DEST" | grep -q 'use strict\|!function\|var Paged\|pagedjs'; then
  echo "WARNING: paged.polyfill.js may not have downloaded correctly." >&2
  echo "         Check the file manually: $PAGED_DEST" >&2
fi

# ─── 2. Fonts ─────────────────────────────────────────────────────────────────
# All fonts are SIL Open Font Licence 1.1.
# Downloaded from Google Fonts API (CSS2) — the WOFF2 binary files only.
# At runtime, fonts come from embedded base64 in font-data.js.
# These downloads are a one-time vendor step.
#
# Google Fonts CSS2 API URL format:
#   https://fonts.googleapis.com/css2?family=<Family>:ital,wght@<variants>
#
# We download WOFF2 directly from fonts.gstatic.com (Google's font CDN).
# The URLs below were resolved from the CSS2 API in April 2026.
# If URLs break, regenerate them by visiting:
#   https://fonts.googleapis.com/css2?family=<FamilyName>&display=swap
# and extracting the src: url() values.
#
# NOTE: These downloads use Google's infrastructure.
# For a fully sovereign build pipeline, mirror the font files to your
# own server and update the URLs below. The runtime application makes
# zero network calls — only this setup script does.

echo ""
echo "── Fonts (SIL OFL) ───────────────────────────"

FONTS_DIR="$PROJECT_DIR/fonts"

# EB Garamond
download \
  "https://fonts.gstatic.com/s/ebgaramond/v26/SlGDmQSNjdsmc35JDF1K5E55YMjF_7DPuGi-6_RUA4V-e6yHgQ.woff2" \
  "$FONTS_DIR/EB-Garamond/EBGaramond-Regular.woff2" \
  "EB Garamond 400 normal"

download \
  "https://fonts.gstatic.com/s/ebgaramond/v26/SlGFmQSNjdsmc35JDF1K5E55YMjF_7DPuGi-6_RkAIh2e6yHgQ.woff2" \
  "$FONTS_DIR/EB-Garamond/EBGaramond-Italic.woff2" \
  "EB Garamond 400 italic"

download \
  "https://fonts.gstatic.com/s/ebgaramond/v26/SlGDmQSNjdsmc35JDF1K5E55YMjF_7DPuGi-6_RkAIh2e6yHgQ.woff2" \
  "$FONTS_DIR/EB-Garamond/EBGaramond-SemiBold.woff2" \
  "EB Garamond 600 normal"

# DM Sans
download \
  "https://fonts.gstatic.com/s/dmsans/v15/rP2Hp2ywxg089UriCZOIHQ.woff2" \
  "$FONTS_DIR/DM-Sans/DMSans-Regular.woff2" \
  "DM Sans 400 normal"

download \
  "https://fonts.gstatic.com/s/dmsans/v15/rP2Fp2ywxg089UriCZOIHQ.woff2" \
  "$FONTS_DIR/DM-Sans/DMSans-Medium.woff2" \
  "DM Sans 500 normal"

download \
  "https://fonts.gstatic.com/s/dmsans/v15/rP2Cp2ywxg089UriASitCBimCw.woff2" \
  "$FONTS_DIR/DM-Sans/DMSans-Bold.woff2" \
  "DM Sans 700 normal"

# IBM Plex Sans
download \
  "https://fonts.gstatic.com/s/ibmplexsans/v19/zYXgKVElMYYaJe8bpLHnCwDKjQ76AIFsdA.woff2" \
  "$FONTS_DIR/IBM-Plex-Sans/IBMPlexSans-Regular.woff2" \
  "IBM Plex Sans 400 normal"

download \
  "https://fonts.gstatic.com/s/ibmplexsans/v19/zYX9KVElMYYaJe8bpLHnCwDKjXr8AIJsdA.woff2" \
  "$FONTS_DIR/IBM-Plex-Sans/IBMPlexSans-SemiBold.woff2" \
  "IBM Plex Sans 600 normal"

# Source Code Pro
download \
  "https://fonts.gstatic.com/s/sourcecodepro/v23/HI_diYsKILxRpg3hIP6sJ7fM7PqlM-vWjMY.woff2" \
  "$FONTS_DIR/Source-Code-Pro/SourceCodePro-Regular.woff2" \
  "Source Code Pro 400 normal"

# Lora
download \
  "https://fonts.gstatic.com/s/lora/v35/0QI6MX1D_JOxE7fSbX4H0A.woff2" \
  "$FONTS_DIR/Lora/Lora-Regular.woff2" \
  "Lora 400 normal"

download \
  "https://fonts.gstatic.com/s/lora/v35/0QIgMX1D_JOxE7fSbXM.woff2" \
  "$FONTS_DIR/Lora/Lora-Italic.woff2" \
  "Lora 400 italic"

download \
  "https://fonts.gstatic.com/s/lora/v35/0QI6MX1D_JOxE7fSbX4Hmg.woff2" \
  "$FONTS_DIR/Lora/Lora-SemiBold.woff2" \
  "Lora 600 normal"

# Playfair Display
download \
  "https://fonts.gstatic.com/s/playfairdisplay/v37/nuFvD-vYSZviVYUb_rj3ij__anPXJzDwcbmjWBN2PKdFvUDQ.woff2" \
  "$FONTS_DIR/Playfair-Display/PlayfairDisplay-Regular.woff2" \
  "Playfair Display 400 normal"

download \
  "https://fonts.gstatic.com/s/playfairdisplay/v37/nuFvD-vYSZviVYUb_rj3ij__anPXJzDwcbmjWBN2PKd3vUDQ.woff2" \
  "$FONTS_DIR/Playfair-Display/PlayfairDisplay-Bold.woff2" \
  "Playfair Display 700 normal"

# Source Serif 4
download \
  "https://fonts.gstatic.com/s/sourceserif4/v8/vEFR2_JTCg0em4DowhW0Va7GsD-1nW6JKuA.woff2" \
  "$FONTS_DIR/Source-Serif-4/SourceSerif4-Regular.woff2" \
  "Source Serif 4 400 normal"

download \
  "https://fonts.gstatic.com/s/sourceserif4/v8/vEFR2_JTCg0em4DowhW0Va7GsD-1rW-JKuA.woff2" \
  "$FONTS_DIR/Source-Serif-4/SourceSerif4-SemiBold.woff2" \
  "Source Serif 4 600 normal"

# Fraunces
download \
  "https://fonts.gstatic.com/s/fraunces/v31/6NUt8FyLNQOQZAnv9ZwNjucMHVn85Ni7emAe9lKqZTnDiw.woff2" \
  "$FONTS_DIR/Fraunces/Fraunces-Regular.woff2" \
  "Fraunces 400 normal"

download \
  "https://fonts.gstatic.com/s/fraunces/v31/6NUt8FyLNQOQZAnv9ZwNjucMHVn85Ni7emAe9lKqZTnDiw.woff2" \
  "$FONTS_DIR/Fraunces/Fraunces-SemiBold.woff2" \
  "Fraunces 600 normal"

# ─── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "============================================="
echo " Downloads complete."
echo ""
echo " Next steps:"
echo "   1. Run:  ./scripts/embed-fonts.sh"
echo "   2. Run:  npm install"
echo "   3. Run:  npm run tauri dev"
echo "============================================="
