// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use crate::content::Section;
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

/// One `<details>` section of the four-section category tree (Taxonomy /
/// Objects / Compositions / Context). Open by default — a catalog shows its
/// inventory rather than hiding it behind a collapsed accordion — but SSR
/// forces `open` on whichever section contains the active page regardless,
/// so a future collapsed-by-default flip stays correct.
fn nav_section(section: Section, active_path: &str, state: &AppState) -> String {
    let members: Vec<_> = state
        .categories
        .iter()
        .filter(|cat| cat.section == section)
        .collect();
    let count = members.len();
    let contains_active = members
        .iter()
        .any(|cat| active_path == format!("/tokens/{}", cat.slug));
    let summary_class = if contains_active {
        "bim-nav-section__summary bim-nav-section__summary--active"
    } else {
        "bim-nav-section__summary"
    };

    let mut links = String::new();
    for cat in &members {
        let href = format!("/tokens/{}", cat.slug);
        links.push_str(&nav_link(&href, active_path, &cat.display_name));
    }

    format!(
        r#"<details class="bim-nav-section" open>
  <summary class="{summary_class}">
    <span class="bim-nav-section__label">{label}</span>
    <span class="bim-nav-section__count">{count}</span>
    <svg class="bim-nav-section__chevron" aria-hidden="true" viewBox="0 0 10 10" fill="none" xmlns="http://www.w3.org/2000/svg">
      <path d="M2.5 1.5L7 5L2.5 8.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"></path>
    </svg>
  </summary>
  <div class="bim-nav-section__links">
    {links}
  </div>
</details>"#,
        label = section.label(),
    )
}

pub fn render_sidebar(active_path: &str, state: &AppState) -> String {
    let top_cluster = nav_group(
        "Overview",
        &format!(
            "{}{}{}",
            nav_link("/", active_path, "Overview"),
            nav_link("/tokens", active_path, "Browse All BIM Objects"),
            nav_link("/about", active_path, "About"),
        ),
    );

    let mut sections = String::new();
    for section in Section::all() {
        sections.push_str(&nav_section(section, active_path, state));
    }

    let bottom_cluster = nav_group(
        "More",
        &format!(
            "{}{}{}",
            nav_link("/key-plans", active_path, "Key Plan Diagrams"),
            nav_link("/furniture", active_path, "Furniture Library"),
            nav_link("/research", active_path, "Research"),
        ),
    );

    format!(
        r#"{top_cluster}
<div class="bim-nav-divider" aria-hidden="true"></div>
{sections}
<div class="bim-nav-divider" aria-hidden="true"></div>
{bottom_cluster}"#
    )
}
