#!/usr/bin/env bash
# fix-loading.sh — Remove the loading splash screen and unused JS chunks
# from both marketing deployment HTML files.
#
# Changes applied:
#   1. Remove #__bundler_thumbnail CSS + div (the full-screen blue loading screen)
#   2. Change outer shell body background from #164679 → #F7F9FA (neutral page colour)
#      so the brief blank state while the bundle parses matches the page background
#   3. Remove the 4 dev-only JS chunks from the manifest:
#        9f4523f8 = Babel.js 3.1 MB decompressed  (in-browser JSX compiler)
#        9302191e = React + ReactDOM 1.08 MB
#        1f4b13c0 = React core 110 KB
#        a3391d3b = Tweaks panel JSX source 24 KB
#      These chunks power the developer tweaks panel only. Visitors never see it.
#      Removing them cuts file size from ~2.4 MB to ~1.2 MB (-50%) and eliminates
#      ~3.1 MB of in-browser gzip decompress + JS parse + Babel compilation.
#   4. Remove the 4 corresponding <script> tags + <div id="tweaks-root"> from template
#
# Run order after any bundle rebuild:
#   bash scripts/fix-loading.sh    ← this script
#   bash scripts/fix-font-zoom.sh
#   bash scripts/fix-viewport.sh
#   bash scripts/verify-live.sh
#
# Idempotent — safe to re-run.

set -euo pipefail

DEPLOYMENTS=(
  "/srv/foundry/deployments/media-marketing-landing-1/content/index.html"
  "/srv/foundry/deployments/media-marketing-landing-2/content/index.html"
)

for FILE in "${DEPLOYMENTS[@]}"; do
  if [[ ! -f "$FILE" ]]; then
    echo "SKIP  $FILE (not found)"
    continue
  fi

  echo "PATCH $FILE ..."

  python3 - "$FILE" <<'PYEOF'
import sys, re, json

path = sys.argv[1]
with open(path, 'r', encoding='utf-8') as f:
    src = f.read()

applied = []

# ── OUTER SHELL EDITS (plain HTML — no JSON encoding involved) ────────────────

# 1. Remove thumbnail CSS rules (background colour varies per tenant)
thumbnail_css = re.search(
    r'\n    #__bundler_thumbnail \{[^}]+\}\n'
    r'    #__bundler_thumbnail svg \{[^}]+\}\n'
    r'    #__bundler_placeholder \{[^}]+\}',
    src
)
if thumbnail_css:
    src = src[:thumbnail_css.start()] + src[thumbnail_css.end():]
    applied.append('removed #__bundler_thumbnail CSS')
elif '#__bundler_thumbnail' not in src:
    applied.append('#__bundler_thumbnail CSS already absent (skip)')
else:
    applied.append('WARN: thumbnail CSS regex did not match — manual check needed')

# 2. Neutral body background (background colour varies per tenant — match any hex)
bg_match = re.search(r'(    body \{ background: )#[0-9A-Fa-f]{3,6}(;)', src)
if bg_match and bg_match.group(0) != '    body { background: #F7F9FA;':
    old_bg = bg_match.group(0)
    new_bg = bg_match.group(1) + '#F7F9FA' + bg_match.group(2)
    src = src.replace(old_bg, new_bg, 1)
    applied.append(f'outer shell body background {old_bg.split("#")[1][:6]} → F7F9FA')
elif '    body { background: #F7F9FA;' in src:
    applied.append('outer shell body background already F7F9FA (skip)')

# 3. Remove the thumbnail div (contains the large embedded SVG logo)
# Matches from <div id="__bundler_thumbnail"> to its closing </div>,
# including the leading newline and two-space indent.
thumbnail_div = re.search(
    r'\n  <div id="__bundler_thumbnail">.*?</div>',
    src, re.DOTALL
)
if thumbnail_div:
    src = src[:thumbnail_div.start()] + src[thumbnail_div.end():]
    applied.append('removed <div id="__bundler_thumbnail"> (SVG splash screen)')
elif '<div id="__bundler_thumbnail">' not in src:
    applied.append('thumbnail div already absent (skip)')
else:
    applied.append('WARN: thumbnail div regex did not match — manual check needed')

