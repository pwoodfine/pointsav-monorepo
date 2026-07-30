use crate::state::AppState;

pub fn page_shell(title: &str, active_path: &str, content: &str, state: &AppState) -> String {
    let sidebar = super::sidebar::render_sidebar(active_path, state);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title} — BIM Objects</title>
  <link rel="stylesheet" href="/static/carbon.min.css">
  <link rel="stylesheet" href="/static/carbon-overrides.css">
  <link rel="stylesheet" href="/static/bim-layout.css">
  <link rel="stylesheet" href="/static/bim-components.css">
  <script type="module" src="/static/carbon.esm.js"></script>
  <script type="module" src="/static/bim.js"></script>
</head>
<body class="bim-body">
  <cds-header aria-label="BIM Objects">
    <cds-header-menu-button button-label-active="Close menu" button-label-inactive="Open menu" panel-id="side-nav">
    </cds-header-menu-button>
    <cds-header-name href="/" prefix="">BIM Objects</cds-header-name>
    <div class="bim-header-spacer"></div>
  </cds-header>
  <div class="bim-shell">
    <cds-side-nav id="side-nav" aria-label="BIM sidebar" is-not-child-of-header>
      {sidebar}
    </cds-side-nav>
    <main id="bim-main-content" class="bim-main">
      {content}
    </main>
  </div>
</body>
</html>"#,
        title = esc(title),
        sidebar = sidebar,
        content = content,
    )
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
