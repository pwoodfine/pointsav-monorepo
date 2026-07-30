"""
Browser-in-the-loop audit — home.woodfinegroup.com + home.pointsav.com
Leapfrog 2030 design + accessibility audit data collection.

Outputs to: outputs/audit-2026-06-02/
Run from: /srv/foundry/clones/project-marketing/
"""

import json
import math
import os
import sys
from pathlib import Path
from playwright.sync_api import sync_playwright

OUT = Path("outputs/audit-2026-06-02")
OUT.mkdir(parents=True, exist_ok=True)
AXE_PATH = str(Path("node_modules/axe-core/axe.min.js").resolve())

SITES = {
    "woodfine": "http://127.0.0.1:9102",
    "pointsav": "http://127.0.0.1:9101",
}

PAGES = {
    "index":      "/",
    "contact":    "/contact.html",
    "disclaimer": "/disclaimer.html",
}

VIEWPORTS = {
    "375": {"width": 375,  "height": 812},
    "768": {"width": 768,  "height": 1024},
    "1024": {"width": 1024, "height": 768},
    "1440": {"width": 1440, "height": 900},
}

def wcag_contrast(fg_rgb, bg_rgb):
    """Relative luminance + contrast ratio per WCAG 2.x."""
    def lum(r, g, b):
        def c(v):
            v /= 255
            return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
        return 0.2126 * c(r) + 0.7152 * c(g) + 0.0722 * c(b)
    l1 = lum(*fg_rgb)
    l2 = lum(*bg_rgb)
    lighter, darker = max(l1, l2), min(l1, l2)
    return (lighter + 0.05) / (darker + 0.05)

def parse_rgb(css_color):
    """Parse rgb(r,g,b) or rgba(r,g,b,a) to (r,g,b) tuple."""
    import re
    m = re.match(r'rgba?\((\d+),\s*(\d+),\s*(\d+)', css_color or "")
    if m:
        return (int(m.group(1)), int(m.group(2)), int(m.group(3)))
    return None

