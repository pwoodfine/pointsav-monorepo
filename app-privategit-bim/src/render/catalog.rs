// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Unified home page (`/`) — a two-tab catalog over the real library:
//!
//! * **Objects** — atomic building-component specifications, walked from
//!   `interior.dtcg.json`'s furniture BIM Objects. Each carries its verified
//!   IFC 4.3 entity class and Uniclass 2015 **Pr** (Products) code.
//! * **Compositions** — Key Plans, walked from `key-plans.dtcg.json` with the
//!   shared `card::collect_kp_leaves` helper and drawn with the same
//!   `svg::render_kp_zone_svg_from_value` generator the old `/key-plans` page
//!   used. Classified one tier up, at Uniclass **SL** (Spaces/locations) level.
//!
//! The page is server-rendered in full: both tab grids, both facet sets, and
//! the result counters are present in the initial HTML so the catalog is
//! legible with JavaScript disabled. `bim-catalog.js` then layers on tab
//! switching, faceted filtering, and the detail modal. The same normalized
//! catalog is exposed to the client (for modal population) through the
//! extended `/api/tokens.json` endpoint under the `_catalog` key — no new
//! route is introduced.
//!
//! Honest-partial-completion convention (matching the corporate-office SVG
//! gap and the `/tokens` "—" cells): only PO-1 carries a real, structured
//! `furniture_refs` array, so only PO-1's "Composed from" bill links to its
//! constituent Objects. Every other Composition renders its prose
//! `furniture_program` with an explicit "structured object linking pending"
//! note rather than a fabricated bill.

use crate::state::AppState;
use serde_json::{json, Map, Value};

use super::card::collect_kp_leaves;
use super::shell::esc;
use super::svg::render_kp_zone_svg_from_value;

// Category display order + labels shared by cards, facets, and the catalog
// payload. Space labels are the Uniclass 2015 SL (Spaces/locations) *framing*
// per key-plans.dtcg.json's own `$description` ("Classified at Uniclass 2015
// SL level") — a descriptive space-type, deliberately not a fabricated
// numeric code (the Objects tab's Pr codes are the real, verified ones).
const CATEGORY_ORDER: &[&str] = &[
    "private-office",
    "corporate-office",
    "medical",
    "business",
    "laboratory",
    "academic",
    "civic",
];

fn category_label(cat: &str) -> &'static str {
    match cat {
        "private-office" => "Private Office",
        "corporate-office" => "Corporate Office",
        "medical" => "Medical",
        "business" => "Business",
        "laboratory" => "Laboratory",
        "academic" => "Academic",
        "civic" => "Civic",
        _ => "Other",
    }
}

fn category_space(cat: &str) -> &'static str {
    match cat {
        "private-office" => "Private office spaces",
        "corporate-office" => "Open-plan office areas",
        "medical" => "Health & care spaces",
        "business" => "General office spaces",
        "laboratory" => "Laboratory spaces",
        "academic" => "Teaching & learning spaces",
        "civic" => "Civic & community spaces",
        _ => "Office spaces",
    }
}

fn category_rank(cat: &str) -> usize {
    CATEGORY_ORDER
        .iter()
        .position(|c| *c == cat)
        .unwrap_or(CATEGORY_ORDER.len())
}

// ── small value helpers ─────────────────────────────────────────────────────

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn int_of(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(Value::as_i64)
}

fn f_of(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn str_vec(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Round to at most two decimals, trailing zeros trimmed (5.9944 → "5.99").
fn round2(n: f64) -> String {
    let r = (n * 100.0).round() / 100.0;
    if r.fract().abs() < 1e-9 {
        format!("{}", r as i64)
    } else {
        let mut out = format!("{r:.2}");
        while out.ends_with('0') {
            out.pop();
        }
        if out.ends_with('.') {
            out.pop();
        }
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Objects — furniture BIM Objects from interior.dtcg.json
// ─────────────────────────────────────────────────────────────────────────────

/// Set of `.ifc` filenames actually present in `blocks/furniture/`. Used to
/// decide whether an Object gets a download link — never fabricate a filename.
fn ifc_file_set(state: &AppState) -> std::collections::HashSet<String> {
    let dir = state.config.library_dir.join("blocks").join("furniture");
    let mut set = std::collections::HashSet::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("ifc") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    set.insert(name.to_string());
                }
            }
        }
    }
    set
}

