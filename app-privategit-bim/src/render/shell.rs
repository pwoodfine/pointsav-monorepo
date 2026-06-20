use crate::{schema::dtcg::SIDEBAR_ORDER, state::AppState};

pub fn page_shell(title: &str, active_path: &str, content: &str, state: &AppState) -> String {
    let sidebar = super::sidebar::render_sidebar(active_path, state);
    let tc = SIDEBAR_ORDER.len();
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
  <link rel="stylesheet" href="/static/carbon.min.css">
  <link rel="stylesheet" href="/static/carbon-overrides.css">
  <link rel="stylesheet" href="/static/bim-layout.css">
  <link rel="stylesheet" href="/static/bim-components.css">
  <script type="module" src="/static/carbon.esm.js"></script>
  <script type="module" src="/static/bim.js"></script>
</head>
<body class="bim-body">
  <cds-header aria-label="BIM Object Library">
    <cds-header-name href="/" prefix="Woodfine">BIM Object Library</cds-header-name>
    <div class="bim-header-spacer"></div>
  </cds-header>
  <div class="bim-shell">
    <cds-side-nav id="side-nav" aria-label="BIM sidebar" is-not-child-of-header expanded>
      {sidebar}
    </cds-side-nav>
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
          <li>{tc} BIM Object categories &middot; IFC&nbsp;4.3 (ISO&nbsp;16739-1:2024) &middot; DTCG 2025.10</li>
        </ul>
      </div>
      <div>
        <p class="bim-footer__heading">Machine-readable surface</p>
        <ul class="bim-footer__list">
          <li><a href="/tokens.json">/tokens.json</a> &mdash; full DTCG bundle</li>
          <li><a href="/components">/components</a> &mdash; component recipes</li>
          <li><a href="/healthz">/healthz</a> &middot; <a href="/readyz">/readyz</a></li>
        </ul>
      </div>
      <div>
        <p class="bim-footer__heading">Platform</p>
        <ul class="bim-footer__list">
          <li>Open-source &middot; Apache-2.0</li>
          <li>Powered by <strong>PointSav Digital Systems</strong></li>
        </ul>
      </div>
    </div>
  </footer>
</body>
</html>"#,
        full_title = full_title,
        sidebar = sidebar,
        content = content,
        tc = tc,
    )
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
