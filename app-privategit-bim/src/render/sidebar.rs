use crate::state::AppState;

fn nav_link(href: &str, active_path: &str, label: &str) -> String {
    let active = if active_path == href {
        r#" aria-current="page" class="bim-nav-link active""#
    } else {
        r#" class="bim-nav-link""#
    };
    format!(r#"<a href="{href}"{active}>{label}</a>"#)
}

fn nav_group(heading: &str, links: &str) -> String {
    format!(
        r#"<div class="bim-nav-group">
  <p class="bim-nav-group__heading">{heading}</p>
  {links}
</div>"#
    )
}

pub fn render_sidebar(active_path: &str, state: &AppState) -> String {
    let overview = nav_group(
        "Overview",
        &format!(
            "{}{}{}",
            nav_link("/", active_path, "What are BIM Objects?"),
            nav_link("/tokens", active_path, "Browse All BIM Objects"),
            nav_link("/about", active_path, "About BIM Objects"),
        ),
    );

    let mut category_links = String::new();
    for cat in state.categories.iter() {
        let href = format!("/tokens/{}", cat.slug);
        category_links.push_str(&nav_link(&href, active_path, &cat.display_name));
    }
    let objects = nav_group("BIM Objects", &category_links);

    let more = nav_group(
        "More",
        &format!(
            "{}{}{}",
            nav_link("/key-plans", active_path, "Key Plans"),
            nav_link("/furniture", active_path, "Furniture Library"),
            nav_link("/research", active_path, "Research"),
        ),
    );

    format!("{overview}{objects}{more}")
}