# ── MANIFEST EDIT (flat JSON object — no embedded HTML) ──────────────────────
# Remove ALL text/javascript and text/jsx chunks — exclusively the dev-only
# tweaks panel (React + Babel). Detected by MIME type, not hardcoded UUID,
# so this works regardless of per-tenant chunk IDs.
js_cids = []   # populated here, reused in template section below
manifest_tag = '<script type="__bundler/manifest">'
if manifest_tag in src:
    m_start = src.index(manifest_tag) + len(manifest_tag)
    m_end   = src.index('</script>', m_start)
    manifest = json.loads(src[m_start:m_end].strip())
    js_cids = [cid for cid, info in manifest.items()
               if info.get('mime', '') in ('text/javascript', 'text/jsx', 'text/babel')]
    if js_cids:
        for cid in js_cids:
            del manifest[cid]
        src = src[:m_start] + json.dumps(manifest) + src[m_end:]
        applied.append(f'removed {len(js_cids)} JS/JSX chunks from manifest: {", ".join(c[:8] for c in js_cids)}')
    else:
        applied.append('JS chunks already absent from manifest (skip)')

# ── TEMPLATE EDITS (use raw_decode — immune to embedded </script>) ────────────

start_tag = '<script type="__bundler/template">'
if start_tag in src:
    si = src.index(start_tag) + len(start_tag)
    while src[si] in ' \t\n\r': si += 1
    decoder = json.JSONDecoder()
    template, end_pos = decoder.raw_decode(src, si)
    orig_len = len(template)
    t_applied = []

    # a. Remove <script> tags referencing removed JS chunks (by UUID).
    #    js_cids populated in manifest section above; empty on idempotent runs.
    removed_script_count = 0
    for cid in js_cids:
        if cid in template:
            idx = template.find(cid)
            tag_start = template.rfind('<script', 0, idx)
            tag_end   = template.find('</script>', idx) + 9
            template = template[:tag_start] + template[tag_end:]
            removed_script_count += 1
    if removed_script_count:
        t_applied.append(f'removed {removed_script_count} <script src="UUID"> tags')
    else:
        t_applied.append('UUID-based <script> tags already absent (skip)')

    # c. Remove the inline <script type="text/babel"> block (TWEAK_DEFAULTS code)
    inline_babel = re.search(r'<script type="text/babel">.*?</script>', template, re.DOTALL)
    if inline_babel:
        template = template[:inline_babel.start()] + template[inline_babel.end():]
        t_applied.append('removed inline <script type="text/babel"> (TWEAK_DEFAULTS)')
    else:
        t_applied.append('inline text/babel already absent (skip)')

    # d. Remove <div id="tweaks-root"></div>
    TWEAKS_DIV = '<div id="tweaks-root"></div>'
    if TWEAKS_DIV in template:
        template = template.replace(TWEAKS_DIV, '', 1)
        t_applied.append('removed <div id="tweaks-root"></div>')
    else:
        t_applied.append('tweaks-root div already absent (skip)')

    # Re-encode with HTML-safe </ escaping (prevents HTML parser from
    # mis-terminating the script tag on embedded </script> sequences)
    new_json = json.dumps(template).replace('</', r'<\/')
    src = src[:si] + new_json + src[end_pos:]
    applied.append(f'template: {", ".join(t_applied)}')
    applied.append(f'  template size: {orig_len} → {len(template)} chars')

# ── Write + sanity check ──────────────────────────────────────────────────────
with open(path, 'w', encoding='utf-8') as f:
    f.write(src)

# Verify template still round-trips
with open(path, 'r', encoding='utf-8') as f:
    check = f.read()
if '<script type="__bundler/template">' in check:
    ci = check.index('<script type="__bundler/template">') + len('<script type="__bundler/template">')
    while check[ci] in ' \t\n\r': ci += 1
    t2, _ = json.JSONDecoder().raw_decode(check, ci)
    assert len(t2) > 10000, 'SANITY: template suspiciously short after patch'

import os
size_kb = os.path.getsize(path) // 1024
print(f"  patched: {size_kb} KB")
for a in applied:
    prefix = '  !' if 'WARN' in a else ('    ' if a.startswith('  ') else '  ✓')
    print(f"{prefix} {a}")
PYEOF

  echo "DONE  $FILE"
  echo ""
done

echo "=== Re-applying viewport patch ==="
bash "$(dirname "$0")/fix-viewport.sh"

echo ""
echo "=== Live state ==="
bash "$(dirname "$0")/verify-live.sh"
