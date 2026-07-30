#!/usr/bin/env bash
# apply-mobile-fixes.sh — Apply mobile logo-bar improvements to both live sites.
#
# Idempotent — safe to re-run. Each patch checks for old string before replacing;
# already-patched sites are skipped with a "(skip)" note.
#
# Changes applied:
#   S1: Fluid logo (height:auto + max-width:100%; 160→180px at ≤480px)
#   S2: Nav font 9→11px, gap 12→14px at ≤480px breakpoint
#   S3: Nav links as 44px tap targets (min-height:44px)
#   S4: Bottom divider on collapsed header at ≤768px
#   S5: Hide .topnav .left (Disclaimer + Contact us) on mobile ≤768px; footer links remain
#   S6: Stack Manifest/BIM Library/Location Intelligence vertically on mobile (Woodfine only)
#   S7: Collapsible disclaimer — <details>/<summary> on both sites; collapsed by default
#   W1: Strip redundant width="320" height="80" SVG attrs (Woodfine)
#   P1: Remove empty <a href="#"> from PointSav right nav
#   P2: Strip redundant width="320" height="80" SVG attrs (PointSav)
#   P3: Equalise Monorepo + Design System button sizes at ≤768px and ≤480px (PointSav)
#   P4: Stack Monorepo + Design System buttons vertically on mobile (PointSav)
#   S8: 3-column icon grid at ≤480px; remove negative-margin overflow hack (both sites)
#   S9: Single-column icon stack at ≤480px; 160px icons (both sites)
#   S10: White card + tinted icon chip + hover lift at ≤768px (both sites)
#
# Usage: bash scripts/apply-mobile-fixes.sh

set -euo pipefail

WOODFINE="/srv/foundry/deployments/media-marketing-landing-1/content/index.html"
POINTSAV="/srv/foundry/deployments/media-marketing-landing-2/content/index.html"

for f in "$WOODFINE" "$POINTSAV"; do
  [[ -f "$f" ]] || { echo "SKIP $f (not found)"; continue; }
done

python3 - "$WOODFINE" "$POINTSAV" << 'PYEOF'
import sys, json, re

WOODFINE = sys.argv[1]
POINTSAV = sys.argv[2]

def load_template(path):
    with open(path, 'r', encoding='utf-8') as f:
        src = f.read()
    start_tag = '<script type="__bundler/template">'
    start_idx = src.index(start_tag) + len(start_tag)
    while src[start_idx] in ' \t\n\r':
        start_idx += 1
    decoder = json.JSONDecoder()
    template, end_pos = decoder.raw_decode(src, start_idx)
    return src, template, start_idx, end_pos

def save_template(path, src, template, start_idx, end_pos):
    new_json = json.dumps(template)
    new_json = new_json.replace('</', r'<\/')
    new_src = src[:start_idx] + new_json + src[end_pos:]
    with open(path, 'w', encoding='utf-8') as f:
        f.write(new_src)
    src2, t2, _, _ = load_template(path)
    return t2

