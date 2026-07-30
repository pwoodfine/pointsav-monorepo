#!/usr/bin/env bash
# fix-font-zoom.sh — Fix the "50% zoom" perception on both marketing sites.
#
# Root causes:
#   1. font-size: 14px base (vs web standard 16px) makes text look tiny
#   2. font-size: 11px navigation/tab text is unusually small
#   3. .doc { margin: 0 56px } inside max-width:1440px means on monitors
#      wider than ~2000px the white content card is only ~50% of screen width
#
# This script patches the __bundler/template JSON string inside each
# deployment index.html to update font sizes and add wide-screen layout fixes.
# It is idempotent — re-running does nothing if already patched.
#
# IMPORTANT: Uses json.JSONDecoder.raw_decode() to extract the template and
# re-encodes with </ → <\/ escaping. This is REQUIRED because the template
# HTML contains </script> tags. Plain json.dumps() leaves those unescaped,
# which causes the browser's HTML parser to terminate the script tag early.
# Never use re.search + json.loads/dumps without this escaping.
#
# Usage: bash scripts/fix-font-zoom.sh

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

  echo "PATCH $FILE ..."

  python3 - "$FILE" <<'PYEOF'
import sys, json

path = sys.argv[1]
with open(path, 'r', encoding='utf-8') as f:
    src = f.read()

# ── Extract template using raw_decode (immune to embedded </script>) ──────────
# NEVER use re.search + json.loads here. The template HTML contains </script>
# tags; plain regex would truncate at the first one. raw_decode parses JSON
# syntax directly and returns (value, end_index_in_source).
start_tag = '<script type="__bundler/template">'
if start_tag not in src:
    print(f"  ERROR: __bundler/template not found in {path}")
    sys.exit(1)

start_idx = src.index(start_tag) + len(start_tag)
while src[start_idx] in ' \t\n\r':
    start_idx += 1

decoder = json.JSONDecoder()
template, end_pos = decoder.raw_decode(src, start_idx)
original_len = len(template)

# ── Apply patches ────────────────────────────────────────────────────────────
applied = []

# 1. Base font size: 14px → 16px
OLD1 = 'font-size: 14px;\n    line-height: 1.55;\n    -webkit-font-smoothing: antialiased;'
NEW1 = 'font-size: 16px;\n    line-height: 1.55;\n    -webkit-font-smoothing: antialiased;'
if OLD1 in template:
    template = template.replace(OLD1, NEW1, 1)
    applied.append('base font-size 14px→16px')
elif NEW1 in template:
    applied.append('base font-size already 16px (skip)')

# 2. Nav text 11px → 13px (Woodfine variant — ink-3 colour)
OLD2 = ('font-size: 11px;\n    font-weight: 600;\n    letter-spacing: 0.14em;\n'
        '    color: var(--ink-3);\n    text-transform: uppercase;')
NEW2 = ('font-size: 13px;\n    font-weight: 600;\n    letter-spacing: 0.10em;\n'
        '    color: var(--ink-3);\n    text-transform: uppercase;')
if OLD2 in template:
    template = template.replace(OLD2, NEW2, 1)
    applied.append('nav font-size 11→13px (Woodfine)')
elif NEW2 in template:
    applied.append('nav 13px Woodfine already (skip)')

# 3. Nav text 11px → 13px (PointSav variant — #164679 colour)
OLD2b = ('font-size: 11px;\n    font-weight: 600;\n    letter-spacing: 0.14em;\n'
         '    color: #164679;\n    text-transform: uppercase;')
NEW2b = ('font-size: 13px;\n    font-weight: 600;\n    letter-spacing: 0.10em;\n'
         '    color: #164679;\n    text-transform: uppercase;')
if OLD2b in template:
    template = template.replace(OLD2b, NEW2b, 1)
    applied.append('nav font-size 11→13px (PointSav)')
elif NEW2b in template:
    applied.append('nav 13px PointSav already (skip)')

# 4. Tab/button 11px → 12px
OLD3 = ('font-size: 11px;\n    font-weight: 700;\n    letter-spacing: 0.16em;\n'
        '    line-height: 1.25;\n    padding: 8px 14px;\n    min-width: 130px;')
NEW3 = ('font-size: 12px;\n    font-weight: 700;\n    letter-spacing: 0.14em;\n'
        '    line-height: 1.25;\n    padding: 8px 14px;\n    min-width: 130px;')
count3 = template.count(OLD3)
if count3 > 0:
    template = template.replace(OLD3, NEW3)
    applied.append(f'tab/btn font-size 11→12px ({count3} occurrences)')
elif NEW3 in template:
    applied.append('tab/btn 12px already (skip)')

# 5. Wide-screen media queries
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
    OLD4 = '\n  @media (max-width: 1200px) {\n    .topnav { padding: 0 36px;'
    if OLD4 in template:
        template = template.replace(OLD4, WIDE_MEDIA + '\n\n  @media (max-width: 1200px) {\n    .topnav { padding: 0 36px;', 1)
        applied.append('wide-screen @media rules added')
    else:
        applied.append('WARN: 1200px anchor not found — wide-screen rules not added')
else:
    applied.append('wide-screen @media already present (skip)')

# ── Re-encode with HTML-safe </ escaping ─────────────────────────────────────
# REQUIRED: json.dumps() leaves </script> unescaped. Inside a <script> tag
# this causes the HTML parser to terminate the tag early, breaking the page.
# Replace </ with <\/ — valid JSON escaping that browsers handle correctly.
new_json = json.dumps(template).replace('</', r'<\/')

# ── Splice back ───────────────────────────────────────────────────────────────
new_src = src[:start_idx] + new_json + src[end_pos:]

with open(path, 'w', encoding='utf-8') as f:
    f.write(new_src)

# Sanity: verify the re-encoded template round-trips cleanly
with open(path, 'r', encoding='utf-8') as f:
    check_src = f.read()
check_start = check_src.index(start_tag) + len(start_tag)
while check_src[check_start] in ' \t\n\r':
    check_start += 1
check_t, _ = json.JSONDecoder().raw_decode(check_src, check_start)
assert 'font-size: 16px' in check_t, 'SANITY: 16px not found after patch'

print(f"  patched successfully:")
for a in applied:
    print(f"    {'!' if 'WARN' in a else '✓'} {a}")
print(f"  template length: {original_len} → {len(check_t)} chars")
PYEOF

  echo "DONE  $FILE"
done

echo ""
echo "=== Verification ==="
python3 - <<'VEOF'
import json

start_tag = '<script type="__bundler/template">'
for fname, label in [
    ("/srv/foundry/deployments/media-marketing-landing-1/content/index.html", "Woodfine"),
    ("/srv/foundry/deployments/media-marketing-landing-2/content/index.html", "PointSav"),
]:
    with open(fname, 'r', encoding='utf-8') as f:
        src = f.read()
    if start_tag not in src:
        print(f"  {label}: template not found"); continue
    si = src.index(start_tag) + len(start_tag)
    while src[si] in ' \t\n\r': si += 1
    t, _ = json.JSONDecoder().raw_decode(src, si)
    base_ok  = 'font-size: 16px' in t
    wide_ok  = 'min-width: 1600px' in t
    nav13_ok = 'font-size: 13px' in t
    print(f"  {label}: base=16px={base_ok}  nav=13px={nav13_ok}  wide-media={wide_ok}")
VEOF
