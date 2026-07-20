// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// First-pass Spanish support (2026-07-12): homepage + shared chrome (nav/footer/
// header) only — matching home.woodfinegroup.com's toggle pattern for the parts that
// overlap. The ~84 deep vault content pages (tokens/components/research/etc.) are NOT
// translated yet; those already have a real per-file precedent (.es.md siblings, see
// vault.rs's existing `.es.md` filtering + README.es.md /
// elements/org-chart-tokens/overview.es.md) that a later pass can extend — this module
// deliberately does not build that path, since content strings here don't come from
// vault files at all (homepage copy is inline in browse.rs, chrome is templates).

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Es,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Es => "es",
        }
    }
}

/// Chrome strings shared by every page (nav/footer/header). Only the fields actually
/// used today are present — extend as more chrome gets a Spanish pass.
pub struct ChromeStrings {
    pub skip_to_content: &'static str,
    pub search_placeholder: &'static str,
    pub search_aria_label: &'static str,
    pub theme_toggle_aria_label: &'static str,
    pub nav_toggle_aria_label: &'static str,
    pub footer_site_label: &'static str,
    pub footer_canonical_title: &'static str,
    pub footer_overview: &'static str,
    pub footer_components: &'static str,
    pub footer_tokens: &'static str,
    pub footer_guidelines: &'static str,
    pub footer_paper: &'static str,
    pub footer_writing: &'static str,
    pub footer_bundles: &'static str,
    pub footer_adoption: &'static str,
    pub footer_get_started: &'static str,
    pub footer_machine_surface_title: &'static str,
    pub footer_family_prefix: &'static str,
    pub footer_copyright: &'static str,
    pub footer_live: &'static str,
    pub footer_powered_by: &'static str,
    /// The OTHER language's own-language name — an EN page's toggle reads "Español",
    /// an ES page's toggle reads "English", matching home.woodfinegroup.com's pattern.
    pub lang_switch_label: &'static str,
    // v3 identity-plate + disclosure footer fields (Phase 4 port from the approved
    // .agent/mockups/v3-design-system/_chrome-snippet.html, round 5-13 wording — real
    // facts (LICENSE-MATRIX.md, home.woodfinegroup.com's live m-footer__notice), not
    // invented here.
    pub footer_identity_tagline: &'static str,
    pub footer_identity_standards: &'static str,
    pub footer_identity_license: &'static str,
    pub footer_identity_source_label: &'static str,
    pub footer_network_title: &'static str,
    pub footer_network_pointsav: &'static str,
    pub footer_network_documentation: &'static str,
    pub footer_network_software: &'static str,
    pub footer_network_woodfine: &'static str,
    pub footer_locations: &'static str,
    pub footer_trademark: &'static str,
    pub footer_disclosure_notice: &'static str,
    pub disclosure_summary: &'static str,
    pub disclosure_label: &'static str,
    pub disclosure_body: &'static str,
    pub nav_tokens: &'static str,
    pub nav_components: &'static str,
    pub nav_guidelines: &'static str,
    pub nav_accessibility: &'static str,
    pub nav_elements: &'static str,
    pub nav_writing: &'static str,
    pub nav_paper: &'static str,
    pub nav_agents: &'static str,
    pub nav_adoption: &'static str,
    pub nav_product_lines: &'static str,
    pub nav_knowledge_platform: &'static str,
    pub nav_org_charts: &'static str,
    pub nav_more: &'static str,
    pub nav_releases: &'static str,
}

