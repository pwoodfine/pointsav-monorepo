// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use crate::{
    component_meta, component_preview,
    i18n::{Lang, PageLang},
    render, schema,
    state::AppState,
    tokens_gallery, vault,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Serialize;
use std::{fs, io::Write};

pub async fn index(State(state): State<AppState>) -> Html<String> {
    render_index(&state, Lang::En).await
}

/// GET /es — Spanish homepage. First-pass Spanish support (2026-07-12): homepage +
/// chrome only, per operator scope decision — the ~84 deep vault content pages aren't
/// translated yet (see i18n.rs's module doc for the reasoning).
pub async fn index_es(State(state): State<AppState>) -> Html<String> {
    render_index(&state, Lang::Es).await
}

async fn render_index(state: &AppState, lang: Lang) -> Html<String> {
    let nav_html = render::render_nav(&state.env, &state.component_groups, "", "");

    // Part C (2026-07-04): the CTA no longer renders as a grid card — a real button
    // next to the lede reads as an action, not a wayfinding tile; the grid below is now
    // a uniform set of true-peer sections instead of one card carrying different weight.
    let mut cards = String::new();
    let mut item_total = 0usize;
    for (section, _, _) in vault::SECTIONS {
        let Some(slugs) = state.nav.get(*section) else {
            continue;
        };
        if slugs.is_empty() {
            continue;
        }
        item_total += slugs.len();
        let first = &slugs[0];
        let tab = vault::default_tab(section);
        let n = slugs.len();
        let noun = match (lang, n) {
            (Lang::Es, 1) => "elemento",
            (Lang::Es, _) => "elementos",
            (Lang::En, 1) => "item",
            (Lang::En, _) => "items",
        };
        cards.push_str(&format!(
            "<a class=\"home-card\" href=\"/{section}/{first}/{tab}\">\
             <h2>{}</h2><p>{n} {noun}</p></a>\n",
            crate::i18n::section_label(lang, section),
        ));
    }

    let tiers = tokens_gallery::load_and_flatten(&state.vault);
    let token_count: usize = tiers
        .iter()
        .flat_map(|tier| &tier.groups)
        .map(|group| group.entries.len())
        .sum();

    // Phase 5 — a live palette strip, not decoration: real swatches pulled from the same
    // primitive.color group the /tokens gallery renders, not invented graphics. Picks one
    // representative mid-tone from each named color family, in a fixed, meaningful order.
    let palette_paths = [
        "color.primary-60",
        "color.positive-60",
        "color.critical-60",
        "color.caution-60",
        "color.neutral-60",
    ];
    let all_entries: Vec<&tokens_gallery::TokenEntry> = tiers
        .iter()
        .flat_map(|tier| &tier.groups)
        .flat_map(|group| &group.entries)
        .collect();
    let found_palette: Vec<&&tokens_gallery::TokenEntry> = palette_paths
        .iter()
        .filter_map(|p| all_entries.iter().find(|e| e.path == *p))
        .collect();
    // Part C (2026-07-04): the swatches attach directly to the token-count stat instead
    // of forming their own row with a separate caption sentence — one visual idea, not
    // three stacked widgets. Real values, same as before; no invented graphics.
    let palette: String = found_palette
        .iter()
        .map(|e| {
            format!(
                "<span class=\"home-swatch\" style=\"background:{}\" title=\"{}\"></span>",
                e.value, e.path
            )
        })
        .collect();

    // Faithful translation of the English copy below, not filler — see PR/commit
    // message for translation-review notes. Both variants kept side by side (rather
    // than an external strings file) since this is the only Spanish page today and a
    // second file would be pure indirection for one caller.
    let (eyebrow, h1, lede1, lede2_pre, cta_text, stat_components_label, stat_tokens_label, tag_dtcg, tag_apache,
         product_strip_label, kp_stat, gis_stat, oc_stat,
         registry_badge, registry_h2, registry_intro, registry_source_label,
         out1_title, out1_body, out2_title, out2_body, out3_title, out3_body, out4_title, out4_body, registry_caption,
         domains_h2, domains_intro,
         d1_h3, d1_a, d1_b, d1_c, d2_h3, d2_a, d2_b, d2_c, d3_h3, d3_a, d3_b, d3_c, d4_h3, d4_a, d4_b, d4_c,
         proof_h2, proof_intro, proof_l1, proof_l2, proof_l3, proof_l4, proof_example_label, proof_footnote,
         closing_h3, closing_p_pre, closing_p_mid, closing_p_post, closing_cta_text,
         selfhost_eyebrow, selfhost_h2, selfhost_p, selfhost_cta) = match lang {
        Lang::En => (
            "Token documentation &amp; component library",
            "One governed token graph, not a components folder every team forks and drifts&nbsp;from.",
            "Most design systems ship a components folder — files, a Storybook, a package to \
             install. The moment a team needs something the library doesn't have, they fork it, and the fork drifts \
             from the source. This system ships the token graph itself: DTCG-native, self-hostable, and readable \
             directly by the codegen agents that consume it, not just by designers.",
            "Every token, every component recipe, and every research decision behind it lives \
             in one versioned source — browse it below, or ",
            "download the whole graph as a bundle",
            "components &amp; elements",
            "tokens",
            "DTCG-native",
            "Apache-2.0 tokens &amp; bundles",
            "Also built on these tokens",
            "Powers documentation.pointsav.com's wiki engine",
            "Live at gis.woodfinegroup.com (v0.1.94)",
            "3 components, landed this release",
            "One source of truth",
            "Everything on this site is read from one registry.",
            "Navigation, the counts on this page, the machine API, and every download all resolve against the same registry file. A component that isn't registered can't appear in the nav, and a count here can't drift from what a release actually contains — there is exactly one place any of these numbers come from.",
            "single source of truth",
            "Nav", "Tokens / Components / Product lines (Knowledge Platform, GIS, Org Charts) / Writing / Paper / Self-host / Agents / Releases",
            "Homepage stats", "Counts rendered above, same numbers the registry serves",
            "Machine API", "MCP tools + component/token endpoints — what agents query",
            "Downloads", "Bundle downloads — nothing served that isn't registered",
            "A published count, a navigation entry, and a bundle download all resolve against the same registered record.",
            "Four token domains, one Apache-2.0 graph.",
            "Every domain below is DTCG JSON under Apache-2.0 — pull the whole graph or just the pieces you need into your own build, with no server required.",
            "The full token set", "Color, type, spacing", "Motion, elevation, status", "Accessibility targets",
            "Recipes, not screenshots", "HTML + CSS + ARIA per component", "Carbon-baseline reference where one exists", "Machine-readable via the registry",
            "Voice, versioned like tokens", "Voice, registers, mechanics", "Terminology A–Z", "Worked before/after pairs",
            "Document-format tokens", "Page geometry, pagination counters", "Four-step rule-weight ladder", "Built for subscription agreements, disclosure documents, and other regulated print formats",
            "Every number below is live.",
            "There's no separate cached copy of this data — the server reads the same dtcg-vault/ directory the numbers below are counted from, every time.",
            "Components", "Tokens", "MCP tools", "Paper document families",
            "Example response",
            "Every component and token endpoint on this site reads from the same dtcg-vault/ directory the server scans directly.",
            "This registry is real, and it grows.",
            "", " components and ", " tokens today, across the token graph, its component recipes, and the Paper/Writing pillars — every release adds to the same registry.",
            "See the version history",
            "Optional",
            "Or run the whole publishing stack yourself.",
            "The tokens above are complete on their own — nothing on this page requires a server. \
             The same engine that runs this site, app-privategit-design, is also available for a \
             different company to publish and govern its own design system on: its own tokens, its \
             own change history, its own on-prem MCP endpoint, inside its own infrastructure. That's \
             a separate product from what's on this page.",
            "Publish your own design system",
        ),
        Lang::Es => (
            "Documentación de tokens y biblioteca de componentes",
            "Un grafo de tokens gobernado — no una carpeta de componentes que cada equipo bifurca y termina&nbsp;desalineando.",
            "La mayoría de los sistemas de diseño distribuyen una carpeta de componentes — archivos, un Storybook, un \
             paquete para instalar. En el momento en que un equipo necesita algo que la biblioteca no tiene, la bifurca, \
             y la bifurcación se desalinea de la fuente. Este sistema distribuye el propio grafo de tokens: nativo en \
             DTCG, autoalojable, y legible directamente por los agentes de generación de código que lo consumen, no \
             solo por los diseñadores.",
            "Cada token, cada receta de componente y cada decisión de investigación detrás de ellos vive en una sola \
             fuente versionada — explórela a continuación, o ",
            "descargue todo el grafo como paquete",
            "componentes y elementos",
            "tokens",
            "Nativo en DTCG",
            "Tokens y paquetes con licencia Apache-2.0",
            "También construido sobre estos tokens",
            "Impulsa el motor wiki de documentation.pointsav.com",
            "En vivo en gis.woodfinegroup.com (v0.1.94)",
            "3 componentes, publicados en esta versión",
            "Una única fuente de verdad",
            "Todo en este sitio se lee de un solo registro.",
            "La navegación, los conteos de esta página, la API para máquinas y cada descarga se resuelven contra el mismo archivo de registro. Un componente que no está registrado no puede aparecer en la navegación, y un conteo aquí no puede desviarse de lo que una versión realmente contiene — hay exactamente un lugar de donde provienen estos números.",
            "única fuente de verdad",
            "Navegación", "Tokens / Componentes / Líneas de producto (Knowledge Platform, GIS, Organigramas) / Writing / Paper / Auto-hospedaje / Agentes / Versiones",
            "Estadísticas de inicio", "Conteos mostrados arriba, los mismos números que sirve el registro",
            "API para máquinas", "Herramientas MCP + endpoints de componentes/tokens — lo que consultan los agentes",
            "Descargas", "Descargas de paquetes — nada se sirve sin estar registrado",
            "Un conteo publicado, una entrada de navegación y una descarga de paquete se resuelven contra el mismo registro.",
            "Cuatro dominios de tokens, un solo grafo Apache-2.0.",
            "Cada dominio a continuación es JSON en formato DTCG bajo licencia Apache-2.0 — descargue el grafo completo o solo las partes que necesita para su propio proyecto, sin necesidad de servidor.",
            "El conjunto completo de tokens", "Color, tipografía, espaciado", "Movimiento, elevación, estado", "Objetivos de accesibilidad",
            "Recetas, no capturas de pantalla", "HTML + CSS + ARIA por componente", "Referencia de línea base Carbon donde exista", "Legible por máquinas a través del registro",
            "Voz, versionada como los tokens", "Voz, registros, mecánica", "Terminología A–Z", "Pares de antes/después trabajados",
            "Tokens de formato de documento", "Geometría de página, contadores de paginación", "Escala de cuatro niveles de grosor de regla", "Diseñado para acuerdos de suscripción, documentos de divulgación y otros formatos impresos regulados",
            "Cada número a continuación está en vivo.",
            "No hay una copia en caché separada de estos datos — el servidor lee el mismo directorio dtcg-vault/ del que se cuentan los números a continuación, cada vez.",
            "Componentes", "Tokens", "Herramientas MCP", "Familias de documentos Paper",
            "Respuesta de ejemplo",
            "Cada endpoint de componentes y tokens en este sitio lee del mismo directorio dtcg-vault/ que el servidor escanea directamente.",
            "Este registro es real, y crece.",
            "", " componentes y ", " tokens hoy, en el grafo de tokens, sus recetas de componentes y los pilares Paper/Writing — cada versión se suma al mismo registro.",
            "Ver el historial de versiones",
            "Opcional",
            "O ejecute usted mismo toda la plataforma de publicación.",
            "Los tokens anteriores son completos por sí solos — nada en esta página requiere un \
             servidor. El mismo motor que ejecuta este sitio, app-privategit-design, también está \
             disponible para que otra empresa publique y gobierne su propio sistema de diseño: sus \
             propios tokens, su propio historial de cambios, su propio endpoint MCP local, dentro \
             de su propia infraestructura. Eso es un producto separado de lo que se muestra en esta \
             página.",
            "Publique su propio sistema de diseño",
        ),
    };

    // Phase C (2026-07-15) -- two-column hero + live token-preview panel, ported
    // from the v3 mockup: the mockup's own research found this the single biggest
    // visual gap (text + stats only, no anchor visual). Swatches/type/rows below
    // are bound to this page's own real CSS custom properties, not a screenshot --
    // an actual live sample of the token graph the page is about. Real values
    // (not the mockup's illustrative ones) confirmed directly against tokens.css:
    // --cds-interactive #234ed8, --cds-font-mono 'JetBrains Mono', --cds-radius-md 0.25rem.
    let content = format!(
        "<div class=\"home-body\">\
         <section class=\"home-hero-section\">\
         <div class=\"home-hero\">\
         <p class=\"home-eyebrow\">{eyebrow}</p>\
         <h1>{h1}</h1>\
         <p class=\"home-lede\">{lede1}</p>\
         <p class=\"home-lede\">{lede2_pre}<a class=\"home-cta-button\" href=\"/bundles/tokens\">{cta_text}</a>.</p>\
         <div class=\"home-stats\">\
         <div class=\"home-stat\"><span class=\"home-stat-value\">{item_total}</span>\
         <span class=\"home-stat-label\">{stat_components_label}</span></div>\
         <div class=\"home-stat\"><span class=\"home-stat-value\">{token_count}</span>\
         <span class=\"home-stat-label\">{stat_tokens_label}</span>\
         <span class=\"home-stat-swatches\" aria-hidden=\"true\">{palette}</span></div>\
         <span class=\"home-stat-tag\">{tag_dtcg}</span>\
         <span class=\"home-stat-tag\">{tag_apache}</span>\
         </div>\
         </div>\
         <div class=\"token-preview\" aria-hidden=\"true\">\
         <div class=\"token-preview__label\">Live from tokens.css :root</div>\
         <div class=\"token-preview__swatches\">\
         <span class=\"token-preview__swatch\" style=\"background: var(--cds-interactive)\"></span>\
         <span class=\"token-preview__swatch\" style=\"background: var(--cds-selected-text)\"></span>\
         <span class=\"token-preview__swatch\" style=\"background: var(--cds-positive-text)\"></span>\
         <span class=\"token-preview__swatch\" style=\"background: var(--cds-caution-text)\"></span>\
         <span class=\"token-preview__swatch\" style=\"background: var(--cds-critical-text)\"></span>\
         </div>\
         <div class=\"token-preview__type\">Aa</div>\
         <div class=\"token-preview__rows\">\
         <div class=\"token-preview__row\"><code>--cds-interactive</code><span class=\"token-preview__row-value\">#234ed8</span></div>\
         <div class=\"token-preview__row\"><code>--cds-font-mono</code><span class=\"token-preview__row-value\">JetBrains Mono</span></div>\
         <div class=\"token-preview__row\"><code>--cds-radius-md</code><span class=\"token-preview__row-value\">0.25rem</span></div>\
         </div>\
         </div>\
         </section>\
         <section class=\"product-strip\" aria-label=\"{product_strip_label}\">\
         <span class=\"product-strip__label\">{product_strip_label}</span>\
         <div class=\"product-strip__items\">\
         <a class=\"product-strip__item\" href=\"/products/knowledge-platform/overview\">\
         <span class=\"product-strip__name\">Knowledge Platform</span>\
         <span class=\"product-strip__stat\">{kp_stat}</span>\
         </a>\
         <a class=\"product-strip__item\" href=\"/products/gis/overview\">\
         <span class=\"product-strip__name\">GIS</span>\
         <span class=\"product-strip__stat\">{gis_stat}</span>\
         </a>\
         <a class=\"product-strip__item\" href=\"/products/org-charts/overview\">\
         <span class=\"product-strip__name\">Org Charts</span>\
         <span class=\"product-strip__stat\">{oc_stat}</span>\
         </a>\
         </div>\
         </section>\
         <section class=\"act\">\
         <div class=\"act__label\"><span class=\"badge badge--brand\">{registry_badge}</span></div>\
         <h2>{registry_h2}</h2>\
         <p class=\"act__intro\">{registry_intro}</p>\
         <div class=\"registry-diagram\">\
         <div class=\"registry-diagram__source\">dtcg-vault/ — {registry_source_label}</div>\
         <div class=\"registry-diagram__stem\"></div>\
         <div class=\"registry-diagram__rail\"></div>\
         <div class=\"registry-diagram__outputs\">\
         <div class=\"registry-diagram__output\"><div class=\"registry-diagram__output-stem\"></div>\
         <div class=\"registry-diagram__output-box\"><strong>{out1_title}</strong>{out1_body}</div></div>\
         <div class=\"registry-diagram__output\"><div class=\"registry-diagram__output-stem\"></div>\
         <div class=\"registry-diagram__output-box\"><strong>{out2_title}</strong>{out2_body}</div></div>\
         <div class=\"registry-diagram__output\"><div class=\"registry-diagram__output-stem\"></div>\
         <div class=\"registry-diagram__output-box\"><strong>{out3_title}</strong>{out3_body}</div></div>\
         <div class=\"registry-diagram__output\"><div class=\"registry-diagram__output-stem\"></div>\
         <div class=\"registry-diagram__output-box\"><strong>{out4_title}</strong>{out4_body}</div></div>\
         </div>\
         <p class=\"registry-diagram__caption\">{registry_caption}</p>\
         </div>\
         </section>\
         <section class=\"act\">\
         <h2>{domains_h2}</h2>\
         <p class=\"act__intro\">{domains_intro}</p>\
         <div class=\"card-grid\">\
         <div class=\"card\"><span class=\"card__eyebrow eyebrow\">Tokens</span><h3>{d1_h3}</h3>\
         <ul><li>{d1_a}</li><li>{d1_b}</li><li>{d1_c}</li></ul></div>\
         <div class=\"card\"><span class=\"card__eyebrow eyebrow\">Components</span><h3>{d2_h3}</h3>\
         <ul><li>{d2_a}</li><li>{d2_b}</li><li>{d2_c}</li></ul></div>\
         <div class=\"card\"><span class=\"card__eyebrow eyebrow\">Writing</span><h3>{d3_h3}</h3>\
         <ul><li>{d3_a}</li><li>{d3_b}</li><li>{d3_c}</li></ul></div>\
         <div class=\"card\"><span class=\"card__eyebrow eyebrow\">Paper</span><h3>{d4_h3}</h3>\
         <ul><li>{d4_a}</li><li>{d4_b}</li><li>{d4_c}</li></ul></div>\
         </div>\
         </section>\
         <section class=\"act\">\
         <span class=\"eyebrow\">{selfhost_eyebrow}</span>\
         <h2>{selfhost_h2}</h2>\
         <p class=\"act__intro\">{selfhost_p}</p>\
         <div class=\"hero__ctas\">\
         <a href=\"/developing/install/overview\" class=\"btn btn--secondary\">{selfhost_cta}</a>\
         </div>\
         </section>\
         <section class=\"act\">\
         <h2>{proof_h2}</h2>\
         <p class=\"act__intro\">{proof_intro}</p>\
         <div class=\"proof-panel\">\
         <div class=\"proof-panel__stats\">\
         <div class=\"proof-stat\"><div class=\"proof-stat__value\">{item_total}</div><div class=\"proof-stat__label\">{proof_l1}</div></div>\
         <div class=\"proof-stat\"><div class=\"proof-stat__value\">{token_count}</div><div class=\"proof-stat__label\">{proof_l2}</div></div>\
         <div class=\"proof-stat\"><div class=\"proof-stat__value\">4</div><div class=\"proof-stat__label\">{proof_l3}</div></div>\
         <div class=\"proof-stat\"><div class=\"proof-stat__value\">6</div><div class=\"proof-stat__label\">{proof_l4}</div></div>\
         </div>\
         <div class=\"curl-block\"><span class=\"curl-prompt\">$ </span><span class=\"curl-cmd\">curl</span> https://design.pointsav.com/components/button/recipe.json\n\
<span class=\"curl-note\"># {proof_example_label}</span>\n\
{{\n\
&nbsp;&nbsp;<span class=\"curl-key\">\"name\"</span>: <span class=\"curl-str\">\"button\"</span>,\n\
&nbsp;&nbsp;<span class=\"curl-key\">\"category\"</span>: <span class=\"curl-str\">\"components\"</span>,\n\
&nbsp;&nbsp;<span class=\"curl-key\">\"registry_type\"</span>: <span class=\"curl-str\">\"component\"</span>,\n\
&nbsp;&nbsp;<span class=\"curl-key\">\"variants\"</span>: [{{ <span class=\"curl-key\">\"name\"</span>: <span class=\"curl-str\">\"primary\"</span> }}, {{ <span class=\"curl-key\">\"name\"</span>: <span class=\"curl-str\">\"secondary\"</span> }} <span class=\"curl-note\">// … 3 more</span>],\n\
&nbsp;&nbsp;<span class=\"curl-key\">\"tokens\"</span>: [<span class=\"curl-str\">\"{{semantic.interactive-primary}}\"</span>, <span class=\"curl-str\">\"{{semantic.interactive-primary-hover}}\"</span>]\n\
}}</div>\
         <p class=\"proof-panel__footnote\">{proof_footnote}</p>\
         </div>\
         </section>\
         <div class=\"closing-cta\">\
         <div class=\"closing-cta__text\">\
         <h3>{closing_h3}</h3>\
         <p>{closing_p_pre}{item_total}{closing_p_mid}{token_count}{closing_p_post}</p>\
         </div>\
         <a class=\"btn btn--primary\" href=\"/releases/changelog/overview\">{closing_cta_text}</a>\
         </div>\
         <div class=\"home-grid\">{cards}</div></div>"
    );

    let (title, description, path) = match lang {
        Lang::En => (
            "PointSav Design System",
            render::SITE_DESCRIPTION,
            "/",
        ),
        Lang::Es => (
            "Sistema de Diseño PointSav",
            "Sistema de diseño de PointSav — tokens, componentes y documentación de investigación para la familia de productos PointSav/Woodfine, que abarca el lenguaje visual y los primitivos de interfaz de usuario.",
            "/es",
        ),
    };
    let page_lang = PageLang {
        lang,
        alt_en_path: "/".to_string(),
        alt_es_path: "/es".to_string(),
    };

    Html(render::shell(
        &state.env,
        &state.vault,
        &state.component_groups,
        &state.site_origin,
        title,
        description,
        path,
        &page_lang,
        &nav_html,
        "",
        "",
        &content,
    ))
}