def apply_shared(template):
    applied = []

    # S1a — base logo: height:80px → auto + max-width:100%
    S1A_OLD = ('.wordmark img, .wordmark .logo-svg {\n'
               '    display: block;\n'
               '    height: 80px;\n'
               '    width: 320px;\n'
               '    /* SVG has institutional-fill paths — set color via filter-free approach: */\n'
               '    /* the SVG uses currentColor where possible; otherwise it carries its own fill */\n'
               '  }')
    S1A_NEW = ('.wordmark img, .wordmark .logo-svg {\n'
               '    display: block;\n'
               '    height: auto;\n'
               '    width: 320px;\n'
               '    max-width: 100%;\n'
               '    /* SVG has institutional-fill paths — set color via filter-free approach: */\n'
               '    /* the SVG uses currentColor where possible; otherwise it carries its own fill */\n'
               '  }')
    if S1A_OLD in template:
        template = template.replace(S1A_OLD, S1A_NEW, 1)
        applied.append('S1a: base logo height:80px→auto + max-width:100%')
    elif S1A_NEW in template:
        applied.append('S1a: already patched (skip)')
    else:
        applied.append('S1a: WARN not found')

    # S1b — 768px logo: height:50px → auto
    S1B_OLD = '.wordmark .logo-svg { width: 200px; height: 50px; }'
    S1B_NEW = '.wordmark .logo-svg { width: 200px; height: auto; }'
    if S1B_OLD in template:
        template = template.replace(S1B_OLD, S1B_NEW, 1)
        applied.append('S1b: 768px logo height:50px→auto')
    elif S1B_NEW in template:
        applied.append('S1b: already patched (skip)')
    else:
        applied.append('S1b: WARN not found')

    # S1c — 480px logo: 160×40 → 180×auto
    S1C_OLD = '.wordmark .logo-svg { width: 160px; height: 40px; }'
    S1C_NEW = '.wordmark .logo-svg { width: 180px; height: auto; }'
    if S1C_OLD in template:
        template = template.replace(S1C_OLD, S1C_NEW, 1)
        applied.append('S1c: 480px logo 160x40→180xauto')
    elif S1C_NEW in template:
        applied.append('S1c: already patched (skip)')
    else:
        applied.append('S1c: WARN not found')

    # S2 — 480px nav: 9px → 11px, gap 12→14px
    S2_OLD = '.topnav .left, .topnav .right { gap: 12px; font-size: 9px; }'
    S2_NEW = '.topnav .left, .topnav .right { gap: 14px; font-size: 11px; }'
    if S2_OLD in template:
        template = template.replace(S2_OLD, S2_NEW, 1)
        applied.append('S2: 480px nav font 9→11px, gap 12→14px')
    elif S2_NEW in template:
        applied.append('S2: already patched (skip)')
    else:
        applied.append('S2: WARN not found')

    # S3 — Tap-target rule
    S3_ANCHOR = '.topnav .left, .topnav .right { font-family: var(--display); font-weight: 500; letter-spacing: 0.16em; }'
    S3_ADDITION = ('\n  .topnav .left a, .topnav .right a {\n'
                   '    display: inline-flex;\n'
                   '    align-items: center;\n'
                   '    min-height: 44px;\n'
                   '    padding: 4px 0;\n'
                   '    text-decoration: none;\n'
                   '  }')
    TAP_MARKER = 'min-height: 44px;\n    padding: 4px 0;\n    text-decoration: none;'
    if TAP_MARKER not in template and S3_ANCHOR in template:
        template = template.replace(S3_ANCHOR, S3_ANCHOR + S3_ADDITION, 1)
        applied.append('S3: nav tap-target rule (min-height:44px) added')
    elif TAP_MARKER in template:
        applied.append('S3: already patched (skip)')
    else:
        applied.append('S3: WARN anchor not found')

    # S4 — 768px topnav border-bottom
    S4_OLD = ('.topnav {\n'
              '      grid-template-columns: 1fr;\n'
              '      grid-template-rows: auto auto;\n'
              '      padding: 0 16px;\n'
              '      margin-bottom: 16px;\n'
              '      gap: 12px;\n'
              '      justify-items: center;\n'
              '    }')
    S4_NEW = ('.topnav {\n'
              '      grid-template-columns: 1fr;\n'
              '      grid-template-rows: auto auto;\n'
              '      padding: 0 16px;\n'
              '      padding-bottom: 12px;\n'
              '      margin-bottom: 16px;\n'
              '      gap: 12px;\n'
              '      justify-items: center;\n'
              '      border-bottom: 1px solid var(--rule);\n'
              '    }')
    S4_MARKER = 'border-bottom: 1px solid var(--rule);\n    }'
    if S4_OLD in template:
        template = template.replace(S4_OLD, S4_NEW, 1)
        applied.append('S4: 768px topnav border-bottom divider')
    elif S4_MARKER in template:
        applied.append('S4: already patched (skip)')
    else:
        applied.append('S4: WARN not found')

    # S5 — Hide .topnav .left on mobile ≤768px; footer links remain visible
    S5_OLD = '.topnav .left, .topnav .right { justify-content: center; gap: 20px; font-size: 10px; }'
    S5_NEW = '.topnav .left { display: none; }\n    .topnav .right { justify-content: center; gap: 20px; font-size: 10px; }'
    S5_MARKER = '.topnav .left { display: none; }'
    if S5_OLD in template:
        template = template.replace(S5_OLD, S5_NEW, 1)
        applied.append('S5: .topnav .left hidden at ≤768px (Disclaimer/Contact moved to footer)')
    elif S5_MARKER in template:
        applied.append('S5: already patched (skip)')
    else:
        applied.append('S5: WARN not found')

    # S7 — Collapsible disclaimer: <details>/<summary> wrapper + CSS (both sites)
    import re as _re
    S7_H4_OLD  = '<h4>Important Information</h4>'
    S7_H4_NEW  = ('<details class="compliance-details">\n'
                  '      <summary>Important Information</summary>')
    S7_CSS_ANC = '.compliance p:last-child { margin-bottom: 0; }'
    S7_CSS_ADD = ('\n  .compliance-details > summary {\n'
                  '    display: block;\n'
                  '    margin: 0;\n'
                  '    font-family: var(--display);\n'
                  '    font-size: 11px;\n'
                  '    font-weight: 700;\n'
                  '    letter-spacing: 0.22em;\n'
                  '    text-transform: uppercase;\n'
                  '    color: var(--ink-3);\n'
                  '    cursor: pointer;\n'
                  '    user-select: none;\n'
                  '  }\n'
                  '  .compliance-details > summary::marker,\n'
                  '  .compliance-details > summary::-webkit-details-marker { display: none; }\n'
                  "  .compliance-details > summary::after { content: ' \\u25be'; }\n"
                  '  .compliance-details[open] > summary { margin-bottom: 14px; }\n'
                  "  .compliance-details[open] > summary::after { content: ' \\u25b4'; }")
    S7_MARKER  = 'class="compliance-details"'
    if S7_MARKER in template:
        applied.append('S7: already patched (skip)')
    else:
        if S7_H4_OLD in template:
            template = template.replace(S7_H4_OLD, S7_H4_NEW, 1)
            applied.append('S7a: <h4> → <details><summary>')
        else:
            applied.append('S7a: WARN — h4 not found')
        before = len(template)
        template = _re.sub(
            r'(class="compliance-details">.*?</p>)\s*</section>',
            r'\1\n    </details>\n  </section>',
            template, flags=_re.DOTALL, count=1)
        if len(template) != before:
            applied.append('S7b: </details> inserted before </section>')
        else:
            applied.append('S7b: WARN — regex found no match')
        if S7_CSS_ANC in template and S7_CSS_ADD not in template:
            template = template.replace(S7_CSS_ANC, S7_CSS_ANC + S7_CSS_ADD, 1)
            applied.append('S7c: summary CSS added')
        else:
            applied.append('S7c: CSS already present (skip)')

    # S8 — 3-col icon grid at ≤480px; remove negative margin hack (both sites)
    S8_OLD = ('.classes { grid-template-columns: repeat(2, 1fr); padding: 0 8px; }\n'
              '    .class .icon { height: 120px; }\n'
              '    .class .icon svg, .class .icon .class-icon { width: 160px; height: 160px; margin: 0 -60px; }')
    S8_NEW = ('.classes { grid-template-columns: repeat(3, 1fr); padding: 0 8px; }\n'
              '    .class .icon { height: 90px; }\n'
              '    .class .icon svg, .class .icon .class-icon { width: 96px; height: 96px; margin: 0; }\n'
              '    .class .label { font-size: 10px; }')
    if S8_OLD in template:
        template = template.replace(S8_OLD, S8_NEW, 1)
        applied.append('S8: 3-col icon grid applied, negative margins removed')
    elif S8_NEW in template or '.classes { grid-template-columns: 1fr; padding: 0 8px; }' in template:
        applied.append('S8: already patched (skip)')
    else:
        applied.append('S8: WARN not found')

    # S9 — single-column icon stack at ≤480px; 160px icons (replaces S8's 3-col/96px)
    S9_OLD = ('.classes { grid-template-columns: repeat(3, 1fr); padding: 0 8px; }\n'
              '    .class .icon { height: 90px; }\n'
              '    .class .icon svg, .class .icon .class-icon { width: 96px; height: 96px; margin: 0; }\n'
              '    .class .label { font-size: 10px; }')
    S9_NEW = ('.classes { grid-template-columns: 1fr; padding: 0 8px; }\n'
              '    .class .icon { height: 160px; }\n'
              '    .class .icon svg, .class .icon .class-icon { width: 160px; height: 160px; margin: 0; }')
    if S9_OLD in template:
        template = template.replace(S9_OLD, S9_NEW, 1)
        applied.append('S9: single-column icon stack applied')
    elif S9_NEW in template:
        applied.append('S9: already patched (skip)')
    else:
        applied.append('S9: WARN not found')

    # S10 — white card + tinted icon chip + hover lift at ≤768px (both sites)
    S10_OLD = '.classes { grid-template-columns: repeat(2, 1fr); padding: 0 12px; }'
    S10_NEW = ('.classes { grid-template-columns: repeat(2, 1fr); padding: 0 12px; gap: 12px; }\n'
               '    .class { background: #fff; border: 1px solid #d1d5db; border-radius: 10px; '
               'padding: 12px; box-shadow: 0 1px 2px rgba(16,24,40,.06), 0 1px 3px rgba(16,24,40,.10); '
               'transition: box-shadow .18s ease, transform .18s ease; }\n'
               '    .class:hover, .class:active { transform: translateY(-2px); '
               'box-shadow: 0 6px 16px rgba(16,24,40,.12); }\n'
               '    .class .icon { background: rgba(22,70,121,.07); border-radius: 8px; }')
    if S10_OLD in template:
        template = template.replace(S10_OLD, S10_NEW, 1)
        applied.append('S10: card + chip + hover applied')
    elif S10_NEW in template:
        applied.append('S10: already patched (skip)')
    else:
        applied.append('S10: WARN not found')

    return template, applied

