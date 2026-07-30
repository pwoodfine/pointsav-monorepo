#!/usr/bin/env bash
# edit-live-content.sh — Open the correct deployment HTML in $EDITOR.
#
# This is the ONLY correct way to make live content changes to
# home.woodfinegroup.com and home.pointsav.com. Git commits to the
# monorepo do NOT update the live sites — only editing these files does.
#
# Usage: bash scripts/edit-live-content.sh <woodfine|pointsav>
#
# After $EDITOR closes:
#   - Confirms the file was modified
#   - Re-applies the viewport patch (fix-viewport.sh) if needed
#   - Shows the new file state

set -euo pipefail

WOODFINE_HTML="/srv/foundry/deployments/media-marketing-landing-1/content/index.html"
POINTSAV_HTML="/srv/foundry/deployments/media-marketing-landing-2/content/index.html"

SCRIPTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIX_VIEWPORT="$SCRIPTS_DIR/fix-viewport.sh"

RED='\033[0;31m'
GRN='\033[0;32m'
YEL='\033[1;33m'
NC='\033[0m'

usage() {
  echo "Usage: bash scripts/edit-live-content.sh <woodfine|pointsav>"
  echo ""
  echo "  woodfine  — edit /srv/foundry/deployments/media-marketing-landing-1/content/index.html"
  echo "  pointsav  — edit /srv/foundry/deployments/media-marketing-landing-2/content/index.html"
  exit 1
}

[[ $# -lt 1 ]] && usage

SITE="${1,,}"
case "$SITE" in
  woodfine) TARGET="$WOODFINE_HTML" ;;
  pointsav) TARGET="$POINTSAV_HTML" ;;
  *) echo -e "${RED}Unknown site: $1${NC}"; usage ;;
esac

if [[ ! -f "$TARGET" ]]; then
  echo -e "${RED}ERROR: deployment file not found: $TARGET${NC}"
  echo "  Check that the deployment instance is provisioned."
  exit 1
fi

MTIME_BEFORE=$(stat --format="%Y" "$TARGET")
SIZE_BEFORE=$(stat --format="%s" "$TARGET")

echo -e "${YEL}Editing live content for: $SITE${NC}"
echo "  File: $TARGET"
echo "  Size before: $(( SIZE_BEFORE / 1024 )) KB"
echo ""
echo "  Opening in \${EDITOR:-nano}. Save and close to apply changes."
echo ""

${EDITOR:-nano} "$TARGET"

MTIME_AFTER=$(stat --format="%Y" "$TARGET")
SIZE_AFTER=$(stat --format="%s" "$TARGET")

echo ""
if [[ "$MTIME_AFTER" -gt "$MTIME_BEFORE" ]]; then
  echo -e "${GRN}File modified.${NC}"
  echo "  Size after:  $(( SIZE_AFTER / 1024 )) KB  (delta: $(( SIZE_AFTER - SIZE_BEFORE )) bytes)"
  echo "  Modified:    $(stat --format="%y" "$TARGET" | cut -d'.' -f1)"
else
  echo -e "${YEL}File unchanged (same mtime). No live change applied.${NC}"
fi

echo ""
echo "── Re-applying viewport patch ──────────────────────────────────────────"
if [[ -x "$FIX_VIEWPORT" ]]; then
  bash "$FIX_VIEWPORT" 2>&1 | grep -E "^(OK|PATCH|DONE|WARN|FAIL)"
else
  echo "  fix-viewport.sh not found or not executable — skipping"
fi

echo ""
echo "── Live state ──────────────────────────────────────────────────────────"
TITLE=$(grep -o '<title>[^<]*</title>' "$TARGET" 2>/dev/null | sed 's/<[^>]*>//g' || echo "(no title found)")
echo "  Title:    $TITLE"
echo "  File:     $TARGET"
echo "  Modified: $(stat --format="%y" "$TARGET" | cut -d'.' -f1)"
echo ""
echo "  Changes are live immediately — no service restart needed."
echo "  Run bash scripts/verify-live.sh to confirm both tenants."
