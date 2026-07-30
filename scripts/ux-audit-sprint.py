#!/usr/bin/env python3
# ux-audit-sprint.py — Apply 10 UX audit items to both marketing landing sites.
# Idempotent. Run: python3 scripts/ux-audit-sprint.py
#
# Covers:
#   U1  Google Fonts → self-hosted (GDPR)
#   U2  PointSav P0 typos: F*KEYS CONSSOLE, DIGTIAL TWIN
#   U3  Woodfine P0 typo: "is an real property developer"
#   U4  PointSav h1 — verify already present
#   U5  Nav font minimum 14px on mobile
#   U6  Footer: remove internal repo path (factory-release-engineering)
#   U7  PointSav navy #164679 as dominant brand color
#   U8  Unpacking splash — verify already removed
#   U9  Nav consolidation across pages
#   U10 Fix dead href="#" in contact page footers

import sys, json, re, os
from pathlib import Path
import urllib.request

LAND1 = Path("/srv/foundry/deployments/media-marketing-landing-1/content")
LAND2 = Path("/srv/foundry/deployments/media-marketing-landing-2/content")

GOOGLE_UA = ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
             "AppleWebKit/537.36 (KHTML, like Gecko) "
             "Chrome/124.0 Safari/537.36")


# ─── Template JSON helpers (same pattern as apply-mobile-fixes.sh) ────────────

def load_template(path: Path):
    src = path.read_text(encoding="utf-8")
    start_tag = '<script type="__bundler/template">'
    start_idx = src.index(start_tag) + len(start_tag)
    while src[start_idx] in " \t\n\r":
        start_idx += 1
    decoder = json.JSONDecoder()
    template, end_pos = decoder.raw_decode(src, start_idx)
    return src, template, start_idx, end_pos


def save_template(path: Path, src, template, start_idx, end_pos):
    new_json = json.dumps(template).replace("</", r"<\/")
    new_src = src[:start_idx] + new_json + src[end_pos:]
    path.write_text(new_src, encoding="utf-8")


# ─── U1: Google Fonts → self-hosted ──────────────────────────────────────────

def _fetch(url, headers=None):
    req = urllib.request.Request(url, headers=headers or {"User-Agent": GOOGLE_UA})
    with urllib.request.urlopen(req, timeout=20) as r:
        return r.read()


def download_fonts_for_dir(content_dir: Path):
    """Find Google Fonts CSS URL in any HTML file, download fonts, return face CSS."""
    css_url = None
    for f in sorted(content_dir.glob("*.html")):
        src = f.read_text(encoding="utf-8")
        m = re.search(r'<link href="(https://fonts\.googleapis\.com/css[^"]+)"', src)
        if m:
            css_url = m.group(1)
            break
    if not css_url:
        return None, None

    print(f"  Fetching: {css_url[:75]}...")
    try:
        css = _fetch(css_url).decode("utf-8")
    except Exception as e:
        print(f"  ! WARN: CSS fetch failed: {e}")
        return None, None

    font_dir = content_dir / "fonts"
    font_dir.mkdir(exist_ok=True)

    face_blocks = []
    for block in re.findall(r"@font-face\s*\{[^}]+\}", css):
        url_m = re.search(r"url\(([^)]+\.woff2[^)]*)\)", block)
        if not url_m:
            continue
        font_url = url_m.group(1).strip("'\"")
        fname = re.sub(r"\?.*$", "", font_url.split("/")[-1])
        local_path = font_dir / fname
        if not local_path.exists():
            try:
                local_path.write_bytes(_fetch(font_url))
            except Exception as e:
                print(f"  ! WARN: could not download {fname}: {e}")
                continue
        block_local = re.sub(
            r"url\([^)]+\.woff2[^)]*\)",
            f"url('/fonts/{fname}')",
            block,
        )
        face_blocks.append(block_local)

    print(f"  Downloaded: {len(face_blocks)} @font-face blocks → {font_dir.relative_to(content_dir.parent.parent.parent.parent.parent)}")
    face_css = "\n".join(face_blocks)
    return font_dir, face_css