# Woodfine
print(f"\n=== Woodfine ===")
src, t, si, ep = load_template(WOODFINE)
t, log = apply_shared(t)
W1_OLD = ' width="320" height="80"'
if W1_OLD in t:
    t = t.replace(W1_OLD, '')
    log.append('W1: SVG width/height attrs removed')
else:
    log.append('W1: already removed (skip)')
# S6a — Woodfine HTML: remove <br> from Location Intelligence
S6A_OLD = '>Location<br>Intelligence</a>'
S6A_NEW = '>Location Intelligence</a>'
if S6A_OLD in t:
    t = t.replace(S6A_OLD, S6A_NEW, 1)
    log.append('S6a: <br> removed from Location Intelligence')
else:
    log.append('S6a: already removed (skip)')

# S6b — Woodfine CSS 768px: stack subnav vertically
S6B_OLD = ('.subnav { padding: 10px 16px; }\n'
           '    .subnav .tab { min-width: 100px; font-size: 10px; }')
S6B_NEW = ('.subnav { padding: 10px 16px; }\n'
           '    .subnav .tabs-right { flex-direction: column; }\n'
           '    .subnav .tab, .subnav a.manifest-btn { width: 100%; min-width: unset; font-size: 11px; box-sizing: border-box; }')
S6B_MARKER = '.subnav .tabs-right { flex-direction: column; }'
if S6B_OLD in t:
    t = t.replace(S6B_OLD, S6B_NEW, 1)
    log.append('S6b: 768px subnav stacked layout applied')
