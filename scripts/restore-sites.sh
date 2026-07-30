#!/usr/bin/env bash
# restore-sites.sh — Restore both marketing sites after the font-zoom patch
# broke them by leaving unescaped </script> inside the __bundler/template tag.
#
# Root cause: json.dumps() does not escape </ sequences. Embedded </script>
# in the template caused the HTML parser (and regex-based readers) to truncate
# the script tag early, leaving an invalid/incomplete JSON string.
#
# This script uses json.JSONDecoder.raw_decode() to correctly extract the full
# template regardless of embedded </script>, applies the font/zoom CSS fixes,
# then re-encodes with </ → <\/ escaping so the HTML parser never mis-terminates
# the script tag again.
#
# Idempotent — safe to run multiple times.
#
# Usage: bash scripts/restore-sites.sh

set -euo pipefail

DEPLOYMENTS=(
  "/srv/foundry/deployments/media-marketing-landing-1/content/index.html"
  "/srv/foundry/deployments/media-marketing-landing-2/content/index.html"
)

all_ok=0

for FILE in "${DEPLOYMENTS[@]}"; do
  if [[ ! -f "$FILE" ]]; then
    echo "SKIP  $FILE (not found)"
    continue
  fi

  echo "RESTORE $FILE ..."

  python3 - "$FILE" <<'PYEOF'
import sys, json, re

path = sys.argv[1]
with open(path, 'r', encoding='utf-8') as f:
    src = f.read()

# ── Step 1: Extract template using raw_decode (immune to embedded </script>) ─
start_tag = '<script type="__bundler/template">'
start_idx = src.index(start_tag) + len(start_tag)

# Skip any whitespace between the tag and the JSON string
while src[start_idx] in ' \t\n\r':
    start_idx += 1

decoder = json.JSONDecoder()
try:
    template, end_pos = decoder.raw_decode(src, start_idx)
except json.JSONDecodeError as e:
    print(f"  ERROR: failed to parse template even with raw_decode: {e}")
    print(f"  Chars around start: {repr(src[start_idx:start_idx+100])}")
    sys.exit(1)

print(f"  template extracted: {len(template)} chars, end_pos={end_pos}")

# ── Step 2: Apply CSS patches ─────────────────────────────────────────────────
applied = []

# 2a. Base font-size: 14px → 16px
OLD_BASE = 'font-size: 14px;\n    line-height: 1.55;\n    -webkit-font-smoothing: antialiased;'
NEW_BASE = 'font-size: 16px;\n    line-height: 1.55;\n    -webkit-font-smoothing: antialiased;'
if OLD_BASE in template:
    template = template.replace(OLD_BASE, NEW_BASE, 1)
    applied.append('base font-size 14px→16px')
elif NEW_BASE in template:
    applied.append('base font-size already 16px (skip)')

# 2b. Woodfine nav: letter-spacing 0.14em + font-size 11px (ink-3 colour variant)
OLD_NAV_WF = ('font-size: 11px;\n    font-weight: 600;\n    letter-spacing: 0.14em;\n'
              '    color: var(--ink-3);\n    text-transform: uppercase;')
NEW_NAV_WF = ('font-size: 13px;\n    font-weight: 600;\n    letter-spacing: 0.10em;\n'
              '    color: var(--ink-3);\n    text-transform: uppercase;')
if OLD_NAV_WF in template:
    template = template.replace(OLD_NAV_WF, NEW_NAV_WF, 1)
    applied.append('nav font-size 11→13px (Woodfine variant)')
elif NEW_NAV_WF in template:
    applied.append('nav font-size already 13px Woodfine (skip)')

# 2c. PointSav nav: letter-spacing 0.14em + font-size 11px (#164679 colour variant)
OLD_NAV_PS = ('font-size: 11px;\n    font-weight: 600;\n    letter-spacing: 0.14em;\n'
              '    color: #164679;\n    text-transform: uppercase;')
NEW_NAV_PS = ('font-size: 13px;\n    font-weight: 600;\n    letter-spacing: 0.10em;\n'
              '    color: #164679;\n    text-transform: uppercase;')
