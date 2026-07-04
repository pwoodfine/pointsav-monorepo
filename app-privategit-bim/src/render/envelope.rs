//! Envelope-as-Navigation homepage hero.
//!
//! Renders the isometric zoning-envelope diagram (the axonometric
//! setback/height-limit/FAR convention dating to NYC's 1916 zoning
//! resolution) as the site's front door and primary navigation device,
//! replacing the sidebar-tree model. Each constraint tier is a clickable
//! hotspot routed to the catalog Section it best represents:
//!
//! - **Ground / site plane** -> Context: site, climate, and landscape overlays.
//! - **Base footprint** (street-wall height) -> Taxonomy: IFC classification
//!   and Identity/Codes, the baseline every other tier is built on.
//! - **Setback mass** (stepped-back tier) -> Compositions: Building Grid,
//!   Floor Plate, and Tile System — the rules that shape a mass once pulled
//!   back from the lot line.
//! - **Tower** (height-limit + FAR ceiling) -> Objects: Key Plans and other
//!   instantiable BIM Object families — what actually gets built inside the
//!   constrained volume.
//!
//! A jurisdiction-overlay toggle swaps between three pre-rendered envelope
//! sizes (municipal / +provincial / +accessibility), dramatizing "composable
//! geometry" — the envelope visibly shrinks as constraints stack.

// Lot spans x:0..14, y:0..10, tallest tower reaches z:8.5 (municipal
// overlay). With these constants the projected bounding box is
// approximately [20,332] x [20,328] — chosen to fill a 350x350 viewBox
// with ~20px margins on every side, rather than leaving the diagram
// stranded in a much larger empty canvas.
fn iso(x: f64, y: f64, z: f64) -> (f64, f64) {
    const OX: f64 = 150.0;
    const OY: f64 = 148.0;
    const S: f64 = 15.0;
    let sx = OX + (x - y) * 0.866_025_4 * S;
    let sy = OY + (x + y) * 0.5 * S - z * S;
    (sx, sy)
}

fn poly(pts: &[(f64, f64)]) -> String {
    pts.iter()
        .map(|p| format!("{:.1},{:.1}", p.0, p.1))
        .collect::<Vec<_>>()
        .join(" ")
}

struct Block {
    top: String,
    left: String,
    right: String,
}

/// One isometric extruded block over plan rectangle (x0,y0)-(x1,y1),
/// from height z0 to z1. Draws only the three faces visible from this
/// camera angle (top, front-left, front-right).
fn block(x0: f64, y0: f64, x1: f64, y1: f64, z0: f64, z1: f64) -> Block {
    let a = iso(x0, y0, z1);
    let b = iso(x1, y0, z1);
    let c = iso(x1, y1, z1);
    let d = iso(x0, y1, z1);
    let b0 = iso(x1, y0, z0);
    let c0 = iso(x1, y1, z0);
    let d0 = iso(x0, y1, z0);
    Block {
        top: poly(&[a, b, c, d]),
        right: poly(&[b, c, c0, b0]),
        left: poly(&[d, c, c0, d0]),
    }
}

fn hotspot(section_id: &str, title: &str, block: &Block, tier_class: &str) -> String {
    format!(
        r#"<a class="bim-envelope__hotspot bim-envelope__hotspot--{tier_class}" href="/tokens#{section_id}" data-path="/tokens#{section_id}">
  <title>{title}</title>
  <polygon class="bim-envelope__face bim-envelope__face--right" points="{right}"></polygon>
  <polygon class="bim-envelope__face bim-envelope__face--left" points="{left}"></polygon>
  <polygon class="bim-envelope__face bim-envelope__face--top" points="{top}"></polygon>
</a>"#,
        section_id = section_id,
        title = title,
        right = block.right,
        left = block.left,
        top = block.top,
    )
}

struct Overlay {
    key: &'static str,
    label: &'static str,
    setback_margin: f64,
    tower_margin: f64,
    tower_height: f64,
}

const OVERLAYS: [Overlay; 3] = [
    Overlay {
        key: "municipal",
        label: "Municipal",
        setback_margin: 1.5,
        tower_margin: 1.5,
        tower_height: 8.5,
    },
    Overlay {
        key: "provincial",
        label: "+ Provincial",
        setback_margin: 2.2,
        tower_margin: 2.0,
        tower_height: 7.5,
    },
    Overlay {
        key: "accessibility",
        label: "+ Accessibility",
        setback_margin: 2.8,
        tower_margin: 2.6,
        tower_height: 6.5,
    },
];