pub async fn tokens_gallery_page(State(state): State<AppState>) -> Html<String> {
    let tiers = tokens_gallery::load_and_flatten(&state.vault);
    // Real stat-panel counts (2026-07-15) -- computed the same way render/mod.rs's
    // footer count is, from the same tiers this page itself renders below, so the
    // stat strip can never drift from what's actually shown on the page.
    let total_tokens: usize = tiers
        .iter()
        .map(|t| t.groups.iter().map(|g| g.entries.len()).sum::<usize>())
        .sum();
    let total_categories: usize = tiers.iter().map(|t| t.groups.len()).sum();
    let total_tiers = tiers.len();
    let body = state
        .env
        .get_template("tokens.html")
        .expect("tokens.html missing")
        .render(minijinja::context! {
            tiers => tiers,
            total_tokens => total_tokens,
            total_categories => total_categories,
            total_tiers => total_tiers,
        })
        .expect("render tokens.html failed");

    let nav_html = render::render_nav(&state.env, &state.component_groups, "", "");
    Html(render::shell(
        &state.env,
        &state.vault,
        &state.component_groups,
        &state.site_origin,
        "Tokens — PointSav Design System",
        "The PointSav Design System's token registry — primitive and theme-level DTCG tokens for the PointSav/Woodfine product family.",
        "/tokens",
        &PageLang::en_only(),
        &nav_html,
        "",
        "Tokens",
        &body,
    ))
}

