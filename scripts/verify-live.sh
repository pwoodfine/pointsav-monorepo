#!/usr/bin/env bash
# verify-live.sh — Show the actual state of the live marketing sites.
#
# Run at session START before making changes (baseline) and at session END
# after changes (confirm they took effect). Do NOT rely on git commits to
# infer what is live — always run this.
#
# Usage: bash scripts/verify-live.sh

set -euo pipefail

WOODFINE_HTML="/srv/foundry/deployments/media-marketing-landing-1/content/index.html"
POINTSAV_HTML="/srv/foundry/deployments/media-marketing-landing-2/content/index.html"
WOODFINE_PORT="9102"
POINTSAV_PORT="9101"

# ANSI colours
RED='\033[0;31m'
YEL='\033[1;33m'
GRN='\033[0;32m'
NC='\033[0m'

ok()   { echo -e "${GRN}  OK${NC}    $*"; }
warn() { echo -e "${YEL}  WARN${NC}  $*"; }
fail() { echo -e "${RED}  FAIL${NC}  $*"; }

echo ""
echo "=== Marketing Site Live State — $(date -u '+%Y-%m-%d %H:%M UTC') ==="
echo ""

# ── File state ──────────────────────────────────────────────────────────────
echo "── Deployment files ────────────────────────────────────────────────────"
for LABEL in "Woodfine" "PointSav"; do
  if [[ "$LABEL" == "Woodfine" ]]; then
    F="$WOODFINE_HTML"
  else
    F="$POINTSAV_HTML"
  fi

  if [[ ! -f "$F" ]]; then
    fail "$LABEL  $F — FILE NOT FOUND"
    continue
  fi

  SIZE=$(stat --format="%s" "$F")
  MTIME=$(stat --format="%y" "$F" | cut -d'.' -f1)
  TITLE=$(grep -o '<title>[^<]*</title>' "$F" 2>/dev/null | sed 's/<[^>]*>//g' || echo "(no title found)")
  SIZE_KB=$(( SIZE / 1024 ))

  ok "$LABEL"
  echo "         file:  $F"
  echo "         size:  ${SIZE_KB} KB  (${SIZE} bytes)"
  echo "        mtime:  $MTIME"
  echo "        title:  $TITLE"
  echo ""
done

# ── Viewport patch state ─────────────────────────────────────────────────────
echo "── Viewport patch (fix-viewport.sh) ───────────────────────────────────"
FIXED_MARKER='document\.head\.innerHTML = doc\.head\.innerHTML'
OLD_PATTERN='document\.documentElement\.replaceWith(doc\.documentElement)'

for LABEL in "Woodfine" "PointSav"; do
  if [[ "$LABEL" == "Woodfine" ]]; then F="$WOODFINE_HTML"; else F="$POINTSAV_HTML"; fi
  [[ ! -f "$F" ]] && continue

  if grep -q "$FIXED_MARKER" "$F"; then
    ok "$LABEL  viewport patch applied"
  elif grep -q "$OLD_PATTERN" "$F"; then
    warn "$LABEL  old pattern present — run: bash scripts/fix-viewport.sh"
  else
    warn "$LABEL  neither pattern found — manual inspection needed"
  fi
done
echo ""

# ── HTTP health ───────────────────────────────────────────────────────────────
echo "── HTTP health (direct to backend, bypassing nginx) ────────────────────"
for LABEL in "Woodfine" "PointSav"; do
  if [[ "$LABEL" == "Woodfine" ]]; then PORT="$WOODFINE_PORT"; else PORT="$POINTSAV_PORT"; fi

  HTTP_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "http://127.0.0.1:${PORT}/" 2>/dev/null || echo "ERR")
  if [[ "$HTTP_STATUS" == "200" ]]; then
    ok "$LABEL  HTTP 200 on port ${PORT}"
  else
    fail "$LABEL  HTTP ${HTTP_STATUS} on port ${PORT} — service may be down"
  fi
done
echo ""

# ── Reminder ─────────────────────────────────────────────────────────────────
echo "── Reminder ─────────────────────────────────────────────────────────────"
echo "   To make a live change:"
echo "     bash scripts/edit-live-content.sh woodfine    # or: pointsav"
echo "   Changes take effect immediately — no service restart needed."
echo ""