fn one_diagram(o: &Overlay) -> String {
    // Fixed lot + base footprint across every overlay state — only the
    // setback and tower tiers shrink as jurisdiction layers stack.
    let (lot_x0, lot_y0, lot_x1, lot_y1) = (0.0, 0.0, 14.0, 10.0);
    let (base_x0, base_y0, base_x1, base_y1) = (1.0, 1.0, 13.0, 9.0);
    let base_h = 2.5;

    let sb_x0 = base_x0 + o.setback_margin;
    let sb_y0 = base_y0 + o.setback_margin;
    let sb_x1 = base_x1 - o.setback_margin;
    let sb_y1 = base_y1 - o.setback_margin;
    let sb_h = 5.0;

    let tw_x0 = sb_x0 + o.tower_margin;
    let tw_y0 = sb_y0 + o.tower_margin;
    let tw_x1 = sb_x1 - o.tower_margin;
    let tw_y1 = sb_y1 - o.tower_margin;

    let ground = block(lot_x0, lot_y0, lot_x1, lot_y1, 0.0, 0.0);
    let base = block(base_x0, base_y0, base_x1, base_y1, 0.0, base_h);
    let setback = block(sb_x0, sb_y0, sb_x1, sb_y1, base_h, sb_h);
    let tower = block(tw_x0, tw_y0, tw_x1, tw_y1, sb_h, o.tower_height);

    format!(
        r#"<svg class="bim-envelope__svg" viewBox="0 0 350 350" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Isometric zoning-envelope diagram — click a volume to browse its BIM Object section">
  <a class="bim-envelope__hotspot bim-envelope__hotspot--ground" href="/tokens#context" data-path="/tokens#context">
    <title>Context — site, climate, and landscape overlays</title>
    <polygon class="bim-envelope__face bim-envelope__face--ground" points="{ground_top}"></polygon>
  </a>
  {base_hotspot}
  {setback_hotspot}
  {tower_hotspot}
</svg>"#,
        ground_top = ground.top,
        base_hotspot = hotspot(
            "taxonomy",
            "Taxonomy — base buildable footprint: IFC classification + Identity/Codes",
            &base,
            "base",
        ),
        setback_hotspot = hotspot(
            "compositions",
            "Compositions — setback mass: Building Grid, Floor Plate, Tile System rules",
            &setback,
            "setback",
        ),
        tower_hotspot = hotspot(
            "objects",
            "Objects — tower: Key Plans and other instantiable BIM Object families",
            &tower,
            "tower",
        ),
    )
}

/// Renders the full envelope hero: the three overlay-state diagrams (only
/// one visible at a time, switched client-side, see bim.js) plus the
/// jurisdiction-overlay toggle and a compact legend explaining the mapping.
pub fn render_envelope_hero() -> String {
    let mut frames = String::new();
    for (i, o) in OVERLAYS.iter().enumerate() {
        let hidden = if i == 0 { "" } else { r#" hidden"# };
        frames.push_str(&format!(
            r#"<div class="bim-envelope__frame" data-overlay="{key}"{hidden}>{svg}</div>"#,
            key = o.key,
            hidden = hidden,
            svg = one_diagram(o),
        ));
    }

    let mut toggle_buttons = String::new();
    for (i, o) in OVERLAYS.iter().enumerate() {
        let active = if i == 0 { r#" aria-pressed="true""# } else { r#" aria-pressed="false""# };
        toggle_buttons.push_str(&format!(
            r#"<button type="button" class="bim-envelope__overlay-btn" data-overlay-target="{key}"{active}>{label}</button>"#,
            key = o.key,
            active = active,
            label = o.label,
        ));
    }

    format!(
        r#"<div class="bim-envelope" data-active-overlay="municipal">
  <div class="bim-envelope__diagram">
    {frames}
  </div>
  <div class="bim-envelope__panel">
    <p class="bim-envelope__eyebrow">Woodfine BIM Object Library</p>
    <p class="bim-envelope__statline">Building specifications that enforce compliance at placement,<br>not inspection after the fact.</p>
    <p class="bim-envelope__lead">Every volume in this diagram is a real section of the catalog — click a tier to browse it. City codes compose directly into the buildable envelope: click the overlay toggle below to watch it shrink as jurisdiction constraints stack. A non-compliant design becomes geometrically impossible to assemble, not something caught in review afterward.</p>
    <div class="bim-envelope__overlay-toggle" role="group" aria-label="Jurisdiction overlay">
      {toggle_buttons}
    </div>
    <ul class="bim-envelope__legend">
      <li><span class="bim-envelope__legend-swatch bim-envelope__legend-swatch--ground"></span> Site &amp; context</li>
      <li><span class="bim-envelope__legend-swatch bim-envelope__legend-swatch--base"></span> Taxonomy — classification &amp; codes</li>
      <li><span class="bim-envelope__legend-swatch bim-envelope__legend-swatch--setback"></span> Compositions — assembly rules</li>
      <li><span class="bim-envelope__legend-swatch bim-envelope__legend-swatch--tower"></span> Objects — placeable BIM Objects</li>
    </ul>
  </div>
</div>"#,
        frames = frames,
        toggle_buttons = toggle_buttons,
    )
}