#[derive(Serialize)]
struct AdoptionGroup {
    consumer: String,
    count: usize,
    slugs: Vec<String>,
}

/// Phase 4 — a real, verifiable "who uses this" view, not an invented usage metric.
/// Reuses the same `recipe.json` `category` field `discover_component_groups` already
/// computes for the sidebar grouping (Phase 2) and the per-component origin badge
/// (Phase 3) — this is the third and final consumer of that one authored fact.
pub async fn adoption_page(State(state): State<AppState>) -> Html<String> {
    let generic_count = state
        .component_groups
        .iter()
        .find(|(label, _)| label.is_empty())
        .map(|(_, slugs)| slugs.len())
        .unwrap_or(0);

    let consumers: Vec<AdoptionGroup> = state
        .component_groups
        .iter()
        .filter(|(label, _)| !label.is_empty())
        .map(|(label, slugs)| AdoptionGroup {
            consumer: label
                .strip_prefix("Also used on ")
                .or_else(|| label.strip_prefix("Also used by "))
                .unwrap_or(label)
                .to_string(),
            count: slugs.len(),
            slugs: slugs.clone(),
        })
        .collect();

    let body = state
        .env
        .get_template("adoption.html")
        .expect("adoption.html missing")
        .render(minijinja::context! { generic_count => generic_count, consumers => consumers })
        .expect("render adoption.html failed");

    let nav_html = render::render_nav(&state.env, &state.component_groups, "", "");
    Html(render::shell(
        &state.env,
        &state.vault,
        &state.component_groups,
        &state.site_origin,
        "Adoption — PointSav Design System",
        "Component adoption across the PointSav/Woodfine product family, grouped by consuming application, in the PointSav Design System registry.",
        "/adoption",
        &PageLang::en_only(),
        &nav_html,
        "",
        "Adoption",
        &body,
    ))
}

