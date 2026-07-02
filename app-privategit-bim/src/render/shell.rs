use crate::state::AppState;

pub fn page_shell(title: &str, active_path: &str, content: &str, state: &AppState) -> String {
    let sidebar = super::sidebar::render_sidebar(active_path, state);
    let tc = state.categories.len();
    let full_title = if title.is_empty() {
        "BIM Object Library — Woodfine".to_string()
    } else {
        format!("{} — BIM Object Library", esc(title))
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
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
  <link rel="stylesheet" href="/static/bim-components.css">
  <script type="module" src="/static/carbon.esm.js"></script>
  <script type="module" src="/static/bim.js"></script>
</head>
<body class="bim-body">
  <header class="bim-topbar">
    <a href="/" class="bim-topbar__brand">Woodfine</a>
    <span class="bim-topbar__sep" aria-hidden="true"></span>
    <span class="bim-topbar__label">BIM Object Library</span>
    <div class="bim-header-spacer"></div>
    <span class="bim-topbar__meta">app-privategit-bim</span>
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
          <li>{tc} BIM Object categories &middot; {comp} components &middot; {rc} research entries</li>
          <li>IFC&nbsp;4.3 (ISO&nbsp;16739-1:2024) &middot; Uniclass&nbsp;2015 &middot; DTCG</li>
        </ul>
      </div>
      <div>
        <p class="bim-footer__heading">Machine-readable surface</p>
        <ul class="bim-footer__list">
          <li><a href="/api/tokens.json">/api/tokens.json</a> &mdash; full DTCG bundle</li>
          <li><a href="/mcp">/mcp</a> &mdash; MCP JSON-RPC endpoint</li>
          <li><a href="/research">/research</a> &mdash; research backplane</li>
          <li><a href="/healthz">/healthz</a> &middot; <a href="/readyz">/readyz</a></li>
        </ul>
      </div>
      <div>
        <p class="bim-footer__heading">Platform</p>
        <ul class="bim-footer__list">
          <li>Open-source &middot; Apache-2.0</li>
          <li>Powered by <strong>PointSav Digital Systems</strong></li>
          <li><a href="https://pointsav.com">pointsav.com</a></li>
        </ul>
      </div>
    </div>
    <div class="bim-footer__base">&copy; 2026 Woodfine Capital Projects Inc. &middot; {public_url}</div>
  </footer>
</body>
</html>"#,
        full_title = full_title,
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