fn dims_summary(dims: &Value) -> String {
    let w = int_of(dims, "w");
    let d = int_of(dims, "d");
    let hmin = int_of(dims, "h_min");
    let hmax = int_of(dims, "h_max");
    match (w, d) {
        (Some(w), Some(d)) => {
            let h = match (hmin, hmax) {
                (Some(a), Some(b)) if a == b => format!(" × {a}"),
                (Some(a), Some(b)) => format!(" × {a}–{b}"),
                _ => String::new(),
            };
            format!("{w} × {d}{h} mm")
        }
        _ => "—".to_string(),
    }
}

fn build_objects(state: &AppState) -> Vec<Value> {
    let ifc_files = ifc_file_set(state);
    let mut out: Vec<Value> = Vec::new();

    let Some(furniture) = state
        .tokens
        .get("interior")
        .and_then(|f| f.get("bim"))
        .and_then(|b| b.get("interior"))
        .and_then(|i| i.get("furniture"))
        .and_then(Value::as_object)
    else {
        return out;
    };

    for (group, slugs) in furniture {
        let Some(slugs) = slugs.as_object() else {
            continue;
        };
        for (slug, entity) in slugs {
            let Some(val) = entity.get("$value") else {
                continue;
            };
            let name = {
                let m = s(val, "model");
                if m.is_empty() {
                    slug.replace('-', " ")
                } else {
                    m.to_string()
                }
            };
            let ifc_class = s(val, "ifc_class");
            let uni_pr = s(val, "uniclass_pr");
            let uni_pr_title = s(val, "uniclass_pr_title");
            let manufacturer = s(val, "manufacturer");
            let dims = val.get("dimensions_mm").cloned().unwrap_or(Value::Null);
            let dim_summary = if dims.is_null() {
                "—".to_string()
            } else {
                dims_summary(&dims)
            };
            let description = entity
                .get("$description")
                .and_then(Value::as_str)
                .unwrap_or("");

            let expected_ifc = format!("{group}-{slug}.ifc");
            let ifc_file = if ifc_files.contains(&expected_ifc) {
                Value::String(expected_ifc)
            } else {
                Value::Null
            };

            // Full spec rows for the detail modal.
            let mut spec: Vec<Value> = Vec::new();
            let mut row = |k: &str, v: String| {
                if !v.is_empty() {
                    spec.push(json!([k, v]));
                }
            };
            row("Manufacturer", s(val, "manufacturer").to_string());
            row("Product line", s(val, "product_line").to_string());
            row("Model", s(val, "model").to_string());
            row("SKU", s(val, "sku").to_string());
            row("Designer", s(val, "designer").to_string());
            if !dims.is_null() {
                row("Dimensions (W × D × H)", dims_summary(&dims));
            }
            if let Some(dia) = int_of(val, "diameter_mm") {
                row("Diameter", format!("⌀ {dia} mm"));
            }
            if let Some(sh) = val.get("seat_height_mm") {
                let (a, b) = (int_of(sh, "min"), int_of(sh, "max"));
                if let (Some(a), Some(b)) = (a, b) {
                    let v = if a == b {
                        format!("{a} mm")
                    } else {
                        format!("{a}–{b} mm")
                    };
                    row("Seat height", v);
                }
            }
            if let Some(cl) = val.get("clearance_mm") {
                let f = int_of(cl, "front").unwrap_or(0);
                let si = int_of(cl, "sides").unwrap_or(0);
                let r = int_of(cl, "rear").unwrap_or(0);
                row(
                    "Clearance (front / sides / rear)",
                    format!("{f} / {si} / {r} mm"),
                );
            }
            let weight = match f_of(val, "weight_kg") {
                Some(w) => format!("{} kg", round2(w)),
                None => s(val, "weight_note").to_string(),
            };
            row("Weight", weight);
            row("IFC 4.3 entity class", ifc_class.to_string());
            if !uni_pr.is_empty() {
                row("Uniclass 2015 (Pr)", format!("{uni_pr} — {uni_pr_title}"));
            }

            let search = format!(
                "{} {} {} {} {} {}",
                name, manufacturer, ifc_class, uni_pr, uni_pr_title, group
            )
            .to_lowercase();

            let mut e = Map::new();
            e.insert("id".into(), json!(slug));
            e.insert("kind".into(), json!("object"));
            e.insert("group".into(), json!(group));
            e.insert(
                "ref".into(),
                json!(format!("bim.interior.furniture.{group}.{slug}")),
            );
            e.insert("name".into(), json!(name));
            e.insert("manufacturer".into(), json!(manufacturer));
            e.insert("ifc_class".into(), json!(ifc_class));
            e.insert("uniclass_pr".into(), json!(uni_pr));
            e.insert("uniclass_pr_title".into(), json!(uni_pr_title));
            e.insert("dims".into(), json!(dim_summary));
            e.insert("ifc_file".into(), ifc_file);
            e.insert("url".into(), val.get("url").cloned().unwrap_or(Value::Null));
            e.insert("description".into(), json!(description));
            e.insert("spec".into(), Value::Array(spec));
            e.insert("search".into(), json!(search));
            out.push(Value::Object(e));
        }
    }

    out.sort_by(|a, b| {
        s(a, "group")
            .cmp(s(b, "group"))
            .then_with(|| s(a, "name").cmp(s(b, "name")))
    });
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Compositions — Key Plans from key-plans.dtcg.json
// ─────────────────────────────────────────────────────────────────────────────

fn build_compositions(state: &AppState, objects: &[Value]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();

    let Some(bim) = state
        .tokens
        .get("key-plans")
        .and_then(|f| f.get("bim"))
        .and_then(Value::as_object)
    else {
        return out;
    };

    // Reuse the shared leaf-walker: any node carrying a `$value`, at any depth.
    let mut leaves: Vec<(&str, &Value)> = Vec::new();
    collect_kp_leaves(bim, &mut leaves);

    for (_slug, entity) in leaves {
        let Some(val) = entity.get("$value") else {
            continue;
        };
        let internal_code = {
            let c = s(val, "internal_code");
            if c.is_empty() {
                s(val, "display_name").to_string()
            } else {
                c.to_string()
            }
        };
        let display_name = s(val, "display_name");
        let category = s(val, "category");
        let cat_label = category_label(category);
        let space = category_space(category);
        let area_m2 = f_of(val, "area_m2");
        let area_sf = int_of(val, "area_sf");
        let z1 = f_of(val, "zone1_depth_m");
        let z2 = f_of(val, "zone2_depth_m");
        let z3 = f_of(val, "zone3_depth_m");
        let has_zone_data = z1.is_some();
        let svg = render_kp_zone_svg_from_value(val);
        let description = entity
            .get("$description")
            .and_then(Value::as_str)
            .unwrap_or("");

        let furniture_program = str_vec(val, "furniture_program");
        let development_classes = str_vec(val, "development_classes");
        let key_rooms = str_vec(val, "key_rooms");

        // Bill of objects. Only PO-1 carries a real `furniture_refs` array;
        // resolve each against the Objects list. Everything else falls back to
        // the prose program with an explicit "linking pending" flag.
        let refs = val.get("furniture_refs").and_then(Value::as_array);
        let (bill, refs_pending) = if let Some(refs) = refs {
            let mut items: Vec<Value> = Vec::new();
            for r in refs {
                let Some(rstr) = r.as_str() else { continue };
                let matched = objects.iter().find(|o| s(o, "ref") == rstr);
                if let Some(o) = matched {
                    items.push(json!({
                        "linked": true,
                        "name": s(o, "name"),
                        "code": s(o, "uniclass_pr"),
                        "obj_id": s(o, "id"),
                    }));
                } else {
                    items.push(json!({ "linked": false, "name": rstr }));
                }
            }
            (Value::Array(items), false)
        } else {
            let items: Vec<Value> = furniture_program
                .iter()
                .map(|line| json!({ "linked": false, "name": line }))
                .collect();
            (Value::Array(items), true)
        };

        // Spec rows for the modal.
        let mut spec: Vec<Value> = Vec::new();
        let mut row = |k: &str, v: String| {
            if !v.is_empty() {
                spec.push(json!([k, v]));
            }
        };
        row("Internal code", internal_code.clone());
        row("Category", cat_label.to_string());
        match (area_m2, area_sf) {
            (Some(m), Some(sf)) => row("Net leasable area", format!("{} m² · {sf} SF", round2(m))),
            (Some(m), None) => row("Net leasable area", format!("{} m²", round2(m))),
            (None, Some(sf)) => row("Net leasable area", format!("{sf} SF")),
            _ => {}
        }
        if let Some(z) = z1 {
            row("Zone 1 (Habitat) depth", format!("{} m", round2(z)));
        }
        if let Some(z) = z2 {
            row("Zone 2 (Magazine) depth", format!("{} m", round2(z)));
        }
        if let Some(z) = z3 {
            row("Zone 3 (Corridor) depth", format!("{} m", round2(z)));
        }
        if let Some(fr) = f_of(val, "facade_frontage_m") {
            row("Facade frontage", format!("{} m", round2(fr)));
        }
        if let Some(o) = int_of(val, "occupancy_persons") {
            row("Occupancy", format!("{o} persons"));
        } else if let (Some(a), Some(b)) = (
            int_of(val, "occupancy_persons_min"),
            int_of(val, "occupancy_persons_max"),
        ) {
            row("Occupancy", format!("{a}–{b} persons"));
        }
        if let Some(bc) = int_of(val, "bench_count") {
            row("Benches", format!("{bc}"));
        }
        if let Some(ec) = int_of(val, "exam_chairs") {
            row("Exam / treatment chairs", format!("{ec}"));
        }
        row("Tile role", s(val, "tile_role").to_string());
        if !development_classes.is_empty() {
            row("Development classes", development_classes.join(", "));
        }
        row("Uniclass 2015 (SL)", format!("SL — {space}"));

        let search =
            format!("{display_name} {internal_code} {category} {cat_label}").to_lowercase();

        let mut e = Map::new();
        e.insert("id".into(), json!(internal_code));
        e.insert("kind".into(), json!("composition"));
        e.insert("name".into(), json!(display_name));
        e.insert("category".into(), json!(category));
        e.insert("category_label".into(), json!(cat_label));
        e.insert("area_m2".into(), json!(area_m2.map(round2)));
        e.insert("area_sf".into(), json!(area_sf));
        e.insert("has_zone_data".into(), json!(has_zone_data));
        e.insert("zone1".into(), json!(z1.map(round2)));
        e.insert("zone2".into(), json!(z2.map(round2)));
        e.insert("zone3".into(), json!(z3.map(round2)));
        e.insert("uniclass_level".into(), json!("SL"));
        e.insert("uniclass_space".into(), json!(space));
        e.insert("refs_pending".into(), json!(refs_pending));
        e.insert("bill".into(), bill);
        e.insert("furniture_program".into(), json!(furniture_program));
        e.insert("development_classes".into(), json!(development_classes));
        e.insert("key_rooms".into(), json!(key_rooms));
        e.insert(
            "tile_role".into(),
            val.get("tile_role").cloned().unwrap_or(Value::Null),
        );
        e.insert(
            "design_notes".into(),
            val.get("design_notes").cloned().unwrap_or(Value::Null),
        );
        e.insert(
            "compliance".into(),
            val.get("compliance").cloned().unwrap_or(Value::Null),
        );
        e.insert("description".into(), json!(description));
        e.insert("svg".into(), json!(svg));
        e.insert("spec".into(), Value::Array(spec));
        e.insert("search".into(), json!(search));
        out.push(Value::Object(e));
    }

    out.sort_by(|a, b| {
        category_rank(s(a, "category"))
            .cmp(&category_rank(s(b, "category")))
            .then_with(|| {
                int_of(a, "area_sf")
                    .unwrap_or(0)
                    .cmp(&int_of(b, "area_sf").unwrap_or(0))
            })
    });
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Public: normalized catalog for the `/api/tokens.json` extension
// ─────────────────────────────────────────────────────────────────────────────

/// Normalized `{ objects: [...], compositions: [...] }` catalog. Consumed by
/// `bim-catalog.js` (via the `_catalog` key added to `/api/tokens.json`) to
/// populate the detail modal without a full page reload.
pub fn build_catalog(state: &AppState) -> Value {
    let objects = build_objects(state);
    let compositions = build_compositions(state, &objects);
    json!({
        "objects": objects,
        "compositions": compositions,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// SSR — the full home page
// ─────────────────────────────────────────────────────────────────────────────

pub fn render_home(state: &AppState) -> String {
    let objects = build_objects(state);
    let compositions = build_compositions(state, &objects);
    let obj_n = objects.len();
    let comp_n = compositions.len();

    let object_cards: String = objects.iter().map(render_object_card).collect();
    let composition_cards: String = compositions.iter().map(render_composition_card).collect();

    let object_facets = render_object_facets(&objects);
    let composition_facets = render_composition_facets(&compositions);

    format!(
        r##"<div class="bim-catalog-home" id="bim-catalog">
  <section class="bim-cat-hero">
    <div class="bim-cat-hero__main">
      <span class="bim-cat-hero__eyebrow">Building-component registry <b>· open standards, versioned</b></span>
      <h1 class="bim-cat-hero__title">One authoritative record for every building component — and the Key&nbsp;Plans composed from them.</h1>
      <p class="bim-cat-hero__lede">The Woodfine BIM Object Library is the registry of record for every atomic building component in the portfolio, and for the <strong>Key&nbsp;Plan Compositions</strong> our architects assemble from them. Every entry is classified, versioned, and inspectable, and is built on <strong>open standards — IFC&nbsp;4.3 and DTCG tokens</strong>.</p>
      <div class="bim-cat-hero__sites">
        <span>Professional Centre</span>
        <span>Suburban Office</span>
        <span>Retail Select</span>
        <span>Tech Industrial</span>
      </div>
    </div>
    <aside class="bim-cat-hero__panel" aria-label="Registry summary">
      <div class="bim-cat-hero__panel-h"><span>Registry — current release</span></div>
      <div class="bim-cat-hero__stats">
        <div class="bim-cat-hstat"><div class="bim-cat-hstat__n">{obj_n}</div><div class="bim-cat-hstat__l">Atomic BIM Objects, classified &amp; versioned</div></div>
        <div class="bim-cat-hstat"><div class="bim-cat-hstat__n">{comp_n}</div><div class="bim-cat-hstat__l">Key&nbsp;Plan Compositions across 7 categories</div></div>
        <div class="bim-cat-hstat"><div class="bim-cat-hstat__n">4</div><div class="bim-cat-hstat__l">Development site-types under one model</div></div>
        <div class="bim-cat-hstat"><div class="bim-cat-hstat__n">IFC<small> 4.3</small></div><div class="bim-cat-hstat__l">Uniclass 2015 classified · DTCG tokens</div></div>
      </div>
    </aside>
  </section>

  <section class="bim-cat-def">
    <div class="bim-cat-sechead">
      <!-- "Taxonomy — Anatomy — Syntax" per the architects' own working
           framing (DISCOVERY_MCorp_Sketches_Key Plans_Business_Notes.pdf,
           "Sketch 1: Spatial Taxonomy — Anatomy — Syntax") — taxonomy
           classifies (IFC/Uniclass), anatomy is the atom (the Object),
           syntax is how atoms compose (the Composition). -->
      <span class="bim-cat-kicker">Taxonomy &middot; Anatomy &middot; Syntax</span>
      <h2>An Object is an atom. A Composition is what you build from atoms.</h2>
      <p>The distinction runs through the whole library: <strong>taxonomy</strong> classifies every entry (IFC&nbsp;4.3, Uniclass&nbsp;2015); <strong>anatomy</strong> is the atom itself — the BIM Object; <strong>syntax</strong> is how objects compose into a Key&nbsp;Plan. Held precisely, every downstream party — architect, property manager, tenant — reads the same specification the same way.</p>
    </div>
    <div class="bim-cat-def-grid">
      <article class="bim-cat-defcard">
        <div class="bim-cat-defcard__tag">BIM Object — the atom</div>
        <h3>A single, atomic building-component specification.</h3>
        <p>One product or entity — a task chair, a desk, a storage pedestal — fixed by its <code>IFC&nbsp;4.3</code> entity class and its <code>Uniclass&nbsp;2015</code> classification at the <strong>Products&nbsp;(Pr)</strong> or <strong>Systems&nbsp;(Ss)</strong> level. It carries manufacturer data and the clearances that apply to it. A BIM Object is never a composed layout; it is the atom a Key&nbsp;Plan is assembled from.</p>
        <div class="bim-cat-defcard__foot">Detail view → manufacturer spec · classification · IFC download</div>
      </article>
      <article class="bim-cat-defcard">
        <div class="bim-cat-defcard__tag">Composition — the assembly</div>
        <h3>A Key&nbsp;Plan template, assembled from many objects.</h3>
        <p>A named spatial program built from several BIM Objects to satisfy a design methodology and the applicable building code — a three-zone cross-section (Zone&nbsp;1 Habitat / Zone&nbsp;2 Magazine / Zone&nbsp;3 Corridor), net leasable area, and accessibility compliance. Classified one tier up, at Uniclass <strong>Elements/functions&nbsp;(EF)</strong> or <strong>Spaces/locations&nbsp;(SL)</strong> level — not a BIM Object itself.</p>
        <div class="bim-cat-defcard__foot">Detail view → composed-from bill · zone allocation · computed area</div>
      </article>
    </div>
  </section>

  <section class="bim-cat-catalog">
    <div class="bim-cat-sechead">
      <span class="bim-cat-kicker">The library</span>
      <h2>Browse the registry.</h2>
    </div>
    <div class="bim-cat-tabbar" role="tablist" aria-label="Catalog type">
      <button class="bim-cat-tab" role="tab" id="bim-tab-objects" data-tab="objects" aria-selected="true" aria-controls="bim-panel-objects">Objects <span class="bim-cat-count">{obj_n}</span></button>
      <button class="bim-cat-tab" role="tab" id="bim-tab-compositions" data-tab="compositions" aria-selected="false" aria-controls="bim-panel-compositions">Compositions <span class="bim-cat-count">{comp_n}</span></button>
    </div>
    <div class="bim-cat-body">
      <aside class="bim-cat-facets">
        <label class="bim-cat-search">
          <span class="bim-cat-search__ico" aria-hidden="true">⌕</span>
          <input type="search" id="bim-cat-search" placeholder="Search name, code, classification…" aria-label="Search catalog">
        </label>
        <div class="bim-cat-facetset" data-tab="objects">{object_facets}</div>
        <div class="bim-cat-facetset" data-tab="compositions" hidden>{composition_facets}</div>
        <button class="bim-cat-reset" id="bim-cat-reset" hidden>Clear all filters</button>
      </aside>
      <div class="bim-cat-gridregion">
        <div class="bim-cat-gridhead">
          <div class="bim-cat-res"><b id="bim-cat-rescount">{obj_n}</b> <span id="bim-cat-resnoun">objects</span></div>
          <div class="bim-cat-hint">Select any card for the full specification.</div>
        </div>
        <div class="bim-cat-grid" role="tabpanel" id="bim-panel-objects" aria-labelledby="bim-tab-objects">{object_cards}</div>
        <div class="bim-cat-grid" role="tabpanel" id="bim-panel-compositions" aria-labelledby="bim-tab-compositions" hidden>{composition_cards}</div>
      </div>
    </div>
  </section>
</div>

<div class="bim-cat-modal" id="bim-cat-modal" aria-hidden="true">
  <div class="bim-cat-modal__bk" data-cat-close></div>
  <div class="bim-cat-modal__panel" role="dialog" aria-modal="true" aria-labelledby="bim-cat-modal-title" tabindex="-1" id="bim-cat-modal-panel">
    <div class="bim-cat-modal__head" id="bim-cat-modal-head"></div>
    <button class="bim-cat-modal__close" data-cat-close aria-label="Close detail panel">✕</button>
    <div class="bim-cat-modal__body" id="bim-cat-modal-body"></div>
  </div>
</div>

<script type="module" src="/static/bim-catalog.js"></script>"##,
        obj_n = obj_n,
        comp_n = comp_n,
        object_facets = object_facets,
        composition_facets = composition_facets,
        object_cards = object_cards,
        composition_cards = composition_cards,
    )
}

fn render_object_card(o: &Value) -> String {
    let id = s(o, "id");
    let name = s(o, "name");
    let group = s(o, "group");
    let manufacturer = s(o, "manufacturer");
    let ifc_class = s(o, "ifc_class");
    let uni_pr = s(o, "uniclass_pr");
    let dims = s(o, "dims");
    let search = s(o, "search");
    let uni_title = s(o, "uniclass_pr_title");
    let ifc_badge = if o.get("ifc_file").map(Value::is_string).unwrap_or(false) {
        r#"<span class="bim-cat-thumb__fmt">IFC</span>"#
    } else {
        ""
    };
    let glyph = group
        .chars()
        .next()
        .unwrap_or('•')
        .to_uppercase()
        .to_string();

    format!(
        r#"<button class="bim-cat-card" data-kind="object" data-id="{id}" data-mfr="{mfr}" data-uni="{uni_title}" data-search="{search}" aria-label="{name} — view specification">
  <span class="bim-cat-thumb bim-cat-thumb--obj">
    <span class="bim-cat-thumb__glyph" aria-hidden="true">{glyph}</span>
    {ifc_badge}
  </span>
  <span class="bim-cat-card__body">
    <span class="bim-cat-chip bim-cat-chip--pr"><span class="bim-cat-chip__lv">Pr</span>{uni_pr}</span>
    <span class="bim-cat-card__name">{name}</span>
    <span class="bim-cat-card__meta"><span class="bim-cat-card__em">{mfr}</span> · {ifc_class}</span>
    <span class="bim-cat-card__dims">{dims}</span>
  </span>
</button>"#,
        id = esc(id),
        name = esc(name),
        mfr = esc(manufacturer),
        uni_title = esc(uni_title),
        uni_pr = esc(uni_pr),
        ifc_class = esc(ifc_class),
        dims = esc(dims),
        search = esc(search),
        glyph = esc(&glyph),
        ifc_badge = ifc_badge,
    )
}

fn render_composition_card(c: &Value) -> String {
    let id = s(c, "id");
    let name = s(c, "name");
    let category = s(c, "category");
    let cat_label = s(c, "category_label");
    let space = s(c, "uniclass_space");
    let search = s(c, "search");
    let svg = s(c, "svg");
    let has_zone = c
        .get("has_zone_data")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let layout = if has_zone { "modeled" } else { "floor" };
    let area_sf = int_of(c, "area_sf");
    let area_line = match area_sf {
        Some(sf) => format!("{sf} SF"),
        None => "—".to_string(),
    };
    let note = if has_zone {
        String::new()
    } else {
        r#"<span class="bim-cat-card__note">Floor-scale — no zone layout modeled</span>"#
            .to_string()
    };

    format!(
        r#"<button class="bim-cat-card bim-cat-card--comp" data-kind="composition" data-id="{id}" data-cat="{category}" data-layout="{layout}" data-search="{search}" aria-label="{name} — view specification">
  <span class="bim-cat-thumb bim-cat-thumb--comp" data-category="{category}">{svg}</span>
  <span class="bim-cat-card__body">
    <span class="bim-cat-chip bim-cat-chip--ef"><span class="bim-cat-chip__lv">SL</span>{space}</span>
    <span class="bim-cat-card__name">{name}</span>
    <span class="bim-cat-card__meta"><span class="bim-cat-card__em">{cat_label}</span> · {area_line}</span>
    {note}
  </span>
</button>"#,
        id = esc(id),
        name = esc(name),
        category = esc(category),
        cat_label = esc(cat_label),
        space = esc(space),
        area_line = esc(&area_line),
        search = esc(search),
        svg = svg,
        layout = layout,
        note = note,
    )
}

// ── facets (server-rendered) ─────────────────────────────────────────────────

/// Ordered `(value, count)` list preserving first-seen or supplied order.
fn counted(values: impl Iterator<Item = String>) -> Vec<(String, usize)> {
    let mut order: Vec<String> = Vec::new();
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for v in values {
        if v.is_empty() {
            continue;
        }
        if !counts.contains_key(&v) {
            order.push(v.clone());
        }
        *counts.entry(v).or_insert(0) += 1;
    }
    order.sort();
    order
        .into_iter()
        .map(|v| {
            let c = counts[&v];
            (v, c)
        })
        .collect()
}

fn render_facet_group(title: &str, key: &str, items: &[(String, usize)]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let rows: String = items
        .iter()
        .map(|(val, count)| {
            format!(
                r#"<label class="bim-cat-facet">
  <input type="checkbox" data-facet="{key}" value="{val}">
  <span class="bim-cat-facet__box" aria-hidden="true"></span>
  <span class="bim-cat-facet__lbl">{val}</span>
  <span class="bim-cat-facet__n">{count}</span>
</label>"#,
                key = key,
                val = esc(val),
                count = count,
            )
        })
        .collect();
    format!(
        r#"<div class="bim-cat-fgrp"><div class="bim-cat-fgrp__h">{title}</div>{rows}</div>"#,
        title = esc(title),
        rows = rows,
    )
}

fn render_object_facets(objects: &[Value]) -> String {
    let uni = counted(
        objects
            .iter()
            .map(|o| s(o, "uniclass_pr_title").to_string()),
    );
    let mfr = counted(objects.iter().map(|o| s(o, "manufacturer").to_string()));
    format!(
        "{}{}",
        render_facet_group("Uniclass Pr — product type", "uni", &uni),
        render_facet_group("Manufacturer", "mfr", &mfr),
    )
}

fn render_composition_facets(comps: &[Value]) -> String {
    // Category facet filters by slug (matches card `data-cat`) but shows the
    // human label, ordered by CATEGORY_ORDER.
    let cat_rows: Vec<(String, usize)> = {
        let mut order: Vec<&str> = Vec::new();
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for c in comps {
            let slug = s(c, "category");
            if slug.is_empty() {
                continue;
            }
            if !counts.contains_key(slug) {
                order.push(slug);
            }
            *counts.entry(slug).or_insert(0) += 1;
        }
        order.sort_by_key(|slug| category_rank(slug));
        order
            .into_iter()
            .map(|slug| (slug.to_string(), counts[slug]))
            .collect()
    };
    let cat_group = if cat_rows.is_empty() {
        String::new()
    } else {
        let rows: String = cat_rows
            .iter()
            .map(|(slug, count)| {
                format!(
                    r#"<label class="bim-cat-facet">
  <input type="checkbox" data-facet="cat" value="{slug}">
  <span class="bim-cat-facet__box" aria-hidden="true"></span>
  <span class="bim-cat-facet__lbl">{label}</span>
  <span class="bim-cat-facet__n">{count}</span>
</label>"#,
                    slug = esc(slug),
                    label = esc(category_label(slug)),
                    count = count,
                )
            })
            .collect();
        // "Use Case" (not "Category") per the architects' own working
        // vocabulary — DISCOVERY_MCorp_Sketches_Key Plans_Business_Notes.pdf
        // ("Each of the Key Plans for Small, Medium, and Large for all of the
        // Use Cases we are examining — Business, Academic, Laboratory,
        // Medical, and Civic").
        format!(
            r#"<div class="bim-cat-fgrp"><div class="bim-cat-fgrp__h">Use Case</div>{rows}</div>"#,
            rows = rows,
        )
    };

    let layout_items = {
        let modeled = comps
            .iter()
            .filter(|c| {
                c.get("has_zone_data")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .count();
        let floor = comps.len() - modeled;
        let mut v: Vec<(String, String, usize)> = Vec::new();
        if modeled > 0 {
            v.push(("modeled".into(), "Zone layout modeled".into(), modeled));
        }
        if floor > 0 {
            v.push(("floor".into(), "Floor-scale".into(), floor));
        }
        v
    };
    let layout_group = if layout_items.is_empty() {
        String::new()
    } else {
        let rows: String = layout_items
            .iter()
            .map(|(val, label, count)| {
                format!(
                    r#"<label class="bim-cat-facet">
  <input type="checkbox" data-facet="layout" value="{val}">
  <span class="bim-cat-facet__box" aria-hidden="true"></span>
  <span class="bim-cat-facet__lbl">{label}</span>
  <span class="bim-cat-facet__n">{count}</span>
</label>"#,
                    val = esc(val),
                    label = esc(label),
                    count = count,
                )
            })
            .collect();
        format!(
            r#"<div class="bim-cat-fgrp"><div class="bim-cat-fgrp__h">Layout</div>{rows}</div>"#,
            rows = rows,
        )
    };

    format!("{cat_group}{layout_group}")
}
