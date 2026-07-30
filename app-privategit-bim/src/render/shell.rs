// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use crate::state::AppState;

pub fn page_shell(title: &str, active_path: &str, content: &str, state: &AppState) -> String {
    let tc = state.categories.len();
    let full_title = if title.is_empty() {
        "Woodfine BIM Library".to_string()
    } else {
        format!("{} — Woodfine BIM Library", esc(title))
    };

    // /edit/* embeds real Carbon Web Components (<cds-content-switcher> etc.)
    // that are only styled for a light Carbon theme — force light there
    // server-side rather than trying to make Carbon's chrome theme-reactive.
    let editor_route = active_path.starts_with("/edit/");
    // "Important Information" band: a short, counsel-owned summary from
    // important-information.md — NOT the full disclaimers_page content
    // (that's a separate, deliberate earlier fix for a different bug —
    // see BRIEF-app-privategit-bim.md's 2026-07-07 entry). This matches
    // Command's actual spec (2026-07-02) and the proven, counsel-approved
    // reference pattern already shipped on project-knowledge's
    // app-mediakit-knowledge: short band + "Full disclaimer" link to the
    // long-form page, with a safe issuer-aware default if the file is ever
    // missing (never a hard failure).
    let disclosure_body: &str = state.important_information.as_deref().unwrap_or(
        "<p>This site presents records maintained by Woodfine Capital Projects Inc. \
The information is provided for general information only and does not constitute \
an offer to sell, a solicitation of an offer to buy, or investment, legal, tax, or \
accounting advice. Statements regarding planned, intended, or targeted future \
activities are forward-looking and subject to change without notice; they are not \
undertaken to be updated except as required by law.</p>",
    );
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
      <a href="/" class="bim-header__brand" aria-label="Woodfine — BIM Library" data-path="/">BIM Library</a>
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
        <p class="bim-disclosure__label">BIM Library disclosure</p>
        {disclosure_body}
        <p class="bim-disclosure__more"><a href="/disclaimers">Full disclaimer &rarr;</a></p>
      </div>
    </details>
  </section>
  <footer class="bim-footer">
    <div class="bim-footer__inner">
      <div>
        <p class="bim-footer__heading">Woodfine BIM Library</p>
        <ul class="bim-footer__list">
          <li>Specification BIM Objects for the built environment</li>
          <li>{tc} BIM Object categories &middot; {comp} components &middot; {rc} research&nbsp;entries</li>
          <li>IFC&nbsp;4.3 (ISO&nbsp;16739-1:2024) &middot; Uniclass&nbsp;2015 &middot; DTCG</li>
          <li>BIM Object data licensed <strong>Apache-2.0</strong> &middot; platform code <strong>AGPL-3.0-or-later</strong></li>
          <li><a href="https://github.com/pointsav/pointsav-monorepo">Platform source code (github.com/pointsav)</a></li>
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
      <div>
        <p class="bim-footer__heading">Woodfine network</p>
        <ul class="bim-footer__list">
          <li><a href="https://home.woodfinegroup.com" target="_blank" rel="noopener">Woodfine Capital Projects</a></li>
          <li><a href="https://corporate.woodfinegroup.com" target="_blank" rel="noopener">Corporate</a></li>
          <li><a href="https://projects.woodfinegroup.com" target="_blank" rel="noopener">Projects</a></li>
          <li><a href="https://github.com/woodfine/woodfine-bim-library" target="_blank" rel="noopener">GitHub</a></li>
          <li><a href="https://home.pointsav.com" target="_blank" rel="noopener">PointSav Digital Systems</a></li>
        </ul>
      </div>
    </div>
    <div class="bim-footer__base">
      <div class="bim-footer__base-row">
        <div class="bim-footer__cities">
          <span>Vancouver</span>
          <span class="bim-footer__cities-sep" aria-hidden="true">|</span>
          <span>New York</span>
        </div>
        <div class="bim-footer__badges">
          <a class="bim-badge bim-badge--license" href="https://creativecommons.org/licenses/by-nd/4.0/"
             target="_blank" rel="noopener license" aria-label="Content licensed CC BY-ND 4.0">
            <span class="bim-badge__cc" aria-hidden="true">
              <img class="bim-cc-icon" src="/static/cc.svg" alt="" width="20" height="20">
              <img class="bim-cc-icon" src="/static/cc-by.svg" alt="" width="20" height="20">
              <img class="bim-cc-icon" src="/static/cc-nd.svg" alt="" width="20" height="20">
            </span>
            <span class="bim-badge__text">
              <span class="bim-badge__lead">Licensed</span>
              <span class="bim-badge__name">CC BY-ND 4.0</span>
            </span>
          </a>
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
      <p>Copyright &copy; 2026 Woodfine Capital Projects Inc. See <a href="https://github.com/pointsav/pointsav-monorepo/blob/main/app-privategit-bim/LICENSE" target="_blank" rel="noopener">LICENSE</a> for terms.</p>
      <p class="bim-footer__disclaimer">Provided for reference and coordination only — not a substitute for code review.</p>
      <p class="bim-footer__trademark">Woodfine Capital Projects&trade;, MCorp&trade;, PointSav Digital Systems&trade;, Totebox Orchestration&trade;, Totebox Archive&trade;, and Capability Geometry&trade; are trademarks of Woodfine Capital Projects Inc., used in Canada, the United States, Latin America, and Europe. Capability Geometry&trade; is an unregistered trademark of Woodfine Capital Projects Inc. All other trademarks are the property of their respective owners.</p>
    </div>
  </footer>
</body>
</html>"#,
        full_title = full_title,
        html_theme_attr = html_theme_attr,
        carbon_assets = carbon_assets,
        theme_preload_script = theme_preload_script,
        theme_toggle = theme_toggle,
        disclosure_body = disclosure_body,
        content = content,
        tc = tc,
        comp = state.components_count,
        rc = state.research_count,
    )
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
