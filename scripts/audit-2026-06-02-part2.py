"""
Part 2: collect tab order, touch targets, contrast, tokens, meta.
Fixes the arrow-function/arguments issue from part 1.
"""
import json
import os
from pathlib import Path
from playwright.sync_api import sync_playwright

OUT = Path("outputs/audit-2026-06-02")
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

def run_extra(page, tenant, page_name, base_url):
    url = base_url + PAGES[page_name]
    print(f"  [{tenant}/{page_name}]")
    page.goto(url, wait_until="networkidle", timeout=30000)
    page.set_viewport_size({"width": 1440, "height": 900})
    page.wait_for_timeout(300)

    # Tab order — pass index as a captured variable, not arguments
    tab_map = []
    page.focus("body")
    for i in range(35):
        page.keyboard.press("Tab")
        focused = page.evaluate(f"""
            () => {{
                const el = document.activeElement;
                const rect = el.getBoundingClientRect();
                const style = window.getComputedStyle(el);
                return {{
                    index: {i},
                    tag: el.tagName,
                    type: el.getAttribute('type'),
                    role: el.getAttribute('role'),
                    aria_label: el.getAttribute('aria-label'),
                    text: (el.textContent || '').trim().slice(0, 60),
                    href: el.getAttribute('href'),
                    w: Math.round(rect.width),
                    h: Math.round(rect.height),
                    outline: style.outline,
                    outline_width: style.outlineWidth,
                    outline_color: style.outlineColor,
                    visible_box: rect.width > 0 && rect.height > 0,
                }};
            }}
        """)
        tab_map.append(focused)
    (OUT / f"{tenant}-{page_name}-taborder.json").write_text(json.dumps(tab_map, indent=2))
    print(f"    tab stops: {len(tab_map)}")

    # Touch targets
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
    (OUT / f"{tenant}-{page_name}-targets.json").write_text(json.dumps(targets, indent=2))
    failing = [t for t in targets if t["visible"] and not t["pass_255"]]
    print(f"    targets: {len(targets)} total, {len(failing)} failing WCAG 2.5.5 (44px)")

    # Contrast pairs
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
                    const m = (s||'').match(/rgba?\\((\\d+),\\s*(\\d+),\\s*(\\d+)/);
                    return m ? [+m[1], +m[2], +m[3]] : null;
                }
                const fgRgb = parseRgb(fg), bgRgb = parseRgb(bg);
                if (!fgRgb || !bgRgb) return null;
                const l1 = getLum(...fgRgb), l2 = getLum(...bgRgb);
                const lighter = Math.max(l1, l2), darker = Math.min(l1, l2);
                return Math.round(((lighter + 0.05) / (darker + 0.05)) * 100) / 100;
            }
            const selectors = [
                ['nav a', 'header'],
                ['h1', 'body'],
                ['h2', 'body'],
                ['p', 'body'],
                ['footer a', 'footer'],
                ['a', 'body'],
            ];
            return selectors.map(function(pair) {
                var elSel = pair[0], bgSel = pair[1];
                var el = document.querySelector(elSel);
                var bgEl = document.querySelector(bgSel) || document.body;
                if (!el) return { selector: elSel, error: 'element not found' };
                var elStyle = window.getComputedStyle(el);
                var bgStyle = window.getComputedStyle(bgEl);
                var ratio = contrastRatio(elStyle.color, bgStyle.backgroundColor);
                return {
                    selector: elSel,
                    bg_selector: bgSel,
                    fg_color: elStyle.color,
                    bg_color: bgStyle.backgroundColor,
                    ratio: ratio,
                    pass_aa_normal: ratio !== null && ratio >= 4.5,
                    pass_aaa: ratio !== null && ratio >= 7.0,
                    font_size: elStyle.fontSize,
                    font_weight: elStyle.fontWeight,
                };
            });
        }
    """)
    (OUT / f"{tenant}-{page_name}-contrast.json").write_text(json.dumps(contrast_pairs, indent=2))
    print(f"    contrast: {len(contrast_pairs)} pairs")

    # CSS tokens
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
                } catch(e) {}
            }
            return Object.fromEntries(props);
        }
    """)
    (OUT / f"{tenant}-{page_name}-tokens.json").write_text(json.dumps(tokens, indent=2, ensure_ascii=False))
    print(f"    tokens: {len(tokens)} CSS custom properties")

    # Meta / accessibility inventory
    meta = page.evaluate("""
        () => {
            var vp = document.querySelector('meta[name=viewport]');
            var vpContent = vp ? vp.getAttribute('content') : null;
            var allEls = document.querySelectorAll('*');
            var ariaCount = 0;
            for (var i = 0; i < allEls.length; i++) {
                for (var j = 0; j < allEls[i].attributes.length; j++) {
                    if (allEls[i].attributes[j].name.startsWith('aria-')) {
                        ariaCount++;
                    }
                }
            }
            var svgs = document.querySelectorAll('svg');
            var svgNoTitle = 0;
            for (var k = 0; k < svgs.length; k++) {
                if (!svgs[k].querySelector('title')) svgNoTitle++;
            }
            var prefers_motion = window.matchMedia('(prefers-reduced-motion: reduce)');
            return {
                title: document.title,
                lang: document.documentElement.lang,
                viewport_content: vpContent,
                has_user_scalable_no: vpContent ? vpContent.includes('user-scalable=no') : false,
                has_max_scale_1: vpContent ? vpContent.includes('maximum-scale=1') : false,
                aria_attr_count: ariaCount,
                role_attr_count: document.querySelectorAll('[role]').length,
                img_count: document.querySelectorAll('img').length,
                img_no_alt: document.querySelectorAll('img:not([alt])').length,
                svg_count: svgs.length,
                svg_no_title: svgNoTitle,
                landmark_count: document.querySelectorAll('header, nav, main, footer, aside, [role="main"], [role="navigation"], [role="banner"], [role="contentinfo"]').length,
                has_skip_link: !!document.querySelector('a[href="#main"], a[href="#content"], .skip-link, [class*="skip"]'),
                focusable_count: document.querySelectorAll('a[href], button, input, select, textarea, [tabindex]').length,
                h_count: {
                    h1: document.querySelectorAll('h1').length,
                    h2: document.querySelectorAll('h2').length,
                    h3: document.querySelectorAll('h3').length,
                },
                has_prefers_motion_css: document.querySelector('style') &&
                    [...document.querySelectorAll('style')].some(s => s.textContent.includes('prefers-reduced-motion')),
                has_dark_mode_css: document.querySelector('style') &&
                    [...document.querySelectorAll('style')].some(s => s.textContent.includes('prefers-color-scheme')),
                has_font_display_swap: document.querySelector('style') &&
                    [...document.querySelectorAll('style')].some(s => s.textContent.includes('font-display: swap') || s.textContent.includes('font-display:swap')),
                link_count: document.querySelectorAll('a[href]').length,
                external_links: [...document.querySelectorAll('a[target="_blank"]')].map(a => ({
                    text: a.textContent.trim().slice(0,30),
                    rel: a.getAttribute('rel'),
                    has_rel_noopener: (a.getAttribute('rel')||'').includes('noopener'),
                    aria_label: a.getAttribute('aria-label'),
                })),
            };
        }
    """)
    (OUT / f"{tenant}-{page_name}-meta.json").write_text(json.dumps(meta, indent=2))
    print(f"    meta: aria={meta['aria_attr_count']}, landmarks={meta['landmark_count']}, "
          f"svg={meta['svg_count']}({meta['svg_no_title']} no-title), "
          f"skip={meta['has_skip_link']}, prefers-motion-css={meta['has_prefers_motion_css']}, "
          f"dark-mode-css={meta['has_dark_mode_css']}, font-swap={meta['has_font_display_swap']}")

    return {
        "tab_stops": len(tab_map),
        "targets_total": len(targets),
        "targets_failing_255": len(failing),
        "meta": meta,
    }


def main():
    os.chdir("/srv/foundry/clones/project-marketing")
    summary = {}
    with sync_playwright() as pw:
        browser = pw.chromium.launch(headless=True)
        ctx = browser.new_context(ignore_https_errors=True)
        page = ctx.new_page()
        for tenant, base in SITES.items():
            print(f"\n--- {tenant} ---")
            summary[tenant] = {}
            for pname in PAGES:
                try:
                    summary[tenant][pname] = run_extra(page, tenant, pname, base)
                except Exception as e:
                    print(f"  ERROR: {e}")
                    summary[tenant][pname] = {"error": str(e)}
        browser.close()

    (OUT / "summary-part2.json").write_text(json.dumps(summary, indent=2))
    print(f"\n=== Part 2 done. Files: {len(list(OUT.iterdir()))} in {OUT}/ ===")


if __name__ == "__main__":
    main()
