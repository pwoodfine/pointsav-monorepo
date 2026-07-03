use crate::state::AppState;

/// The Woodfine wordmark, identical to the SVG home.woodfinegroup.com renders
/// (fill left as `currentColor` so callers control light/dark placement via CSS).
fn wordmark_svg(class: &str) -> String {
    format!(
        r##"<svg class="{class}" aria-hidden="true" viewBox="0 0 144 36" xmlns="http://www.w3.org/2000/svg">
        <g fill="currentColor" fill-rule="evenodd">
          <path d="M 16.069 29.8287 L 16.069 31.0989 L 26.233 31.0989 L 26.233 29.8287 Z"></path>
          <path d="M 118.051 29.8287 L 118.051 31.0989 L 128.216 31.0989 L 128.216 29.8287 Z"></path>
          <path d="M 19.7019 23.1944 L 23.3651 23.1944 L 26.3901 9.48497 L 26.4456 9.48497 L 29.2485 23.1944 L 32.9395 23.1944 L 37.3244 3.15753 L 33.7444 3.15753 L 31.0802 16.2565 L 31.0247 16.2565 L 28.2495 3.15753 L 24.5585 3.15753 L 21.811 16.2565 L 21.7555 16.2565 L 19.0636 3.15753 L 15.5113 3.15753 L 19.7019 23.1944 Z M 38.5452 16.756 C 38.5452 21.6958 41.1539 23.472 44.8727 23.472 C 48.5914 23.472 51.2001 21.6958 51.2001 16.756 L 51.2001 9.59598 C 51.2001 4.65613 48.5914 2.88 44.8727 2.88 C 41.1539 2.88 38.5452 4.65613 38.5452 9.59598 L 38.5452 16.756 Z M 42.375 9.09645 C 42.375 6.87629 43.3463 6.26575 44.8727 6.26575 C 46.399 6.26575 47.3704 6.87629 47.3704 9.09645 L 47.3704 17.2555 C 47.3704 19.4757 46.399 20.0862 44.8727 20.0862 C 43.3463 20.0862 42.375 19.4757 42.375 17.2555 L 42.375 9.09645 Z M 53.4203 16.756 C 53.4203 21.6958 56.029 23.472 59.7477 23.472 C 63.4665 23.472 66.0752 21.6958 66.0752 16.756 L 66.0752 9.59598 C 66.0752 4.65613 63.4665 2.88 59.7477 2.88 C 56.029 2.88 53.4203 4.65613 53.4203 9.59598 L 53.4203 16.756 Z M 57.25 9.09645 C 57.25 6.87629 58.2214 6.26575 59.7477 6.26575 C 61.2741 6.26575 62.2454 6.87629 62.2454 9.09645 L 62.2454 17.2555 C 62.2454 19.4757 61.2741 20.0862 59.7477 20.0862 C 58.2214 20.0862 57.25 19.4757 57.25 17.2555 L 57.25 9.09645 Z M 68.4896 23.1944 L 73.818 23.1944 C 78.2028 23.1944 80.6449 21.3073 80.8114 16.2565 L 80.8114 10.0955 C 80.6449 5.04466 78.2028 3.15753 73.818 3.15753 L 68.4896 3.15753 L 68.4896 23.1944 Z M 72.3194 6.54326 L 73.6514 6.54326 C 76.0381 6.54326 76.9817 7.70885 76.9817 10.5395 L 76.9817 15.8124 C 76.9817 18.8096 75.7606 19.8087 73.6514 19.8087 L 72.3194 19.8087 L 72.3194 6.54326 Z M 86.9169 23.1944 L 86.9169 14.5358 L 91.7457 14.5358 L 91.7457 11.1501 L 86.9169 11.1501 L 86.9169 6.54326 L 93.0778 6.54326 L 93.0778 3.15753 L 83.0871 3.15753 L 83.0871 23.1944 L 86.9169 23.1944 Z M 99.4603 23.1944 L 99.4603 3.15753 L 95.6306 3.15753 L 95.6306 23.1944 L 99.4603 23.1944 Z M 105.594 23.1944 L 105.594 10.262 L 105.649 10.262 L 111.505 23.1944 L 115.168 23.1944 L 115.168 3.15753 L 111.671 3.15753 L 111.671 15.0354 L 111.616 15.0354 L 106.287 3.15753 L 102.097 3.15753 L 102.097 23.1944 L 105.594 23.1944 Z M 128.489 23.1944 L 128.489 19.8087 L 121.551 19.8087 L 121.551 14.5358 L 126.629 14.5358 L 126.629 11.1501 L 121.551 11.1501 L 121.551 6.54326 L 128.211 6.54326 L 128.211 3.15753 L 117.721 3.15753 L 117.721 23.1944 L 128.489 23.1944 Z"></path>
        </g>
      </svg>"##,
        class = class
    )
}