elif S6B_MARKER in t:
    log.append('S6b: already patched (skip)')
else:
    log.append('S6b: WARN not found')

# S6c — Woodfine CSS 480px: update subnav tab sizing
S6C_OLD = ('.subnav { padding: 8px 12px; }\n'
           '    .subnav .tab { min-width: 80px; padding: 6px 10px; font-size: 9px; }')
S6C_NEW = ('.subnav { padding: 8px 12px; }\n'
           '    .subnav .tab, .subnav a.manifest-btn { font-size: 11px; padding: 8px 14px; }')
S6C_MARKER = '.subnav .tab, .subnav a.manifest-btn { font-size: 11px; padding: 8px 14px; }'
if S6C_OLD in t:
    t = t.replace(S6C_OLD, S6C_NEW, 1)
    log.append('S6c: 480px subnav font/padding updated')
elif S6C_MARKER in t:
    log.append('S6c: already patched (skip)')
else:
    log.append('S6c: WARN not found')

t_check = save_template(WOODFINE, src, t, si, ep)
for l in log: print(f"  {'✓' if 'WARN' not in l else '!'} {l}")

# PointSav
print(f"\n=== PointSav ===")
src, t, si, ep = load_template(POINTSAV)
t, log = apply_shared(t)
P1_OLD = '<a href="#"></a>\n      <a href="https://documentation.pointsav.com/"'
P1_NEW = '<a href="https://documentation.pointsav.com/"'
if P1_OLD in t:
    t = t.replace(P1_OLD, P1_NEW, 1)
    log.append('P1: empty anchor removed')