def u1_self_host_fonts(content_dir: Path, label: str):
    print(f"\n── U1 Google Fonts → self-hosted ({label}) ──────────────────────")
    font_dir, face_css = download_fonts_for_dir(content_dir)

    if font_dir is None:
        # No CSS link found — just clean up stale preconnect hints
        for f in sorted(content_dir.glob("*.html")):
            src = f.read_text(encoding="utf-8")
            if "fonts.googleapis.com" not in src and "fonts.gstatic.com" not in src:
                continue
            if "Self-hosted fonts" in src:
                print(f"  {f.name}: already self-hosted (skip)")
                continue
            cleaned = re.sub(
                r"\s*<link[^>]*(fonts\.googleapis\.com|fonts\.gstatic\.com)[^>]*>",
                "",
                src,
            )
            if cleaned != src:
                f.write_text(cleaned, encoding="utf-8")
                print(f"  ✓  {f.name}: stale preconnect hints removed")
            else:
                print(f"  {f.name}: no googleapis refs (skip)")
        return

    style_block = (
        "<style>\n"
        "/* Self-hosted fonts — replaces Google Fonts (GDPR) */\n"
        f"{face_css}\n"
        "</style>"
    )

    for f in sorted(content_dir.glob("*.html")):
        src = f.read_text(encoding="utf-8")
        if "fonts.googleapis.com" not in src and "fonts.gstatic.com" not in src:
            print(f"  {f.name}: no googleapis refs (skip)")
            continue
        if "Self-hosted fonts" in src:
            print(f"  {f.name}: already self-hosted (skip)")
            continue
        cleaned = re.sub(
            r"\s*<link[^>]*(fonts\.googleapis\.com|fonts\.gstatic\.com)[^>]*>",
            "",
            src,
        )
        if "</head>" in cleaned:
            cleaned = cleaned.replace("</head>", style_block + "\n</head>", 1)
        if cleaned != src:
            f.write_text(cleaned, encoding="utf-8")
            print(f"  ✓  {f.name}: Google Fonts → self-hosted")
        else:
            print(f"  !  WARN {f.name}: no change made")

    # Also remove preconnect hints from index.html template JSON
    for path, site_label in [(LAND1 / "index.html", "woodfine"), (LAND2 / "index.html", "pointsav")]:
        if path.parent != content_dir:
            continue
        try:
            src, t, si, ep = load_template(path)
        except Exception:
            continue
        old_pc = '<link rel="preconnect" href="https://fonts.googleapis.com">'
        if old_pc in t:
            cleaned_t = t.replace(old_pc, "")
            # Also remove gstatic preconnect
            cleaned_t = re.sub(
                r'<link rel="preconnect" href="https://fonts\.gstatic\.com"[^>]*>',
                "",
                cleaned_t,
            )
            save_template(path, src, cleaned_t, si, ep)
            print(f"  ✓  index.html template: preconnect hints removed")
        else:
            print(f"  index.html template: no preconnect hints (skip)")


# ─── Template patches ─────────────────────────────────────────────────────────

def patch_template_strings(path: Path, patches, section_label: str):
    """patches = [(old, new, name), ...]  — applied to template JSON string."""
    try:
        src, t, si, ep = load_template(path)
    except Exception as e:
        print(f"  ! ERROR loading template {path.name}: {e}")
        return

    changed = False
    for old, new, name in patches:
        if old in t:
            t = t.replace(old, new, 1)
            changed = True
            print(f"  ✓  {name}")
        elif new in t:
            print(f"     {name} (skip — already applied)")
        else:
            print(f"  !  WARN: {name} — string not found")

    if changed:
        save_template(path, src, t, si, ep)


# ─── Outer / static HTML patches ──────────────────────────────────────────────

def patch_file(path: Path, patches, section_label: str = ""):
    """patches = [(old, new, name), ...]  — applied to raw file."""
    src = path.read_text(encoding="utf-8")
    changed = False
    for old, new, name in patches:
        if old in src:
            src = src.replace(old, new, 1)
            changed = True
            print(f"  ✓  {path.name}: {name}")
        elif new in src:
            print(f"     {path.name}: {name} (skip — already applied)")
        else:
            print(f"  !  WARN {path.name}: {name} — not found")
    if changed:
        path.write_text(src, encoding="utf-8")