def run_page_audit(page, tenant, page_name, base_url):
    url = base_url + PAGES[page_name]
    print(f"  [{tenant}/{page_name}] navigating to {url}")
    page.goto(url, wait_until="networkidle", timeout=30000)

    # Screenshots at all viewports
    for vp_name, vp in VIEWPORTS.items():
        page.set_viewport_size(vp)
        page.wait_for_timeout(300)
        fname = OUT / f"{tenant}-{page_name}-{vp_name}.png"
        page.screenshot(path=str(fname), full_page=True)
        print(f"    screenshot {vp_name}px → {fname.name}")

    # Reset to desktop for JS-based audits
    page.set_viewport_size(VIEWPORTS["1440"])
    page.wait_for_timeout(200)

    # Axe-core accessibility scan
    page.add_script_tag(path=AXE_PATH)
    page.wait_for_timeout(500)
    axe_raw = page.evaluate("() => axe.run()")
    axe_out = {
        "violations": axe_raw.get("violations", []),
        "incomplete": axe_raw.get("incomplete", []),
        "passes_count": len(axe_raw.get("passes", [])),
        "inapplicable_count": len(axe_raw.get("inapplicable", [])),
        "violation_summary": [
            {
                "id": v["id"],
                "impact": v["impact"],
                "description": v["description"],
                "help": v["help"],
                "helpUrl": v["helpUrl"],
                "nodes_count": len(v["nodes"]),
                "nodes_snippet": [n["html"][:200] for n in v["nodes"][:3]],
            }
            for v in axe_raw.get("violations", [])
        ],
    }
    axe_file = OUT / f"{tenant}-{page_name}-axe.json"
    axe_file.write_text(json.dumps(axe_out, indent=2))
    vcount = len(axe_out["violations"])
    print(f"    axe: {vcount} violations, {axe_out['passes_count']} passes")

    # Tab order map (30 tabs from body start)
    page.keyboard.press("Tab")  # focus first element
    tab_map = []
    for i in range(30):
        focused = page.evaluate("""
            () => {
                const el = document.activeElement;
                const rect = el.getBoundingClientRect();
                const style = window.getComputedStyle(el);
                return {
                    index: arguments[0],
                    tag: el.tagName,
                    type: el.getAttribute('type'),
                    role: el.getAttribute('role'),
                    aria_label: el.getAttribute('aria-label'),
                    text: (el.textContent || '').trim().slice(0, 60),
                    href: el.getAttribute('href'),
                    w: Math.round(rect.width),
                    h: Math.round(rect.height),
                    outline: style.outline,
                    outline_color: style.outlineColor,
                    outline_width: style.outlineWidth,
                    visible: rect.width > 0 && rect.height > 0,
                };
            }
        """, i)
        tab_map.append(focused)
        page.keyboard.press("Tab")
    tab_file = OUT / f"{tenant}-{page_name}-taborder.json"
    tab_file.write_text(json.dumps(tab_map, indent=2))
    print(f"    tab order: {len(tab_map)} stops mapped")

    # Touch target sizes — all <a> and <button>
    targets = page.evaluate("""
        () => [...document.querySelectorAll('a, button')].map(el => {
            const r = el.getBoundingClientRect();
            return {
                tag: el.tagName,
                text: (el.textContent || '').trim().slice(0, 40),
                href: el.getAttribute('href'),
                w: Math.round(r.width),
                h: Math.round(r.height),
                pass_255: r.width >= 44 && r.height >= 44,
                pass_258: r.width >= 24 && r.height >= 24,
                visible: r.width > 0 && r.height > 0,
            };
        })
    """)
    targets_file = OUT / f"{tenant}-{page_name}-targets.json"
    targets_file.write_text(json.dumps(targets, indent=2))
    failing_255 = [t for t in targets if t["visible"] and not t["pass_255"]]
    print(f"    touch targets: {len(targets)} total, {len(failing_255)} failing WCAG 2.5.5")

    # Color contrast — sample key element pairs
    contrast_pairs = page.evaluate("""
        () => {
            function getLum(r, g, b) {
                function c(v) {
                    v /= 255;
                    return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
                }
                return 0.2126 * c(r) + 0.7152 * c(g) + 0.0722 * c(b);
            }
            function contrastRatio(fg, bg) {
                function parseRgb(s) {
                    const m = s.match(/rgba?\\((\\d+),\\s*(\\d+),\\s*(\\d+)/);
                    return m ? [+m[1], +m[2], +m[3]] : null;
                }
                const fgRgb = parseRgb(fg), bgRgb = parseRgb(bg);
                if (!fgRgb || !bgRgb) return null;
                const l1 = getLum(...fgRgb), l2 = getLum(...bgRgb);
                const lighter = Math.max(l1, l2), darker = Math.min(l1, l2);
                return Math.round(((lighter + 0.05) / (darker + 0.05)) * 100) / 100;
            }
            const selectors = [
                ['nav a', 'nav'],
                ['h1', 'body'],
                ['h2', 'body'],
                ['p', 'body'],
                ['.footnav a', 'footer'],
                ['.copyright', 'footer'],
                ['.page-hero h1', '.page-hero'],
                ['a', 'body'],
            ];
            return selectors.map(([elSel, bgSel]) => {
                const el = document.querySelector(elSel);
                const bg = document.querySelector(bgSel);
                if (!el) return { selector: elSel, error: 'element not found' };
                const elStyle = window.getComputedStyle(el);
                const bgEl = bg || document.body;
                const bgStyle = window.getComputedStyle(bgEl);
                const ratio = contrastRatio(elStyle.color, bgStyle.backgroundColor);
                return {
                    selector: elSel,
                    bg_selector: bgSel,
                    fg_color: elStyle.color,
                    bg_color: bgStyle.backgroundColor,
                    ratio: ratio,
                    pass_aa_normal: ratio >= 4.5,
                    pass_aaa_normal: ratio >= 7.0,
                    pass_aa_large: ratio >= 3.0,
                    font_size: elStyle.fontSize,
                    font_weight: elStyle.fontWeight,
                };
            });
        }
    """)
    contrast_file = OUT / f"{tenant}-{page_name}-contrast.json"
    contrast_file.write_text(json.dumps(contrast_pairs, indent=2))
    print(f"    contrast: {len(contrast_pairs)} pairs sampled")

    # CSS custom properties from :root
    tokens = page.evaluate("""
        () => {
            const s = getComputedStyle(document.documentElement);
            const props = [];
            for (const sheet of document.styleSheets) {
                try {
                    for (const rule of sheet.cssRules) {
                        if (rule.selectorText === ':root') {
                            for (const prop of rule.style) {
                                if (prop.startsWith('--')) {
                                    props.push([prop, s.getPropertyValue(prop).trim()]);
                                }
                            }
                        }
                    }
                } catch (e) {}
            }
            return Object.fromEntries(props);
        }
    """)
    tokens_file = OUT / f"{tenant}-{page_name}-tokens.json"
    tokens_file.write_text(json.dumps(tokens, indent=2, ensure_ascii=False))
    print(f"    CSS tokens: {len(tokens)} custom properties")

    # Viewport meta + lang check
    meta = page.evaluate("""
        () => {
            const vp = document.querySelector('meta[name=viewport]');
            const lang = document.documentElement.lang;
            const title = document.title;
            const desc = (document.querySelector('meta[name=description]') || {}).content;
            const ariaCount = document.querySelectorAll('[aria-*]').length;
            const roleCount = document.querySelectorAll('[role]').length;
            const imgCount = document.querySelectorAll('img').length;
            const imgNoAlt = document.querySelectorAll('img:not([alt])').length;
            const svgCount = document.querySelectorAll('svg').length;
            const svgNoTitle = [...document.querySelectorAll('svg')].filter(
                s => !s.querySelector('title')
            ).length;
            const landmarkCount = document.querySelectorAll(
                'header, nav, main, footer, aside, section[aria-label], [role=main], [role=navigation], [role=banner], [role=contentinfo]'
            ).length;
            const hasSkipLink = !!document.querySelector('a[href="#main"], a[href="#content"], .skip-link');
            const focusableCount = document.querySelectorAll(
                'a[href], button, input, select, textarea, [tabindex]'
            ).length;
            return {
                title, lang, desc,
                viewport_content: vp ? vp.getAttribute('content') : null,
                has_user_scalable_no: vp ? (vp.getAttribute('content') || '').includes('user-scalable=no') : false,
                has_max_scale_1: vp ? (vp.getAttribute('content') || '').includes('maximum-scale=1') : false,
                aria_attr_count: ariaCount,
                role_attr_count: roleCount,
                img_count: imgCount,
                img_no_alt: imgNoAlt,
                svg_count: svgCount,
                svg_no_title: svgNoTitle,
                landmark_count: landmarkCount,
                has_skip_link: hasSkipLink,
                focusable_count: focusableCount,
            };
        }
    """)
    meta_file = OUT / f"{tenant}-{page_name}-meta.json"
    meta_file.write_text(json.dumps(meta, indent=2))
    print(f"    meta: aria={meta['aria_attr_count']}, landmarks={meta['landmark_count']}, svg={meta['svg_count']} ({meta['svg_no_title']} no-title), skip-link={meta['has_skip_link']}")

    return {
        "axe_violations": axe_out["violation_summary"],
        "axe_passes_count": axe_out["passes_count"],
        "tab_stops": len(tab_map),
        "targets_failing_255": len(failing_255),
        "meta": meta,
    }


