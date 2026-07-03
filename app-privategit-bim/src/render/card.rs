use crate::{content, state::AppState};
use serde_json::Value;

use super::shell::esc;

pub fn render_home(state: &AppState) -> String {
    let cards = render_category_cards(state);
    let category_count = state.categories.len();
    let page = &state.home_page;
    let hero_eyebrow = esc(&page.field("hero_eyebrow"));
    // hero_statline carries an intentional literal <br>, so it's not escaped.
    let hero_statline = page.field("hero_statline");
    let hero_lead = esc(&page.field("hero_lead"));

    let mut sections = String::new();
    for (i, section) in page.sections.iter().enumerate() {
        if i > 0 {
            sections.push_str(r#"<hr class="bim-rule">"#);
        }
        sections.push_str(&format!(
            "<section><h2>{}</h2>{}</section>",
            esc(&section.heading),
            section.body_html,
        ));
    }

    format!(
        r#"<div class="bim-hero">
  <p class="bim-hero__eyebrow">{hero_eyebrow}</p>
  <p class="bim-hero__statline">{hero_statline}</p>
  <p class="bim-hero__lead">{hero_lead}</p>
  <div class="bim-chip-row">
    <span class="bim-chip">CATEGORIES <strong>{category_count}</strong></span>
    <span class="bim-chip">STANDARD <strong>IFC 4.3 &middot; ISO 16739-1:2024</strong></span>
    <span class="bim-chip bim-chip--muted">FORMAT <strong>DTCG</strong></span>
  </div>
</div>
<hr class="bim-rule">
<article class="bim-article">
  {sections}
</article>
<div class="bim-home">
  <h2>Categories</h2>
  <div class="bim-category-grid">{cards}</div>
</div>"#,
    )
}

pub fn render_tokens_index(state: &AppState) -> String {
    render_home(state)
}

pub fn render_token_page(category: &str, state: &AppState) -> String {
    let meta = state.categories.iter().find(|c| c.slug == category);

    let Some(file_val) = state.tokens.get(category) else {
        return format!(
            r#"<div class="bim-empty"><p>No token file found for category <code>{}</code>.</p></div>"#,
            esc(category)
        );
    };

    let bim = match file_val.get("bim").and_then(|v| v.as_object()) {
        Some(b) => b,
        None => {
            return format!(
                r#"<div class="bim-empty"><p>Token file for <code>{}</code> has no 'bim' root.</p></div>"#,
                esc(category)
            );
        }
    };

    let intro_html = meta.map(|m| m.intro_html.as_str()).unwrap_or("");
    let ifc_anchor = meta.map(|m| m.ifc_anchor.as_str()).unwrap_or("");
    let elements = meta.map(|m| m.elements.as_str()).unwrap_or("");
    let uniclass = meta.map(|m| m.uniclass.as_str()).unwrap_or("—");
    let ifc_hierarchy = meta.map(|m| m.ifc_hierarchy.as_str()).unwrap_or("—");
    let empty_psets = Vec::new();
    let property_sets = meta.map(|m| &m.property_sets).unwrap_or(&empty_psets);

    let mut entity_count = 0usize;
    let mut rows = String::new();
    for (_cat_key, cat_val) in bim {
        if let Some(entities) = cat_val.as_object() {
            let mut slugs: Vec<&String> = entities.keys().collect();
            slugs.sort();
            for slug in slugs {
                entity_count += 1;
                let entity = &entities[slug];
                let description = entity
                    .get("$description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let ifc_class = entity
                    .get("$value")
                    .and_then(|v| v.get("ifc_class"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("—");
                rows.push_str(&format!(
                    r#"<tr>
  <td><code>{slug}</code></td>
  <td><code>{ifc_class}</code></td>
  <td>{description}</td>
</tr>"#,
                    slug = esc(slug),
                    ifc_class = esc(ifc_class),
                    description = esc(description),
                ));
            }
        }
    }

    let mut pset_rows = String::new();
    for (pset, prop, ty) in property_sets {
        pset_rows.push_str(&format!(
            r#"<tr><td><code>{pset}</code></td><td><code>{prop}</code></td><td><code>{ty}</code></td></tr>"#,
            pset = esc(pset),
            prop = esc(prop),
            ty = esc(ty),
        ));
    }
    let pset_block = if pset_rows.is_empty() {
        r#"<p class="bim-empty">No property sets registered for this category yet.</p>"#
            .to_string()
    } else {
        format!(
            r#"<table class="bim-table-wrap bim-token-table">
  <thead><tr><th>Property set</th><th>Property</th><th>Type</th></tr></thead>
  <tbody>{pset_rows}</tbody>
</table>"#
        )
    };

    let dtcg_json = serde_json::to_string_pretty(file_val).unwrap_or_default();

    format!(
        r#"<div class="bim-category-page">
  <div class="bim-breadcrumbs">
    <a href="/" data-path="/" class="bim-nav-link">Home</a> / <a href="/tokens" data-path="/tokens" class="bim-nav-link">BIM Objects</a>
  </div>
  <p class="bim-category-page__anchor"><code>{ifc_anchor}</code></p>
  <h1>{display_name}</h1>
  <div class="bim-chip-row">
    <span class="bim-chip">IFC <code>{ifc_anchor}</code></span>
    <span class="bim-chip bim-chip--accent">UNICLASS <strong>{uniclass}</strong></span>
    <span class="bim-chip bim-chip--muted">REGULATORY OVERLAYS <strong>0 registered</strong></span>
  </div>

  <details class="bim-spec-card" open>
    <summary>Specification</summary>
    <div class="bim-spec-card__body">
      <div class="bim-intro">{intro_html}</div>
      <p class="bim-elements"><code>{elements}</code></p>
      <table class="bim-detail-table">
        <tr><th>IFC entity</th><td><code>{ifc_anchor}</code></td></tr>
        <tr><th>Uniclass 2015</th><td>{uniclass}</td></tr>
        <tr><th>bSDD URI</th><td class="bim-fg-muted">pending</td></tr>
        <tr><th>IFC hierarchy</th><td class="bim-ifc-hierarchy"><code>{ifc_hierarchy}</code></td></tr>
      </table>
      <h2>Applicable property sets</h2>
      {pset_block}
    </div>
  </details>

  <details class="bim-spec-card" open>
    <summary>BIM Objects ({entity_count})</summary>
    <div class="bim-spec-card__body">
      <table class="bim-token-table">
        <thead>
          <tr>
            <th>Token slug</th>
            <th>IFC class</th>
            <th>Description</th>
          </tr>
        </thead>
        <tbody>{rows}</tbody>
      </table>
    </div>
  </details>

  <details class="bim-accordion">
    <summary>Regulation</summary>
    <div class="bim-spec-card__body"><p class="bim-empty">No regulatory overlays registered for this category yet.</p></div>
  </details>
  <details class="bim-accordion">
    <summary>Climate Zone</summary>
    <div class="bim-spec-card__body"><p class="bim-empty">Climate zone constraints not yet modeled for this category.</p></div>
  </details>
  <details class="bim-accordion">
    <summary>Token Format</summary>
    <div class="bim-spec-card__body"><pre><code>{dtcg_json}</code></pre></div>
  </details>
</div>"#,
        display_name = esc(meta.map(|m| m.display_name.as_str()).unwrap_or(category)),
        intro_html = intro_html,
        ifc_anchor = esc(ifc_anchor),
        elements = esc(elements),
        uniclass = esc(uniclass),
        ifc_hierarchy = esc(ifc_hierarchy),
        pset_block = pset_block,
        entity_count = entity_count,
        rows = rows,
        dtcg_json = esc(&dtcg_json),
    )
}

pub fn render_key_plans(state: &AppState) -> String {
    // Phase 4 will fill in SVG zone diagrams; stub for compile
    let Some(file_val) = state.tokens.get("key-plans") else {
        return r#"<div class="bim-empty"><p>key-plans.dtcg.json not found in library.</p></div>"#
            .into();
    };
    let bim = match file_val.get("bim").and_then(|v| v.as_object()) {
        Some(b) => b,
        None => {
            return r#"<div class="bim-empty"><p>No bim root in key-plans.dtcg.json.</p></div>"#
                .into()
        }
    };

    // key-plans.dtcg.json nests entities three levels deep — category (e.g.
    // "key-plan") -> subcategory (e.g. "private-office") -> size variant
    // (e.g. "small"), each variant carrying $type/$value. Walk to any depth
    // and collect every node that actually has a $value, rather than
    // assuming a fixed depth.
    let mut leaves: Vec<(&str, &Value)> = Vec::new();
    collect_kp_leaves(bim, &mut leaves);
    leaves.sort_by_key(|(slug, _)| *slug);

    let mut cards = String::new();
    for (slug, entity) in leaves {
        let val = entity.get("$value").cloned().unwrap_or(Value::Null);
        let display_name = val
            .get("display_name")
            .and_then(|v| v.as_str())
            .unwrap_or(slug);
        let internal_code = val
            .get("internal_code")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let category = val.get("category").and_then(|v| v.as_str()).unwrap_or("—");
        let area_sf = val.get("area_sf").and_then(|v| v.as_u64()).unwrap_or(0);

        let svg = super::svg::render_kp_zone_svg_from_value(&val);

        cards.push_str(&format!(
            r#"<div class="bim-kp-card">
  <div class="bim-kp-svg">{svg}</div>
  <div class="bim-kp-info">
    <div class="bim-kp-name">{display_name}</div>
    <div class="bim-kp-meta"><span class="bim-tag">{internal_code}</span> <span class="bim-cat">{category}</span></div>
    <div class="bim-kp-area">{area_sf} SF</div>
  </div>
</div>"#,
            display_name = esc(display_name),
            internal_code = esc(internal_code),
            category = esc(category),
            area_sf = area_sf,
            svg = svg,
        ));
    }

    format!(
        r#"<div class="bim-key-plans">
  <h1>Key Plans</h1>
  <p class="bim-intro">Key Plans are the smallest BIM Object unit — spatial programs defined by three-zone cross-section and furniture arrangement.</p>
  <div class="bim-kp-grid">
    {cards}
  </div>
</div>"#,
        cards = cards,
    )
}

pub fn render_furniture(state: &AppState) -> String {
    let components_dir = state.config.library_dir.join("blocks").join("furniture");
    let mut items = String::new();
    if let Ok(rd) = std::fs::read_dir(&components_dir) {
        let mut names: Vec<String> = rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("ifc"))
            .filter_map(|e| {
                e.path()
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .collect();
        names.sort();
        for name in &names {
            items.push_str(&format!(
                r#"<div class="bim-furniture-item">
  <span class="bim-furniture-name">{name}</span>
  <a class="cds-btn cds-btn--ghost" href="/furniture/download/{name}">Download IFC</a>
</div>"#,
                name = esc(name),
            ));
        }
    }

    format!(
        r#"<div class="bim-furniture">
  <h1>Furniture Library</h1>
  <p class="bim-intro">IFC furniture components for use in Key Plan BIM Objects.</p>
  <div class="bim-furniture-actions">
    <a class="cds-btn cds-btn--primary" href="/furniture/download/bundle.zip">Download All (ZIP)</a>
  </div>
  <div class="bim-furniture-list">
    {items}
  </div>
</div>"#,
        items = items,
    )
}

pub fn render_research_index(state: &AppState) -> String {
    let research_dir = state.config.vault_dir.join("research");
    let mut items = String::new();
    if let Ok(rd) = std::fs::read_dir(&research_dir) {
        let mut names: Vec<String> = rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .collect();
        names.sort();
        for slug in &names {
            items.push_str(&format!(
                r#"<div class="bim-research-item">
  <a href="/research/{slug}" data-path="/research/{slug}" class="bim-nav-link">{slug}</a>
</div>"#,
                slug = esc(slug),
            ));
        }
    }
    if items.is_empty() {
        items = r#"<p class="bim-empty">No research documents found.</p>"#.into();
    }
    format!(
        r#"<div class="bim-research"><h1>Research</h1><div class="bim-research-list">{items}</div></div>"#,
        items = items,
    )
}

pub fn render_research_item(slug: &str, state: &AppState) -> String {
    let path = state
        .config
        .vault_dir
        .join("research")
        .join(format!("{slug}.md"));
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => {
            return format!(
                r#"<div class="bim-empty"><p>Research document <code>{}</code> not found.</p></div>"#,
                esc(slug)
            )
        }
    };
    let html_body = content::render_markdown(&raw);
    format!(
        r#"<div class="bim-research-item-page">
  <div class="bim-breadcrumbs">
    <a href="/research" data-path="/research" class="bim-nav-link">Research</a> / <span>{slug}</span>
  </div>
  <div class="bim-markdown">{html_body}</div>
</div>"#,
        slug = esc(slug),
        html_body = html_body,
    )
}

fn render_category_cards(state: &AppState) -> String {
    let mut out = String::new();
    for cat in state.categories.iter() {
        let count = count_entities_in_file(state, &cat.slug);
        out.push_str(&format!(
            r#"<a class="bim-category-card bim-nav-link" href="/tokens/{slug}" data-path="/tokens/{slug}">
  <div class="bim-category-card-name">{display}</div>
  <div class="bim-category-card-desc">{desc}</div>
  <div class="bim-category-card-count">{count} entities</div>
</a>"#,
            slug = cat.slug,
            display = esc(&cat.display_name),
            desc = esc(&cat.card_desc),
            count = count,
        ));
    }
    out
}

/// Recursively walk a DTCG object collecting every node that carries a
/// `$value` field, regardless of nesting depth — key-plans.dtcg.json nests
/// three levels deep (category -> subcategory -> size variant); other files
/// nest two. `slug` is set to the object key one level above the leaf.
fn collect_kp_leaves<'a>(obj: &'a serde_json::Map<String, Value>, out: &mut Vec<(&'a str, &'a Value)>) {
    for (key, val) in obj {
        if key == "$description" {
            continue;
        }
        if val.get("$value").is_some() {
            out.push((key.as_str(), val));
        } else if let Some(child) = val.as_object() {
            collect_kp_leaves(child, out);
        }
    }
}

fn count_entities_in_file(state: &AppState, category: &str) -> usize {
    let Some(file_val) = state.tokens.get(category) else {
        return 0;
    };
    let Some(bim) = file_val.get("bim").and_then(|v| v.as_object()) else {
        return 0;
    };
    bim.values()
        .filter_map(|v| v.as_object())
        .flat_map(|o| o.values())
        .count()
}