pub fn page_shell(title: &str, active_path: &str, content: &str, state: &AppState) -> String {
    let sidebar = super::sidebar::render_sidebar(active_path, state);
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
    let wordmark = wordmark_svg("bim-header__logo");
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
  <link rel="stylesheet" href="/static/carbon.min.css">
  <link rel="stylesheet" href="/static/carbon-overrides.css">
  <link rel="stylesheet" href="/static/bim-layout.css">
  <link rel="stylesheet" href="/static/bim-components.css">{theme_preload_script}
  <script type="module" src="/static/carbon.esm.js"></script>
  <script type="module" src="/static/bim.js"></script>
</head>
<body class="bim-body">
  <div class="bim-utility">
    <div class="bim-utility__inner">
      <a href="https://woodfinegroup.com" class="bim-utility__home">Woodfine Capital Projects</a>
      <nav class="bim-utility__nav" aria-label="Woodfine network">
        <a class="bim-utility__link" href="https://corporate.woodfinegroup.com" target="_blank" rel="noopener">Corporate</a>
        <a class="bim-utility__link" href="https://projects.woodfinegroup.com" target="_blank" rel="noopener">Projects</a>
        <a class="bim-utility__link" href="https://github.com/pointsav" target="_blank" rel="noopener">GitHub</a>
      </nav>
    </div>
  </div>
  <header class="bim-header">
    <div class="bim-header__inner">
      <button class="bim-topbar__toggle" aria-label="Toggle menu" aria-expanded="false" type="button">&#9776;</button>
      <a href="/" class="bim-header__brand" aria-label="Woodfine — BIM Object Library" data-path="/">
        {wordmark}
        <span class="bim-header__lockup">
          <span class="bim-header__word">Woodfine</span>
          <span class="bim-header__subtitle">BIM Object Library</span>
        </span>
      </a>
      <form class="bim-search" action="/search" method="get" role="search">
        <label class="bim-search__label" for="bim-search-input">Search BIM Objects</label>
        <div class="bim-search__form">
          <svg class="bim-search__icon" aria-hidden="true" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">
            <circle cx="9" cy="9" r="6.5" stroke="currentColor" stroke-width="1.5"></circle>
            <path d="M14 14L18 18" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"></path>
          </svg>
          <input id="bim-search-input" class="bim-search__input" type="search" name="q" placeholder="Search categories, entities, research&hellip;" autocomplete="off">
          <button class="bim-search__button" type="submit"><span class="bim-search__button-label">Search</span></button>
        </div>
      </form>
      {theme_toggle}
    </div>
  </header>
  <div class="bim-shell">
    <nav class="bim-side-nav" aria-label="BIM sidebar">
      {sidebar}
    </nav>
    <main id="bim-main-content" class="bim-main">
      {content}
    </main>
  </div>
  <footer class="bim-footer">
    <div class="bim-footer__inner">
      <div>
        <p class="bim-footer__heading">Woodfine BIM Object Library</p>
        <ul class="bim-footer__list">
          <li>Specification BIM Objects for the built environment</li>
          <li>{tc} BIM Object categories &middot; {comp} components &middot; {rc} research&nbsp;entries</li>
          <li>IFC&nbsp;4.3 (ISO&nbsp;16739-1:2024) &middot; Uniclass&nbsp;2015 &middot; DTCG</li>
          <li>BIM Object data licensed <strong>Apache-2.0</strong></li>
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
        <p class="bim-footer__heading">Platform</p>
        <ul class="bim-footer__list">
          <li>Platform code licensed <strong>AGPL-3.0-or-later</strong></li>
          <li><a href="https://github.com/pointsav/pointsav-monorepo">Source (github.com/pointsav)</a></li>
          <li>Powered by <strong>PointSav Digital Systems&trade;</strong></li>
          <li><a href="https://pointsav.com">pointsav.com</a></li>
        </ul>
      </div>
    </div>
    <div class="bim-footer__base">
      <p>Copyright &copy; 2026 Woodfine Capital Projects Inc. See LICENSE for terms. &middot; {public_url}</p>
      <p class="bim-footer__trademark">Woodfine Capital Projects&trade;, Woodfine Management Corp&trade;, PointSav Digital Systems&trade;, Totebox Orchestration&trade;, Totebox Archive&trade;, and Capability Geometry&trade; are trademarks of Woodfine Capital Projects Inc., used in Canada, the United States, Latin America, and Europe. Capability Geometry&trade; is an unregistered trademark of Woodfine Capital Projects Inc. All other trademarks are the property of their respective owners.</p>
    </div>
  </footer>
</body>
</html>"#,
        full_title = full_title,
        html_theme_attr = html_theme_attr,
        theme_preload_script = theme_preload_script,
        wordmark = wordmark,
        theme_toggle = theme_toggle,
        sidebar = sidebar,
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