def main():
    print("=== Leapfrog 2030 Browser Audit ===")
    summary = {}

    with sync_playwright() as pw:
        browser = pw.chromium.launch(headless=True)
        context = browser.new_context(
            user_agent="Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ignore_https_errors=True,
        )
        page = context.new_page()

        for tenant, base_url in SITES.items():
            print(f"\n--- {tenant} ({base_url}) ---")
            summary[tenant] = {}
            for page_name in PAGES:
                try:
                    result = run_page_audit(page, tenant, page_name, base_url)
                    summary[tenant][page_name] = result
                except Exception as e:
                    print(f"  ERROR [{tenant}/{page_name}]: {e}")
                    summary[tenant][page_name] = {"error": str(e)}

        browser.close()

    summary_file = OUT / "summary.json"
    summary_file.write_text(json.dumps(summary, indent=2))
    print(f"\n=== Done. Output in {OUT}/ ===")
    print(f"Files: {len(list(OUT.iterdir()))}")

    # Print quick summary
    print("\n=== Quick summary ===")
    for tenant, pages in summary.items():
        print(f"\n{tenant.upper()}:")
        for pname, data in pages.items():
            if "error" in data:
                print(f"  {pname}: ERROR — {data['error']}")
            else:
                vcount = len(data.get("axe_violations", []))
                tcount = data.get("targets_failing_255", 0)
                meta = data.get("meta", {})
                print(f"  {pname}: {vcount} axe violations | {tcount} small targets | "
                      f"aria={meta.get('aria_attr_count','?')} | "
                      f"skip-link={meta.get('has_skip_link','?')} | "
                      f"svg-no-title={meta.get('svg_no_title','?')}")


if __name__ == "__main__":
    os.chdir("/srv/foundry/clones/project-marketing")
    main()