if OLD_NAV_PS in template:
    template = template.replace(OLD_NAV_PS, NEW_NAV_PS, 1)
    applied.append('nav font-size 11→13px (PointSav variant)')
elif NEW_NAV_PS in template:
    applied.append('nav font-size already 13px PointSav (skip)')

# 2d. Tab/button font-size 11px → 12px
OLD_TAB = ('font-size: 11px;\n    font-weight: 700;\n    letter-spacing: 0.16em;\n'
           '    line-height: 1.25;\n    padding: 8px 14px;\n    min-width: 130px;')
NEW_TAB = ('font-size: 12px;\n    font-weight: 700;\n    letter-spacing: 0.14em;\n'
           '    line-height: 1.25;\n    padding: 8px 14px;\n    min-width: 130px;')
count_tab = template.count(OLD_TAB)
if count_tab > 0:
    template = template.replace(OLD_TAB, NEW_TAB)
    applied.append(f'tab/btn font-size 11→12px ({count_tab} occurrences)')
elif NEW_TAB in template:
    applied.append('tab/btn font-size already 12px (skip)')

# 2e. Wide-screen media queries (only if not already present)
WIDE_MARKER = '@media (min-width: 1600px)'
if WIDE_MARKER not in template:
    WIDE_MEDIA = (
        '\n  @media (min-width: 1600px) {\n'
        '    .doc { margin: 0 36px; }\n'
        '    .props { padding: 36px 80px 56px; }\n'
        '    .compliance { margin: 0 36px; padding: 28px 80px 32px; }\n'
        '  }\n'
        '  @media (min-width: 1920px) {\n'
        '    .doc { margin: 0 24px; }\n'
        '    .props { padding: 36px 48px 56px; }\n'
        '    .compliance { margin: 0 24px; padding: 28px 48px 32px; }\n'
        '  }'
    )
    OLD_MEDIA = '\n  @media (max-width: 1200px) {\n    .topnav { padding: 0 36px;'
    if OLD_MEDIA in template:
        template = template.replace(OLD_MEDIA, WIDE_MEDIA + '\n\n  @media (max-width: 1200px) {\n    .topnav { padding: 0 36px;', 1)
        applied.append('wide-screen @media rules added')
    else:
        applied.append('WARN: could not find 1200px breakpoint anchor — wide-screen rules not added')
else:
    applied.append('wide-screen @media already present (skip)')

# ── Step 3: Re-encode with HTML-safe </ escaping ──────────────────────────────
new_json = json.dumps(template)
# Escape </ so the HTML parser never terminates the script tag early.
# json.dumps leaves </script> as-is; we must encode it before inserting into HTML.
new_json = new_json.replace('</', r'<\/')

# ── Step 4: Splice back into the file ────────────────────────────────────────
new_src = src[:start_idx] + new_json + src[end_pos:]

with open(path, 'w', encoding='utf-8') as f:
    f.write(new_src)

# ── Step 5: Report ────────────────────────────────────────────────────────────
print(f"  re-encoded: {len(new_json)} chars (was {end_pos - start_idx})")
print(f"  applied:")
for a in applied:
    print(f"    {'✓' if 'WARN' not in a else '!'} {a}")

# Quick sanity: verify the JSON can be extracted again using the same method
with open(path, 'r', encoding='utf-8') as f:
    check_src = f.read()
check_start = check_src.index(start_tag) + len(start_tag)
while check_src[check_start] in ' \t\n\r':
    check_start += 1
check_template, _ = json.JSONDecoder().raw_decode(check_src, check_start)
assert 'font-size: 16px' in check_template or NEW_BASE in check_template, \
    "SANITY FAIL: 16px not found in re-decoded template"
print(f"  sanity check: PASS (font-size: 16px confirmed in decoded template)")
PYEOF

  echo "DONE  $FILE"
  echo ""
done

echo "=== Re-applying viewport patch ==="
bash "$(dirname "$0")/fix-viewport.sh"

echo ""
echo "=== Live state ==="
bash "$(dirname "$0")/verify-live.sh"