impl ChromeStrings {
    pub fn for_lang(lang: Lang) -> Self {
        match lang {
            Lang::En => ChromeStrings {
                skip_to_content: "Skip to main content",
                search_placeholder: "Search…",
                search_aria_label: "Search the design system",
                theme_toggle_aria_label: "Toggle dark mode",
                nav_toggle_aria_label: "Toggle navigation menu",
                footer_site_label: "Site footer",
                footer_canonical_title: "Canonical",
                footer_overview: "Overview",
                footer_components: "Components",
                footer_tokens: "Tokens",
                footer_guidelines: "Guidelines",
                footer_paper: "Paper",
                footer_writing: "Writing",
                footer_bundles: "Bundles",
                footer_adoption: "Adoption",
                footer_get_started: "Get started — download tokens",
                footer_machine_surface_title: "Machine surface",
                footer_family_prefix: "Part of the PointSav family:",
                footer_copyright: "Copyright © 2026 Woodfine Capital Projects Inc. All rights reserved.",
                footer_live: "live",
                footer_powered_by: "Powered by",
                lang_switch_label: "Español",
                footer_identity_tagline: "Open design tokens and components, published for anyone to use",
                footer_identity_standards: "DTCG Format Module 2025.10 · W3C Design Tokens Community Group",
                footer_identity_license: "Design tokens licensed Apache-2.0 · server source AGPL-3.0-or-later · distributed binary PointSav-Commercial",
                footer_identity_source_label: "Design system source (github.com/pointsav)",
                footer_network_title: "Network",
                footer_network_pointsav: "PointSav Digital Systems",
                footer_network_documentation: "Documentation",
                footer_network_software: "Software",
                footer_network_woodfine: "Woodfine Capital Projects",
                footer_locations: "Vancouver | New York",
                footer_trademark: "Woodfine Capital Projects™, MCorp™, PointSav Digital Systems™, Totebox Orchestration™, Totebox Archive™, and Capability Geometry™ are trademarks of Woodfine Capital Projects Inc., used in Canada, the United States, Latin America, and Europe. All other trademarks are the property of their respective owners.",
                footer_disclosure_notice: "Provided for information only — not an offer, solicitation, or advice.",
                disclosure_summary: "Important Information",
                disclosure_label: "Design System disclosure",
                disclosure_body: "This site provides open-source design tokens, documentation, and self-hostable software published by Woodfine Capital Projects Inc. Information here is for general reference only and does not constitute an offer, warranty, or a guarantee of fitness for any particular purpose. Statements regarding planned, intended, or targeted future features are forward-looking and subject to change without notice; they are not undertaken to be updated except as required by law.",
                nav_tokens: "Tokens",
                nav_components: "Components",
                nav_guidelines: "Guidelines",
                nav_accessibility: "Accessibility",
                nav_elements: "Elements",
                nav_writing: "Writing",
                nav_paper: "Paper",
                nav_agents: "Agents",
                nav_adoption: "Self-host",
                nav_product_lines: "Product lines",
                nav_knowledge_platform: "Knowledge Platform",
                nav_org_charts: "Org Charts",
                nav_more: "Develop",
                nav_releases: "Releases",
            },
            Lang::Es => ChromeStrings {
                skip_to_content: "Saltar al contenido principal",
                search_placeholder: "Buscar…",
                search_aria_label: "Buscar en el sistema de diseño",
                theme_toggle_aria_label: "Cambiar a modo oscuro",
                nav_toggle_aria_label: "Alternar menú de navegación",
                footer_site_label: "Pie de página del sitio",
                footer_canonical_title: "Canónico",
                footer_overview: "Resumen",
                footer_components: "Componentes",
                footer_tokens: "Tokens",
                footer_guidelines: "Directrices",
                footer_paper: "Paper",
                footer_writing: "Redacción",
                footer_bundles: "Paquetes",
                footer_adoption: "Adopción",
                footer_get_started: "Empezar — descargar tokens",
                footer_machine_surface_title: "Interfaz para máquinas",
                footer_family_prefix: "Parte de la familia PointSav:",
                footer_copyright: "Copyright © 2026 Woodfine Capital Projects Inc. Todos los derechos reservados.",
                footer_live: "en vivo",
                footer_powered_by: "Desarrollado con",
                lang_switch_label: "English",
                footer_identity_tagline: "Tokens y componentes de diseño abiertos, publicados para que cualquiera los use",
                footer_identity_standards: "DTCG Format Module 2025.10 · W3C Design Tokens Community Group",
                footer_identity_license: "Tokens de diseño licenciados bajo Apache-2.0 · código del servidor bajo AGPL-3.0-or-later · binario distribuido PointSav-Commercial",
                footer_identity_source_label: "Código fuente del sistema de diseño (github.com/pointsav)",
                footer_network_title: "Red",
                footer_network_pointsav: "PointSav Digital Systems",
                footer_network_documentation: "Documentación",
                footer_network_software: "Software",
                footer_network_woodfine: "Woodfine Capital Projects",
                footer_locations: "Vancouver | Nueva York",
                footer_trademark: "Woodfine Capital Projects™, MCorp™, PointSav Digital Systems™, Totebox Orchestration™, Totebox Archive™ y Capability Geometry™ son marcas registradas de Woodfine Capital Projects Inc., utilizadas en Canadá, Estados Unidos, Latinoamérica y Europa. Todas las demás marcas son propiedad de sus respectivos dueños.",
                footer_disclosure_notice: "Se proporciona solo con fines informativos — no constituye una oferta, solicitud ni asesoría.",
                disclosure_summary: "Información Importante",
                disclosure_label: "Divulgación del sistema de diseño",
                disclosure_body: "Este sitio proporciona tokens de diseño de código abierto, documentación y software autoalojable publicado por Woodfine Capital Projects Inc. La información aquí es solo para referencia general y no constituye una oferta, garantía ni garantía de idoneidad para ningún propósito particular. Las declaraciones sobre características futuras planeadas, previstas o proyectadas son prospectivas y están sujetas a cambios sin previo aviso; no se asume la obligación de actualizarlas excepto cuando lo exija la ley.",
                nav_tokens: "Tokens",
                nav_components: "Componentes",
                nav_guidelines: "Directrices",
                nav_accessibility: "Accesibilidad",
                nav_elements: "Elementos",
                nav_writing: "Redacción",
                nav_paper: "Paper",
                nav_agents: "Agentes",
                nav_adoption: "Auto-hospedar",
                nav_product_lines: "Líneas de producto",
                nav_knowledge_platform: "Knowledge Platform",
                nav_org_charts: "Organigramas",
                nav_more: "Desarrollo",
                nav_releases: "Versiones",
            },
        }
    }
}

/// Bundles a page's language + its known translation-pair paths (empty string = no
/// counterpart page exists yet, so shell() skips rendering hreflang alternates and the
/// language-switch toggle rather than pointing at a page that doesn't exist).
pub struct PageLang {
    pub lang: Lang,
    pub alt_en_path: String,
    pub alt_es_path: String,
}

impl PageLang {
    /// The common case: an English-only page with no Spanish counterpart yet.
    pub fn en_only() -> Self {
        PageLang {
            lang: Lang::En,
            alt_en_path: String::new(),
            alt_es_path: String::new(),
        }
    }
}

/// Sidebar section-heading labels. Only the 7 real `vault::SECTIONS` names — item
/// slugs underneath stay in English regardless of `lang` (their content isn't
/// translated yet; translating just the label would misrepresent what's behind it).
pub fn section_label(lang: Lang, section: &str) -> String {
    if lang == Lang::Es {
        match section {
            "elements" => "Elementos".to_string(),
            "components" => "Componentes".to_string(),
            "guidelines" => "Directrices".to_string(),
            "developing" => "Desarrollo".to_string(),
            "designing" => "Diseño".to_string(),
            "about" => "Acerca de".to_string(),
            "research" => "Investigación".to_string(),
            other => crate::vault::to_title(other),
        }
    } else {
        crate::vault::to_title(section)
    }
}