else:
    log.append('P1: already removed (skip)')
P2_OLD = ' width="320" height="80"'
if P2_OLD in t:
    t = t.replace(P2_OLD, '')
    log.append('P2: SVG width/height attrs removed')
else:
    log.append('P2: already removed (skip)')
P3_OLD_768 = '.subnav .tab { min-width: 100px; font-size: 10px; }'
P3_NEW_768 = '.subnav .tab, a.monorepo-btn { min-width: 100px; font-size: 10px; }'
P3_OLD_480 = '.subnav .tab { min-width: 80px; padding: 6px 10px; font-size: 9px; }'
P3_NEW_480 = '.subnav .tab, a.monorepo-btn { min-width: 80px; padding: 6px 10px; font-size: 9px; } a.monorepo-btn svg { width: 10px; height: 10px; }'
P4_MARKER_768 = '.subnav .tabs-right { flex-direction: column; }'
P4_MARKER_480 = '.subnav .tab, a.monorepo-btn { font-size: 11px; padding: 8px 14px; }'
if P3_OLD_768 in t:
    t = t.replace(P3_OLD_768, P3_NEW_768, 1)
    log.append('P3: 768px monorepo-btn equalised with tab')
elif P3_NEW_768 in t or P4_MARKER_768 in t:
    log.append('P3: 768px already patched (skip)')
else:
    log.append('P3: 768px WARN not found')
if P3_OLD_480 in t:
    t = t.replace(P3_OLD_480, P3_NEW_480, 1)
    log.append('P3: 480px monorepo-btn equalised with tab + icon scaled')
elif P3_NEW_480 in t or P4_MARKER_480 in t:
    log.append('P3: 480px already patched (skip)')
else:
    log.append('P3: 480px WARN not found')
P4_OLD_768 = '.subnav .tab, a.monorepo-btn { min-width: 100px; font-size: 10px; }'
P4_NEW_768 = '.subnav .tabs-right { flex-direction: column; }\n  .subnav .tab, a.monorepo-btn { width: 100%; min-width: unset; font-size: 11px; box-sizing: border-box; }'
P4_OLD_480 = '.subnav .tab, a.monorepo-btn { min-width: 80px; padding: 6px 10px; font-size: 9px; } a.monorepo-btn svg { width: 10px; height: 10px; }'
P4_NEW_480 = '.subnav .tab, a.monorepo-btn { font-size: 11px; padding: 8px 14px; }'
if P4_OLD_768 in t:
    t = t.replace(P4_OLD_768, P4_NEW_768, 1)
    log.append('P4: 768px stacking applied')
elif P4_NEW_768 in t:
    log.append('P4: 768px already patched (skip)')
else:
    log.append('P4: 768px WARN not found')
if P4_OLD_480 in t:
    t = t.replace(P4_OLD_480, P4_NEW_480, 1)
    log.append('P4: 480px stacking applied')
elif P4_NEW_480 in t:
    log.append('P4: 480px already patched (skip)')
else:
    log.append('P4: 480px WARN not found')
t_check = save_template(POINTSAV, src, t, si, ep)
for l in log: print(f"  {'✓' if 'WARN' not in l else '!'} {l}")

print("\n=== Done — run bash scripts/verify-live.sh to confirm ===")
PYEOF