pub async fn item_redirect(
    Path((section, slug)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Response {
    if slug.contains("..") || slug.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid").into_response();
    }
    if !vault::is_known_section(&section) {
        return (StatusCode::NOT_FOUND, "unknown section").into_response();
    }
    let tabs = vault::discover_tabs(&state.vault, &section, &slug);
    let first = tabs
        .into_iter()
        .next()
        .unwrap_or_else(|| vault::default_tab(&section).to_string());
    Redirect::permanent(&format!("/{}/{}/{}", section, slug, first)).into_response()
}

pub async fn item_tab(
    Path((section, slug, tab)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Response {
    if slug.contains("..") || slug.contains('/') || tab.contains("..") || tab.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid").into_response();
    }
    if !vault::is_known_section(&section) {
        return (StatusCode::NOT_FOUND, "unknown section").into_response();
    }
    // A flat section (research/developing/designing/about) has exactly one canonical
    // tab per slug — reject any other tab value in the URL rather than silently serving
    // the same content at every tab (would otherwise be a duplicate-content URL smell).
    if vault::is_flat_section(&section) && tab != vault::default_tab(&section) {
        return (StatusCode::NOT_FOUND, "tab not found").into_response();
    }
    let tabs = vault::discover_tabs(&state.vault, &section, &slug);
    if tabs.is_empty() {
        return (StatusCode::NOT_FOUND, "item not found").into_response();
    }
    let md_path = vault::content_path(&state.vault, &section, &slug, &tab);
    let raw = match fs::read_to_string(&md_path) {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, "tab not found").into_response(),
    };

    let (frontmatter, body) = vault::parse_frontmatter(&raw);
    let schema_type = schema::detect(&frontmatter);
    let mut content = schema::render(schema_type, &frontmatter, &body);

    // P1.1 — live component preview (recipe.json variants, sandboxed via iframe).
    // Phase 3 — origin + freshness meta badges, ahead of the preview.
    if section == "components" {
        let badges = component_meta::render_meta_badges(&state.component_groups, &state.vault, &slug);
        if let Some(preview) = component_preview::render_preview(&state.vault, &slug) {
            content = format!("{badges}{preview}{content}");
        } else {
            content = format!("{badges}{content}");
        }
    }

    let nav_html = render::render_nav(&state.env, &state.component_groups, &section, &slug);
    let tab_bar = render::render_tab_bar(&state.env, &section, &slug, &tabs, &tab);
    let label = vault::to_title(&slug);

    // P2.2 — breadcrumb wayfinding (Home > Section > Item), especially useful in the
    // mobile drawer where the sidebar is collapsed by default.
    let breadcrumb = format!(
        "<nav class=\"breadcrumb\" aria-label=\"Breadcrumb\">\
         <a href=\"/\">Home</a><span aria-hidden=\"true\"> / </span>\
         <span>{}</span><span aria-hidden=\"true\"> / </span>\
         <span aria-current=\"page\">{}</span></nav>",
        vault::to_title(&section),
        label
    );
    let content = format!("{breadcrumb}{content}");

    // No real per-item description field exists anywhere in the vault content (checked
    // directly — the frontmatter parser exposes whatever fields a markdown file declares,
    // but no file in the vault actually declares one), so frontmatter is checked first and
    // a grounded, section/label-derived fallback covers every other case rather than
    // inventing per-item copy.
    let description = frontmatter
        .get("description")
        .cloned()
        .unwrap_or_else(|| {
            format!(
                "{label} — {} in the PointSav Design System registry for the PointSav/Woodfine product family.",
                vault::to_title(&section)
            )
        });
    let path = format!("/{section}/{slug}/{tab}");

    Html(render::shell(
        &state.env,
        &state.vault,
        &state.component_groups,
        &state.site_origin,
        &format!("{} — PointSav Design System", label),
        &description,
        &path,
        &PageLang::en_only(),
        &nav_html,
        &tab_bar,
        &label,
        &content,
    ))
    .into_response()
}

/// GET /elements/:slug/download — ZIP all non-.md members from vault/elements/<slug>/
pub async fn bundle_download(Path(slug): Path<String>, State(state): State<AppState>) -> Response {
    if slug.contains("..") || slug.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid").into_response();
    }
    let elem_dir = state.vault.join("elements").join(&slug);
    let Ok(entries) = fs::read_dir(&elem_dir) else {
        return (StatusCode::NOT_FOUND, "bundle not found").into_response();
    };

    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let mut zip_writer = zip::ZipWriter::new(cursor);
    let zip_opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        // include all files; .md are the vault doc, skip them in the download
        if name.ends_with(".md") {
            continue;
        }
        let Ok(content) = fs::read(entry.path()) else {
            continue;
        };
        let _ = zip_writer.start_file(&name, zip_opts);
        let _ = zip_writer.write_all(&content);
    }
    let Ok(cursor) = zip_writer.finish() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "zip error").into_response();
    };
    let body = cursor.into_inner();
    let disposition = format!("attachment; filename=\"{}.zip\"", slug);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, &disposition)
        .body(Body::from(body))
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "response error").into_response())
}

/// GET /components/:slug/recipe.json — serve one component's recipe.json verbatim as
/// application/json. Closes a real ergonomics gap: before this route, the only way to
/// fetch a single component's recipe over the wire was a POST /mcp JSON-RPC round trip
/// (get_component_recipe), with the recipe string nested inside result.content[0].text.
/// This gives agents and plain curl a flat GET, matching how every other file-shaped
/// resource on this server is served (see bundle::file).
pub async fn component_recipe(Path(slug): Path<String>, State(state): State<AppState>) -> Response {
    if slug.contains("..") || slug.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid").into_response();
    }
    let recipe_path = state.vault.join("components").join(&slug).join("recipe.json");
    match fs::read_to_string(&recipe_path) {
        Ok(raw) => ([(header::CONTENT_TYPE, "application/json")], raw).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "component not found").into_response(),
    }
}
