#!/usr/bin/env bash
# fix-viewport.sh — Re-apply the body-only DOM swap viewport fix to both
# marketing deployment index.html files.
#
# Background: The original bundle outer-shell JS used
#   document.documentElement.replaceWith(doc.documentElement)
# which causes iOS Safari to fall back to the 980px desktop viewport, shrinking
# max-width:1440px layouts to ~30% on 390px screens. This script replaces that
# pattern with a head+body split swap that keeps the outer shell <head> (and its
# viewport meta) intact. The nudgeViewport() workaround is removed.
#
# Run this after any bundle rebuild that replaces index.html:
#   bash scripts/fix-viewport.sh
#
# Exit codes:
#   0 — all files already fixed or successfully patched
#   1 — one or more files could not be patched (pattern not found)

set -euo pipefail

DEPLOYMENTS=(
  "/srv/foundry/deployments/media-marketing-landing-1/content/index.html"
  "/srv/foundry/deployments/media-marketing-landing-2/content/index.html"
)

OLD_PATTERN='document\.documentElement\.replaceWith(doc\.documentElement)'
FIXED_MARKER='document\.head\.innerHTML = doc\.head\.innerHTML'

all_ok=0

for FILE in "${DEPLOYMENTS[@]}"; do
  if [[ ! -f "$FILE" ]]; then
    echo "SKIP  $FILE (not found)"
    continue
  fi

  # Already fixed?
  if grep -q "$FIXED_MARKER" "$FILE"; then
    echo "OK    $FILE (already patched)"
    continue
  fi

  # Needs fix?
  if ! grep -q "$OLD_PATTERN" "$FILE"; then
    echo "WARN  $FILE — neither old nor new pattern found; manual inspection needed"
    all_ok=1
    continue
  fi

  echo "PATCH $FILE ..."

  python3 - "$FILE" <<'PYEOF'
import sys, re

path = sys.argv[1]
with open(path, 'r', encoding='utf-8') as f:
    src = f.read()

OLD = (
    '    // Parse the template and swap the root element. Scripts inserted via\n'
    '    // DOMParser/replaceWith are inert per spec — re-create each with\n'
    '    // createElement so they execute, awaiting onload for src scripts to\n'
    '    // preserve ordering (React before ReactDOM before Babel before text/babel).\n'
    '    const doc = new DOMParser().parseFromString(template, \'text/html\');\n'
    '    document.documentElement.replaceWith(doc.documentElement);\n'
    '\n'
    '    // iOS Safari viewport-meta re-evaluation. After documentElement.replaceWith\n'
    '    // the outer-shell <meta name="viewport"> is gone (it lived in the replaced\n'
    '    // <head>). The inner template carries its own viewport meta, but iOS Safari\n'
    '    // (and some Android Chrome builds) do not always re-read meta[name=viewport]\n'
    '    // when the element appears via DOMParser-imported documentElement swap.\n'
    '    // Symptom: page lays out against the 980px fallback "desktop" viewport,\n'
    '    // user sees the .page max-width:1440px shrunk to ~30% on a 390px screen.\n'
    '    // Fix: ensure a viewport meta exists, then force a re-parse by toggling\n'
    '    // its content. Removing-and-reinserting (or .content = .content) is the\n'
    '    // documented iOS-Safari nudge.\n'
    '    (function nudgeViewport() {\n'
    '      var head = document.head;\n'
    '      if (!head) return;\n'
    '      var vp = head.querySelector(\'meta[name="viewport"]\');\n'
    '      if (!vp) {\n'
    '        vp = document.createElement(\'meta\');\n'
    '        vp.setAttribute(\'name\', \'viewport\');\n'
    '        head.insertBefore(vp, head.firstChild);\n'
    '      }\n'
    '      var target = \'width=device-width, initial-scale=1, viewport-fit=cover\';\n'
    '      // First set to a deliberately different value, then back. iOS Safari\n'
    '      // only re-runs viewport calculation on observed mutation of the content\n'
    '      // attribute, not on insertion of an identical-content node.\n'
    '      vp.setAttribute(\'content\', \'width=device-width, initial-scale=1.0001\');\n'
    '      // Force a style flush so the intermediate state is observed.\n'
    '      void document.documentElement.offsetWidth;\n'
    '      vp.setAttribute(\'content\', target);\n'
    '    })();'
)

NEW = (
    '    // Parse the template. Swap <head> content and <body> separately — never\n'
    '    // replacing documentElement. documentElement.replaceWith() caused iOS Safari\n'
    '    // to fall back to the 980px desktop viewport (shrinking max-width:1440px\n'
    '    // layouts to ~30% on 390px screens); the nudgeViewport workaround was\n'
    '    // unreliable. Scripts inserted via innerHTML/replaceWith are inert per spec\n'
    '    // and are re-created below so they execute in order.\n'
    '    const doc = new DOMParser().parseFromString(template, \'text/html\');\n'
    '    if (doc.title) document.title = doc.title;\n'
    '    document.head.innerHTML = doc.head.innerHTML;\n'
    '    // Ensure viewport meta is always present and first in <head>.\n'
    '    (function() {\n'
    '      var vp = document.head.querySelector(\'meta[name="viewport"]\');\n'
    '      if (!vp) { vp = document.createElement(\'meta\'); vp.setAttribute(\'name\', \'viewport\'); }\n'
    '      vp.setAttribute(\'content\', \'width=device-width, initial-scale=1\');\n'
    '      document.head.insertBefore(vp, document.head.firstChild);\n'
    '    })();\n'
    '    document.body.replaceWith(doc.body);'
)

if OLD not in src:
    print(f"  ERROR: exact old pattern not found in {path}; manual patch needed")
    sys.exit(1)

patched = src.replace(OLD, NEW, 1)
with open(path, 'w', encoding='utf-8') as f:
    f.write(patched)
print(f"  patched successfully")
PYEOF

  echo "DONE  $FILE"
done

if [[ $all_ok -ne 0 ]]; then
  echo ""
  echo "One or more files need manual inspection. See WARN lines above."
  exit 1
fi

echo ""
echo "Verification:"
for FILE in "${DEPLOYMENTS[@]}"; do
  [[ -f "$FILE" ]] || continue
  if grep -q "document\.documentElement\.replaceWith(doc" "$FILE"; then
    echo "  FAIL $FILE — old pattern still present"
  else
    echo "  PASS $FILE"
  fi
done
