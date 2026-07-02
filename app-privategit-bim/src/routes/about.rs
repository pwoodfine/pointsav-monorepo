use axum::{extract::State, response::Html};

use crate::{render, state::AppState};

pub async fn about_handler(State(state): State<AppState>) -> Html<String> {
    let content = r#"<div class="bim-breadcrumbs">
  <a href="/" data-path="/" class="bim-nav-link">Home</a>
</div>
<h1>About BIM Objects</h1>
<article class="bim-article">
  <section>
    <h2>What is a BIM Object?</h2>
    <p>A BIM Object is a machine-readable specification unit stored in W3C Design Token
    Community Group (DTCG) format JSON. Each object is anchored to an IFC 4.3 entity class
    and carries three layers of constraint data:</p>
    <ol>
      <li><strong>Specification</strong> &mdash; the IFC entity anchor, applicable Uniclass 2015
      code, bSDD URI, and property set definitions.</li>
      <li><strong>Regulation</strong> &mdash; jurisdictional overlays (building code clauses,
      fire ratings, energy standards) registered against the element type.</li>
      <li><strong>Climate Zone</strong> &mdash; performance requirements that vary by geographic
      zone (ASHRAE, NBC 2020).</li>
    </ol>
  </section>
  <section>
    <h2>Key Plans</h2>
    <p>Key Plans extend the BIM Object model to the spatial program layer. A Key Plan is the
    smallest leasable BIM Object: a bounded IfcSpace defined by real furniture placement, a
    three-zone cross-section (Zone 1 Habitat / Zone 2 Magazine / Zone 3 Corridor), net leasable
    area, and accessibility compliance.</p>
    <p>Key Plans nest into Tiles (climate zone boundaries), which nest into Floor Plates (full
    building-floor programs). The tool-buildingwidth Rust engine computes remainder-free nesting
    in both directions.</p>
  </section>
  <section>
    <h2>Standards</h2>
    <ul>
      <li><strong>IFC 4.3</strong> (ISO 16739-1:2024) &mdash; entity backbone</li>
      <li><strong>Uniclass 2015</strong> &mdash; classification floor</li>
      <li><strong>IDS 1.0</strong> &mdash; regulatory overlay constraint format</li>
      <li><strong>bSDD</strong> (buildingSMART Data Dictionary) &mdash; URI authority</li>
      <li><strong>DTCG</strong> &mdash; W3C Design Token Community Group token format</li>
    </ul>
  </section>
</article>"#;
    Html(render::shell::page_shell(
        "About BIM Objects",
        "/about",
        content,
        &state,
    ))
}
