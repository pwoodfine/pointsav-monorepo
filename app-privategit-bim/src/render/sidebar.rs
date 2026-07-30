use crate::{schema::dtcg::SIDEBAR_ORDER, state::AppState};

pub fn render_sidebar(active_path: &str, _state: &AppState) -> String {
    let mut items = String::new();

    // Tokens index
    let tokens_active = if active_path == "/tokens" {
        "active"
    } else {
        ""
    };
    items.push_str(&format!(
        r#"<cds-side-nav-items>
  <cds-side-nav-link href="/tokens" data-path="/tokens" {active} class="bim-nav-link">
    BIM Object Catalog
  </cds-side-nav-link>"#,
        active = if !tokens_active.is_empty() {
            r#"aria-current="page""#
        } else {
            ""
        }
    ));

    // Category links
    items.push_str(r#"<cds-side-nav-menu title="Object Types">"#);
    for (slug, label) in SIDEBAR_ORDER {
        let href = format!("/tokens/{slug}");
        let is_active = active_path == href;
        let active_attr = if is_active {
            r#"aria-current="page""#
        } else {
            ""
        };
        items.push_str(&format!(
            r#"<cds-side-nav-menu-item href="{href}" data-path="{href}" {active_attr} class="bim-nav-link">{label}</cds-side-nav-menu-item>"#,
        ));
    }
    items.push_str("</cds-side-nav-menu>");

    // Key Plans
    let kp_active = if active_path == "/key-plans" {
        r#"aria-current="page""#
    } else {
        ""
    };
    items.push_str(&format!(
        r#"<cds-side-nav-link href="/key-plans" data-path="/key-plans" {kp_active} class="bim-nav-link">Key Plans</cds-side-nav-link>"#,
    ));

    // Furniture
    let fur_active = if active_path == "/furniture" {
        r#"aria-current="page""#
    } else {
        ""
    };
    items.push_str(&format!(
        r#"<cds-side-nav-link href="/furniture" data-path="/furniture" {fur_active} class="bim-nav-link">Furniture Library</cds-side-nav-link>"#,
    ));

    // Research
    let res_active = if active_path == "/research" {
        r#"aria-current="page""#
    } else {
        ""
    };
    items.push_str(&format!(
        r#"<cds-side-nav-link href="/research" data-path="/research" {res_active} class="bim-nav-link">Research</cds-side-nav-link>"#,
    ));

    items.push_str("</cds-side-nav-items>");
    items
}
