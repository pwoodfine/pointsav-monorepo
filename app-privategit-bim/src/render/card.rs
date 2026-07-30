use crate::{
    schema::dtcg::{known_categories, SIDEBAR_ORDER},
    state::AppState,
};
use serde_json::Value;

use super::shell::esc;

pub fn render_home(state: &AppState) -> String {
    format!(
        r#"<div class="bim-home">
  <h1>BIM Object Catalog</h1>
  <p class="bim-home-subtitle">PointSav BIM Object Schema v1 (PBS-1) — {token_count} entities across {cat_count} categories</p>
  <div class="bim-category-grid">
    {cards}
  </div>
</div>"#,
        token_count = state.token_count,
        cat_count = SIDEBAR_ORDER.len(),
        cards = render_category_cards(state),
    )
}

pub fn render_tokens_index(state: &AppState) -> String {
    render_home(state)
}

pub fn render_token_page(category: &str, state: &AppState) -> String {
    let cats = known_categories();
    let meta = cats.get(category);

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

    let intro = meta.map(|m| m.intro).unwrap_or("");
    let ifc_anchor = meta.map(|m| m.ifc_anchor).unwrap_or("");
    let elements = meta.map(|m| m.elements).unwrap_or("");

    let mut rows = String::new();
    for (_cat_key, cat_val) in bim {
        if let Some(entities) = cat_val.as_object() {
            let mut slugs: Vec<&String> = entities.keys().collect();
            slugs.sort();
            for slug in slugs {
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
                    r#"<cds-table-row>
  <cds-table-cell><code>{slug}</code></cds-table-cell>
  <cds-table-cell><code>{ifc_class}</code></cds-table-cell>
  <cds-table-cell>{description}</cds-table-cell>
</cds-table-row>"#,
                    slug = esc(slug),
                    ifc_class = esc(ifc_class),
                    description = esc(description),
                ));
            }
        }
    }

    format!(
        r#"<div class="bim-category-page">
  <div class="bim-breadcrumbs">
    <a href="/tokens" data-path="/tokens" class="bim-nav-link">Catalog</a> / <span>{category}</span>
  </div>
  <h1>{display_name}</h1>
  <p class="bim-intro">{intro}</p>
  <p class="bim-ifc-anchor"><strong>IFC anchor:</strong> <code>{ifc_anchor}</code></p>
  <p class="bim-elements">{elements}</p>
  <cds-data-table>
    <cds-table-head>
      <cds-table-header-row>
        <cds-table-header-cell>Token slug</cds-table-header-cell>
        <cds-table-header-cell>IFC class</cds-table-header-cell>
        <cds-table-header-cell>Description</cds-table-header-cell>
      </cds-table-header-row>
    </cds-table-head>
    <cds-table-body>
      {rows}
    </cds-table-body>
  </cds-data-table>
</div>"#,
        category = esc(category),
        display_name = esc(meta.map(|m| m.display_name).unwrap_or(category)),
        intro = esc(intro),
        ifc_anchor = esc(ifc_anchor),
        elements = esc(elements),
        rows = rows,
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

    let mut cards = String::new();
    for (_cat, cat_val) in bim {
        if let Some(entities) = cat_val.as_object() {
            let mut slugs: Vec<&String> = entities.keys().collect();
            slugs.sort();
            for slug in slugs {
                let entity = &entities[slug];
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
        }
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
    let components_dir = state.config.library_dir.join("components");
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
    let html_body = render_markdown(&raw);
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
    let cats = known_categories();
    let mut out = String::new();
    for (slug, _label) in SIDEBAR_ORDER {
        let meta = cats.get(slug);
        let display = meta.map(|m| m.display_name).unwrap_or(slug);
        let desc = meta.map(|m| m.card_desc).unwrap_or("");
        let count = count_entities_in_file(state, slug);
        out.push_str(&format!(
            r#"<a class="bim-category-card bim-nav-link" href="/tokens/{slug}" data-path="/tokens/{slug}">
  <div class="bim-category-card-name">{display}</div>
  <div class="bim-category-card-desc">{desc}</div>
  <div class="bim-category-card-count">{count} entities</div>
</a>"#,
            slug = slug,
            display = esc(display),
            desc = esc(desc),
            count = count,
        ));
    }
    out
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

fn render_markdown(md: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let parser = Parser::new_ext(md, opts);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}
