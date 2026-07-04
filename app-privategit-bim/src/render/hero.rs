//! Homepage hero: "Anatomy of a Key Plan — PO-1".
//!
//! Replaces the 2026-07-03 isometric zoning-envelope diagram, which
//! represented claim #41 (City Code as Composable Geometry) — a real but
//! explicitly v0.0.2+ roadmap idea, not what this catalog does today. This
//! hero shows a real, already-shipped catalog entry instead: PO-1 ("Private
//! Office — Small"), drawn with the same generator already used on
//! `/key-plans` (`render::svg::render_kp_zone_svg_from_value`) — not new
//! invented geometry — and annotated with PO-1's real attached data.
//!
//! Dimensions (5.9944 m / 1.3716 m, no Zone 3) are sourced to the real
//! drafted key-plan sheet `DISCOVERY_MCorp_Sketches_Key Plans_Private
//! Office.pdf` ("PO 1 / 325 SF"), corrected into `key-plans.dtcg.json`
//! 2026-07-04 — the live token had previously inherited the Professional
//! Office use-type's zone depths in error. See BRIEF-app-privategit-bim.md.
//!
//! "Key Plan" (not "Bundle") is the real, established term for a spatial
//! unit (`IfcSpace`) containing BIM Objects — see `content.rs`'s
//! `private-office` category description and the Tiles methodology PDF's
//! own definition.

use crate::state::AppState;
use serde_json::Value;

use super::shell::esc;

fn callout(href: &str, label: &str, fact: &str) -> String {
    format!(
        r#"<a class="bim-hero-callout" href="{href}" data-path="{href}">
  <span class="bim-hero-callout__label">{label}</span>
  <span class="bim-hero-callout__fact">{fact}</span>
</a>"#,
        href = href,
        label = esc(label),
        fact = fact,
    )
}

pub fn render_hero(state: &AppState) -> String {
    let po1 = state
        .tokens
        .get("key-plans")
        .and_then(|f| f.get("bim"))
        .and_then(|b| b.get("key-plan"))
        .and_then(|kp| kp.get("private-office"))
        .and_then(|po| po.get("small"))
        .and_then(|s| s.get("$value"));

    let Some(po1) = po1 else {
        // Should not happen with the shipped token file; degrade to a plain
        // heading rather than panic if key-plans.dtcg.json is ever missing
        // this entry.
        return r#"<h1>Woodfine BIM Object Library</h1>"#.to_string();
    };

    let diagram = super::svg::render_kp_zone_svg_from_value(po1);

    let area_sf = po1.get("area_sf").and_then(Value::as_u64).unwrap_or(325);
    let z1 = po1
        .get("zone1_depth_m")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let z2 = po1
        .get("zone2_depth_m")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let tile_role = po1
        .get("tile_role")
        .and_then(Value::as_str)
        .unwrap_or("nests into a larger Tile");
    let furniture: Vec<&str> = po1
        .get("furniture_program")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let furniture_summary = furniture.first().copied().unwrap_or("desk + task chair");
    let furniture_count = furniture.len();
    let lighting = po1
        .get("compliance")
        .and_then(|c| c.get("european_lighting_standard"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let circulation = po1
        .get("compliance")
        .and_then(|c| c.get("german_circulation_law"))
        .and_then(Value::as_str)
        .unwrap_or("");

    let taxonomy_callout = callout(
        "/tokens#taxonomy",
        "PO-1 · 325 SF · Private Office — Small",
        &format!(
            "IFC anchor <code>IfcSpace</code> &middot; Uniclass <code>SL_25</code> &middot; {area_sf} SF",
            area_sf = area_sf
        ),
    );
    let context_callout = callout(
        "/tokens#context",
        "Zone 1 Habitat — code-derived",
        &format!(
            "{z1:.4} m ({lighting}) &middot; +0.7 m addition for three desks in series ({circulation})",
            z1 = z1,
            lighting = esc(lighting),
            circulation = esc(circulation),
        ),
    );
    let objects_callout = callout(
        "/tokens/furniture",
        "Real BIM Objects placed inside",
        &format!(
            "{furniture_summary} &middot; {n} objects total, each its own IFC entity",
            furniture_summary = esc(furniture_summary),
            n = furniture_count,
        ),
    );
    let compositions_label = format!("Zone 2 Magazine — {z2:.4} m");
    let compositions_callout = callout("/tokens#compositions", &compositions_label, &esc(tile_role));

    format!(
        r#"<div class="bim-hero">
  <div class="bim-hero__diagram">{diagram}</div>
  <div class="bim-hero__panel">
    <p class="bim-hero__eyebrow">Anatomy of a Key Plan</p>
    <h1 class="bim-hero__h1">Building specifications that enforce compliance at placement, not inspection after the fact.</h1>
    <p class="bim-hero__example-label">Anatomy of a Key Plan — <strong>PO-1, Private Office (Small)</strong></p>
    <p class="bim-hero__lead">A Key Plan is a real spatial unit (<code>IfcSpace</code>) containing real BIM Objects, arranged under zone depths that real building codes — not this site — set. Four BIM Objects, arranged by code, contained in one Key Plan. Click any fact below to browse that part of the catalog.</p>
    <div class="bim-hero__callouts">
      {taxonomy_callout}
      {context_callout}
      {objects_callout}
      {compositions_callout}
    </div>
    <p class="bim-hero__credit">Methodology authored by Jennifer M. Woodfine — "Spatial Taxonomy — Key Plan Methodology," V12, January 2025.</p>
  </div>
</div>"#,
        diagram = diagram,
        taxonomy_callout = taxonomy_callout,
        context_callout = context_callout,
        objects_callout = objects_callout,
        compositions_callout = compositions_callout,
    )
}