def patch_file_regex(path: Path, pattern, repl, name: str):
    src = path.read_text(encoding="utf-8")
    new_src, n = re.subn(pattern, repl, src)
    if n > 0:
        path.write_text(new_src, encoding="utf-8")
        print(f"  ✓  {path.name}: {name} ({n} occurrence{'s' if n > 1 else ''})")
    elif re.search(repl if isinstance(repl, str) and not re.search(r'\\[0-9]', repl) else pattern, src):
        print(f"     {path.name}: {name} (skip — already applied)")
    else:
        print(f"  !  WARN {path.name}: {name} — not found")


# ─── MAIN ─────────────────────────────────────────────────────────────────────

def main():
    # ── U1: Google Fonts ──────────────────────────────────────────────────────
    u1_self_host_fonts(LAND1, "woodfine")
    u1_self_host_fonts(LAND2, "pointsav")

    # ── U2: PointSav P0 typos ─────────────────────────────────────────────────
    print("\n── U2 PointSav P0 typos ─────────────────────────────────────────────")
    patch_template_strings(
        LAND2 / "index.html",
        [
            ("F*KEYS CONSSOLE", "F-KEYS CONSOLE", "F*KEYS CONSSOLE → F-KEYS CONSOLE"),
            ("DIGTIAL TWIN",    "DIGITAL TWIN",   "DIGTIAL TWIN → DIGITAL TWIN"),
        ],
        "U2",
    )

    # ── U3: Woodfine P0 hero typo ─────────────────────────────────────────────
    print("\n── U3 Woodfine hero typo ────────────────────────────────────────────")
    patch_template_strings(
        LAND1 / "index.html",
        [
            (
                "is an real property",
                "is a real property",
                '"is an real property" → "is a real property"',
            ),
        ],
        "U3",
    )

    # ── U4: PointSav h1 ───────────────────────────────────────────────────────
    # h1 lives in outer HTML, not the template JSON
    print("\n── U4 PointSav h1 ───────────────────────────────────────────────────")
    raw = (LAND2 / "index.html").read_text(encoding="utf-8")
    start_tag = '<script type="__bundler/template">'
    outer = raw[:raw.index(start_tag)]
    if re.search(r"<h1[^>]*>", outer):
        print("  ✓  <h1> present in outer HTML (skip)")
    else:
        print("  !  WARN: no <h1> found in PointSav outer HTML — manual action needed")

    # ── U5: Nav font minimum 14px ─────────────────────────────────────────────
    print("\n── U5 Nav font minimum 14px ─────────────────────────────────────────")
    # Woodfine uses .subnav a.manifest-btn; PointSav uses a.monorepo-btn
    woodfine_nav_patches = [
        (
            ".topnav .right { justify-content: center; gap: 20px; font-size: 10px; }",
            ".topnav .right { justify-content: center; gap: 20px; font-size: 14px; }",
            "768px .topnav .right: 10px → 14px",
        ),
        (
            ".topnav .left, .topnav .right { gap: 14px; font-size: 11px; }",
            ".topnav .left, .topnav .right { gap: 14px; font-size: 14px; }",
            "480px .topnav: 11px → 14px",
        ),
        (
            ".subnav .tab, .subnav a.manifest-btn { width: 100%; min-width: unset; font-size: 11px; box-sizing: border-box; }",
            ".subnav .tab, .subnav a.manifest-btn { width: 100%; min-width: unset; font-size: 14px; box-sizing: border-box; }",
            "768px subnav .tab: 11px → 14px",
        ),
        (
            ".subnav .tab, .subnav a.manifest-btn { font-size: 11px; padding: 8px 14px; }",
            ".subnav .tab, .subnav a.manifest-btn { font-size: 14px; padding: 8px 14px; }",
            "480px subnav .tab: 11px → 14px",
        ),
    ]
    pointsav_nav_patches = [
        (
            ".topnav .right { justify-content: center; gap: 20px; font-size: 10px; }",
            ".topnav .right { justify-content: center; gap: 20px; font-size: 14px; }",
            "768px .topnav .right: 10px → 14px",
        ),
        (
            ".topnav .left, .topnav .right { gap: 14px; font-size: 11px; }",
            ".topnav .left, .topnav .right { gap: 14px; font-size: 14px; }",
            "480px .topnav: 11px → 14px",
        ),
        (
            ".subnav .tab, a.monorepo-btn { width: 100%; min-width: unset; font-size: 11px; box-sizing: border-box; }",
            ".subnav .tab, a.monorepo-btn { width: 100%; min-width: unset; font-size: 14px; box-sizing: border-box; }",
            "768px subnav .tab: 11px → 14px",
        ),
        (
            ".subnav .tab, a.monorepo-btn { font-size: 11px; padding: 8px 14px; }",
            ".subnav .tab, a.monorepo-btn { font-size: 14px; padding: 8px 14px; }",
            "480px subnav .tab: 11px → 14px",
        ),
    ]
    print("  [woodfine]")
    patch_template_strings(LAND1 / "index.html", woodfine_nav_patches, "U5-woodfine")
    print("  [pointsav]")
    patch_template_strings(LAND2 / "index.html", pointsav_nav_patches, "U5-pointsav")

    # ── U6: Remove internal repo path from footers ────────────────────────────
    print("\n── U6 Footer: remove factory-release-engineering path ───────────────")
    factory_pattern = re.compile(
        r"\s*\n?\s*Source: factory-release-engineering/policies/DISCLAIMER\.md\n?"
    )
    factory_repl = ""
    factory_name = "remove 'Source: factory-release-engineering/…'"

    for path in [
        LAND1 / "contact.html",
        LAND1 / "disclaimer.html",
        LAND2 / "contact.html",
        LAND2 / "disclaimer.html",
    ]:
        patch_file_regex(path, factory_pattern, factory_repl, factory_name)

    # PointSav index template (contains the line in footer HTML)
    print("  [pointsav index.html template]")
    try:
        src, t, si, ep = load_template(LAND2 / "index.html")
        if "factory-release-engineering" in t:
            t = re.sub(
                r"\s*\n?\s*Source: factory-release-engineering/policies/DISCLAIMER\.md\n?",
                "",
                t,
            )
            save_template(LAND2 / "index.html", src, t, si, ep)
            print("  ✓  index.html: factory path removed from template")
        else:
            print("     index.html: factory path not in template (skip)")
    except Exception as e:
        print(f"  ! ERROR: {e}")

    # ── U7: PointSav navy #164679 as dominant color ───────────────────────────
    print("\n── U7 PointSav navy #164679 dominant ────────────────────────────────")
    try:
        src, t, si, ep = load_template(LAND2 / "index.html")
        changed = False
        # Replace #1d5594 (off-spec lighter navy) with #164679 in background rules
        if "background: #1d5594" in t:
            t = t.replace("background: #1d5594", "background: #164679")
            changed = True
            print("  ✓  background: #1d5594 → #164679")
        else:
            print(f"     background #1d5594 not found")
        # Check for steel-gray backgrounds (not desired)
        steel_bgs = re.findall(r"background(?:-color)?:\s*#(?:4a4a4a|555555|6b7280|9ca3af)[^;]*;", t)
        if steel_bgs:
            for bg in steel_bgs[:3]:
                print(f"  !  WARN: steel-gray background found: {bg[:60]}")
        else:
            navy_count = t.count("#164679")
            print(f"     #164679 appears {navy_count}× — navy dominant (skip)")
        if changed:
            save_template(LAND2 / "index.html", src, t, si, ep)
    except Exception as e:
        print(f"  ! ERROR: {e}")

    # ── U8: Unpacking splash — patch outer HTML (not template JSON) ──────────
    print("\n── U8 Unpacking splash ──────────────────────────────────────────────")
    for path, label in [(LAND1 / "index.html", "woodfine"), (LAND2 / "index.html", "pointsav")]:
        src = path.read_text(encoding="utf-8")
        changed = False
        old_div = '<div id="__bundler_loading">Unpacking...</div>'
        new_div = '<div id="__bundler_loading"></div>'
        old_status = "setStatus('Unpacking ' + uuids.length + ' assets...');"
        new_status = "setStatus('');"
        if old_div in src:
            src = src.replace(old_div, new_div, 1)
            changed = True
            print(f"  ✓  {label}: loading div text cleared")
        elif new_div in src:
            print(f"     {label}: loading div already cleared (skip)")
        else:
            print(f"  !  WARN {label}: loading div not found")
        if old_status in src:
            src = src.replace(old_status, new_status, 1)
            changed = True
            print(f"  ✓  {label}: setStatus('Unpacking…') silenced")
        elif new_status in src:
            print(f"     {label}: setStatus already silenced (skip)")
        else:
            print(f"  !  WARN {label}: setStatus call not found")
        if changed:
            path.write_text(src, encoding="utf-8")

    # ── U9: Nav consolidation ─────────────────────────────────────────────────
    # Idempotency: check for the DESIRED link before inserting; don't match
    # by old-string (the new content always contains the contact/disclaimer href).
    print("\n── U9 Nav consolidation across pages ────────────────────────────────")

    def nav_ensure_link(path: Path, insert_before_href, new_link_href, new_link_text):
        """Idempotently insert new_link into <nav class="left"> before insert_before_href.
        If insert_before_href is None, append before </nav>."""
        src = path.read_text(encoding="utf-8")
        nav_m = re.search(r'<nav class="left"[^>]*>(.*?)</nav>', src, re.DOTALL)
        if not nav_m:
            print(f"  !  WARN {path.name}: nav not found")
            return
        nav_body = nav_m.group(1)
        if f'href="{new_link_href}"' in nav_body:
            print(f"     {path.name}: {new_link_href} already in nav (skip)")
            return
        # Detect existing indentation from any link in the nav
        indent_m = re.search(r'^(\s+)<a ', nav_body, re.MULTILINE)
        indent = indent_m.group(1) if indent_m else "      "
        new_link_tag = f'{indent}<a href="{new_link_href}">{new_link_text}</a>\n'
        if insert_before_href:
            # Find the line containing insert_before_href
            before_m = re.search(
                r'(' + re.escape(indent) + r'<a [^>]*href="' + re.escape(insert_before_href) + r'")',
                nav_body,
            )
            if not before_m:
                print(f"  !  WARN {path.name}: insert_before {insert_before_href} not in nav")
                return
            new_nav_body = nav_body[:before_m.start()] + new_link_tag + nav_body[before_m.start():]
        else:
            # Append before closing </nav>
            closing_m = re.search(r'\s*</nav>', src[nav_m.start():])
            if not closing_m:
                print(f"  !  WARN {path.name}: </nav> not found")
                return
            insert_pos = nav_m.end(1)
            src = src[:insert_pos] + "\n" + new_link_tag + src[insert_pos:]
            path.write_text(src, encoding="utf-8")
            print(f"  ✓  {path.name}: {new_link_href} appended to nav")
            return
        new_src = src[:nav_m.start(1)] + new_nav_body + src[nav_m.end(1):]
        path.write_text(new_src, encoding="utf-8")
        print(f"  ✓  {path.name}: {new_link_href} added to nav")

    # Woodfine: bim-library + location-intelligence missing Disclaimer
    for fname in ("bim-library.html", "location-intelligence.html"):
        nav_ensure_link(LAND1 / fname, "/page/contact", "/page/disclaimer", "Disclaimer")

    # PointSav contact.html: missing Contact us in nav
    nav_ensure_link(LAND2 / "contact.html", None, "/page/contact", "Contact us")

    # PointSav disclaimer.html: missing Disclaimer in nav
    nav_ensure_link(LAND2 / "disclaimer.html", "/page/contact", "/page/disclaimer", "Disclaimer")

    # ── U10: Fix dead href="#" in contact page footers ────────────────────────
    print("\n── U10 Fix href=\"#\" in contact page footers ────────────────────────")
    for path in [LAND1 / "contact.html", LAND2 / "contact.html"]:
        patch_file(
            path,
            [
                (
                    '<a href="#">Contact us</a>',
                    '<a href="/page/contact">Contact us</a>',
                    'footer href="#" → href="/page/contact"',
                )
            ],
        )

    print("\n═══════════════════════════════════════════════════════════════════")
    print("Done. Run: bash scripts/verify-live.sh")
    print("═══════════════════════════════════════════════════════════════════")


if __name__ == "__main__":
    main()
