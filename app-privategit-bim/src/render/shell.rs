// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use crate::state::AppState;

pub fn page_shell(title: &str, active_path: &str, content: &str, state: &AppState) -> String {
    let tc = state.categories.len();
    let full_title = if title.is_empty() {
        "BIM Object Library — Woodfine".to_string()
    } else {
        format!("{} — BIM Object Library", esc(title))
    };

    // /edit/* embeds real Carbon Web Components (<cds-content-switcher> etc.)
    // that are only styled for a light Carbon theme — force light there
    // server-side rather than trying to make Carbon's chrome theme-reactive.
    let editor_route = active_path.starts_with("/edit/");
    // Full disclosure copy inline, not a truncated summary + "read more" link
    // — matches the pattern already proven correct on home.woodfinegroup.com
    // and home.pointsav.com (both inline their complete disclosure text in
    // the footer <details>, with a "Full disclaimer" pointer only at the
    // very end for anyone who wants the standalone page). The prior
    // 2-paragraph important-information.md summary + "Read the full
    // disclaimer" link-out truncated real disclosure content — fixed here
    // by reusing the same disclaimers_page sections /disclaimers renders.
    let mut disclosure_sections = String::new();
    for section in state.disclaimers_page.sections.iter() {
        disclosure_sections.push_str(&format!(
            "<h3>{}</h3>{}",
            esc(&section.heading),
            section.body_html,
        ));
    }
    let theme_toggle = if editor_route {
        String::new()
    } else {
        r#"<button class="bim-theme-toggle" type="button" aria-pressed="false" aria-label="Switch to dark theme">
        <svg class="bim-theme-toggle__sun" aria-hidden="true" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">
          <circle cx="10" cy="10" r="4" stroke="currentColor" stroke-width="1.5"></circle>
          <path d="M10 1.5V3.5M10 16.5V18.5M18.5 10H16.5M3.5 10H1.5M15.9 4.1L14.5 5.5M5.5 14.5L4.1 15.9M15.9 15.9L14.5 14.5M5.5 5.5L4.1 4.1" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"></path>
        </svg>
        <svg class="bim-theme-toggle__moon" aria-hidden="true" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">
          <path d="M17 11.5A7 7 0 118.5 3a5.5 5.5 0 108.5 8.5Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"></path>
        </svg>
      </button>"#
            .to_string()
    };
    // Carbon Web Components + their CSS are only used by /edit/* (real
    // <cds-content-switcher> etc.) — the public catalog no longer borrows
    // Carbon's visual language, so it no longer ships Carbon's CSS either.
    let carbon_assets = if editor_route {
        r#"
  <link rel="stylesheet" href="/static/carbon.min.css">
  <link rel="stylesheet" href="/static/carbon-overrides.css">
  <script type="module" src="/static/carbon.esm.js"></script>"#
    } else {
        ""
    };
    let html_theme_attr = if editor_route { r#" data-theme="light""# } else { "" };
    let theme_preload_script = if editor_route {
        String::new()
    } else {
        r#"
  <script>
    (function () {
      var stored = null;
      try { stored = localStorage.getItem('bim-theme'); } catch (e) {}
      var theme = stored || (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
      document.documentElement.setAttribute('data-theme', theme);
    })();
  </script>"#
            .to_string()
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en"{html_theme_attr}>
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{full_title}</title>
  <meta name="description" content="Building specifications that enforce compliance at placement, not inspection after the fact. Open-standard IFC 4.3 BIM Object catalog.">
  <link rel="stylesheet" href="/static/fonts.css">
  <link rel="stylesheet" href="/static/tokens.css">
  <link rel="stylesheet" href="/static/bim-layout.css">
  <link rel="stylesheet" href="/static/bim-components.css">{carbon_assets}{theme_preload_script}
  <script type="module" src="/static/bim.js"></script>
</head>
<body class="bim-body">
  <header class="bim-header">
    <div class="bim-header__inner">
      <a href="/" class="bim-header__brand" aria-label="Woodfine — BIM Object Library" data-path="/">BIM Object Library</a>
      <div class="bim-header__right">
        <span class="bim-header__standards">IFC 4.3 &middot; ISO 16739-1:2024 &middot; DTCG</span>
        {theme_toggle}
      </div>
    </div>
  </header>
  <div class="bim-shell">
    <main id="bim-main-content" class="bim-main">
      {content}
    </main>
  </div>
  <section class="bim-disclosure" aria-label="Important information">
    <details class="bim-disclosure__details">
      <summary class="bim-disclosure__summary">Important Information</summary>
      <div class="bim-disclosure__body">
        <p class="bim-disclosure__label">BIM Object Library disclosure</p>
        {disclosure_sections}
        <p class="bim-disclosure__more"><a href="/disclaimers" data-path="/disclaimers">Full disclaimer &rarr;</a></p>
      </div>
    </details>
  </section>
  <footer class="bim-footer">
    <div class="bim-footer__inner">
      <div>
        <p class="bim-footer__heading">Woodfine BIM Object Library</p>
        <ul class="bim-footer__list">
          <li>Specification BIM Objects for the built environment</li>
          <li>{tc} BIM Object categories &middot; {comp} components &middot; {rc} research&nbsp;entries</li>
          <li>IFC&nbsp;4.3 (ISO&nbsp;16739-1:2024) &middot; Uniclass&nbsp;2015 &middot; DTCG</li>
          <li>BIM Object data licensed <strong>Apache-2.0</strong> &middot; platform code <strong>AGPL-3.0-or-later</strong></li>
          <li><a href="https://github.com/pointsav/pointsav-monorepo">Source (github.com/pointsav)</a></li>
        </ul>
      </div>
      <div>
        <p class="bim-footer__heading">Machine-readable surface</p>
        <ul class="bim-footer__list">
          <li><a href="/api/tokens.json">/api/tokens.json</a> &mdash; full DTCG bundle</li>
          <li><a href="/mcp">/mcp</a> &mdash; MCP JSON-RPC endpoint</li>
          <li><a href="/research">/research</a> &mdash; research backplane</li>
        </ul>
      </div>
    </div>
    <p class="bim-footer__family">Part of the Woodfine network:
      <a href="https://woodfinegroup.com">home</a> &middot;
      <a href="https://corporate.woodfinegroup.com" target="_blank" rel="noopener">Corporate</a> &middot;
      <a href="https://projects.woodfinegroup.com" target="_blank" rel="noopener">Projects</a> &middot;
      <a href="https://github.com/pointsav" target="_blank" rel="noopener">GitHub</a>
    </p>
    <div class="bim-footer__base">
      <div class="bim-footer__base-row">
        <div class="bim-footer__cities">
          <span>Vancouver</span>
          <span class="bim-footer__cities-sep" aria-hidden="true">|</span>
          <span>New York</span>
        </div>
        <div class="bim-footer__badges">
          <span class="bim-badge">
            <svg class="bim-badge__glyph" aria-hidden="true" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">
              <path d="M5 2.5h7l3 3v12a1 1 0 01-1 1H5a1 1 0 01-1-1v-14a1 1 0 011-1Z" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"></path>
              <path d="M12 2.5v3h3" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"></path>
            </svg>
            <span class="bim-badge__text">
              <span class="bim-badge__lead">Powered by</span>
              <span class="bim-badge__name">PrivateGit</span>
            </span>
          </span>
        </div>
      </div>
      <p>Copyright &copy; 2026 Woodfine Capital Projects Inc. See LICENSE for terms. &middot; {public_url}</p>
      <p class="bim-footer__disclaimer">Provided for reference and coordination only — not a substitute for code review. See <a href="/disclaimers" data-path="/disclaimers">Important Information</a>.</p>
      <p class="bim-footer__trademark">Woodfine Capital Projects&trade;, Woodfine Management Corp&trade;, PointSav Digital Systems&trade;, Totebox Orchestration&trade;, Totebox Archive&trade;, and Capability Geometry&trade; are trademarks of Woodfine Capital Projects Inc., used in Canada, the United States, Latin America, and Europe. Capability Geometry&trade; is an unregistered trademark of Woodfine Capital Projects Inc. All other trademarks are the property of their respective owners.</p>
    </div>
  </footer>
</body>
</html>"#,
        full_title = full_title,
        html_theme_attr = html_theme_attr,
        carbon_assets = carbon_assets,
        theme_preload_script = theme_preload_script,
        theme_toggle = theme_toggle,
        disclosure_sections = disclosure_sections,
        content = content,
        tc = tc,
        comp = state.components_count,
        rc = state.research_count,
        public_url = esc(&state.config.public_url),
    )
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
