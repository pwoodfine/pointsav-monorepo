// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// Phase 3 (hyperscaler-tier initiative) — per-component origin + freshness badges,
// rendered on each component's own page (not just the sidebar grouping shipped in
// Phase 2). Echoes Spectrum's per-component changelog/version pattern from the external
// research. Thin HTML-formatting layer over `vault::component_origin_label` and
// `vault::file_freshness_label` — those own the actual lookup/mtime logic.
use crate::vault;
use std::path::Path;

/// Origin badge text for a component's own page — distinct wording from the sidebar's
/// "Also used by..." grouping labels, since this reads as a standalone page badge.
pub fn origin_badge(component_groups: &[(String, Vec<String>)], slug: &str) -> Option<String> {
    let label = vault::component_origin_label(component_groups, slug)?;
    let text = if label.contains("gis.woodfinegroup.com") {
        "GIS-origin component"
    } else if label.contains("wiki engine") {
        "Wiki-origin component"
    } else {
        return None;
    };
    Some(format!(
        "<span class=\"component-origin-badge\">{text}</span>"
    ))
}

/// Freshness label sourced from `recipe.json`'s mtime — every component has one, even
/// the undocumented ones, so it's the one file guaranteed to exist as a freshness anchor.
pub fn freshness_badge(vault_dir: &Path, slug: &str) -> Option<String> {
    let recipe_path = vault_dir.join("components").join(slug).join("recipe.json");
    let text = vault::file_freshness_label(&recipe_path)?;
    Some(format!(
        "<span class=\"component-freshness-badge\">{text}</span>"
    ))
}

/// Combined badge row — both badges together, or an empty string if neither applies.
pub fn render_meta_badges(
    component_groups: &[(String, Vec<String>)],
    vault_dir: &Path,
    slug: &str,
) -> String {
    let origin = origin_badge(component_groups, slug).unwrap_or_default();
    let freshness = freshness_badge(vault_dir, slug).unwrap_or_default();
    if origin.is_empty() && freshness.is_empty() {
        String::new()
    } else {
        format!("<div class=\"component-meta-badges\">{origin}{freshness}</div>")
    }
}
