// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Chrome shell: masthead, hero band, footer, mobile drawer. Tenant-dispatched
//! through [`Tenant`] so one binary serves two brands with the same chrome
//! shape and different marks/links/legal text — the Sovereign Editorial
//! direction's locked architecture, reimplemented fresh (not ported) here.
//!
//! Per DESIGN-SYSTEM.md: no masthead search bar (no search corpus to justify
//! one); the mobile drawer mirrors the wiki's proven pre-rendered slide-in
//! pattern; the footer is three-tier (nav columns → on-page jurisdiction
//! disclosure slots → badge/trademark/copyright base row).

use maud::{html, Markup, PreEscaped, DOCTYPE};

use crate::content::{Page, Section};

/// Pick a UI string by page language. Chrome-level "furniture" strings only
/// (nav labels, button labels, boilerplate) — never legal/disclosure text,
/// which is routed to project-editorial for professional translation rather
/// than drafted here (see `Tenant::trademark_line`/`disclosure_slots`).
fn t<'a>(lang: &str, en: &'a str, es: &'a str) -> &'a str {
    if lang == "es" {
        es
    } else {
        en
    }
}

#[derive(Debug, Clone)]
pub struct NavLink {
    pub label: &'static str,
    pub label_es: &'static str,
    pub href: &'static str,
    pub external: bool,
}

impl NavLink {
    pub const fn internal(label: &'static str, label_es: &'static str, href: &'static str) -> Self {
        Self {
            label,
            label_es,
            href,
            external: false,
        }
    }
    pub const fn external(label: &'static str, label_es: &'static str, href: &'static str) -> Self {
        Self {
            label,
            label_es,
            href,
            external: true,
        }
    }

    fn label_for(&self, lang: &str) -> &'static str {
        t(lang, self.label, self.label_es)
    }
}

/// A single on-page jurisdiction disclosure slot, rendered inside the
/// footer's collapsed-by-default "Important information" accordion. Per SEC
/// Marketing Rule "clear and prominent" guidance: on-page, not hidden behind
/// a separate link — but collapsed (not a second always-visible copy) once
/// operator feedback found it duplicating the page's own legal prose.
#[derive(Debug, Clone)]
pub struct DisclosureSlot {
    pub label: &'static str,
    pub label_es: &'static str,
    /// Markdown source (rendered via [`crate::content::render_markdown`]) —
    /// this carries the full legal text that previously lived as a separate
    /// `type: prose` content section on the home page. A real, existing
    /// human translation (not machine-generated) — ported verbatim from
    /// that Spanish prose section, not newly drafted here.
    pub body: &'static str,
    pub body_es: &'static str,
}

impl DisclosureSlot {
    fn label_for(&self, lang: &str) -> &'static str {
        t(lang, self.label, self.label_es)
    }
    fn body_for(&self, lang: &str) -> &'static str {
        t(lang, self.body, self.body_es)
    }
}

/// Per-tenant chrome configuration. Chrome *shape* is identical across
/// tenants (see [`page_shell`]); only marks, links, and legal text differ.
#[derive(Debug, Clone)]
pub struct Tenant {
    pub module_id: &'static str,
    pub site_title: &'static str,
    pub wordmark_label: &'static str,
    pub nav_links: Vec<NavLink>,
    pub footer_nav: Vec<NavLink>,
    /// External network links duplicated into a second footer column so the
    /// off-site destinations (Corporate/Projects/Newsroom, or Documentation/
    /// Software/Design System/Newsroom) are reachable from the footer without
    /// opening the hamburger drawer — mirrors the wiki's own footer pattern
    /// (Browse / This site / Network). Reuses the same label/href triples as
    /// the tenant's masthead `nav_links` external entries.
    pub footer_network: Vec<NavLink>,
    pub cities: Vec<&'static str>,
    /// Always "Woodfine Capital Projects Inc." per TRADEMARK.md v1.1 —
    /// never the tenant's own operating entity, on either brand's site.
    pub copyright_holder: &'static str,
    /// Verbatim canonical sentence per TRADEMARK.md — identical on both
    /// brands' sites (operator call 2026-07-02; an earlier per-brand
    /// shorter-subset design for PointSav was superseded).
    pub trademark_line: &'static str,
    pub disclosure_slots: Vec<DisclosureSlot>,
    /// Real canonical favicon SVG (FABLE audit 2026-07-02 found NEITHER
    /// tenant shipped a favicon link at all — not just PointSav).
    pub favicon_href: &'static str,

    // --- SEO (P4) ---
    /// Canonical base URL for this tenant (no trailing slash).
    pub canonical_base: &'static str,
    /// Open Graph `og:site_name`.
    pub og_site_name: &'static str,
    /// schema.org `@type` for the root LD+JSON block.
    pub ld_json_type: &'static str,
    /// Site-level description used in LD+JSON when a page has none.
    pub ld_json_description: &'static str,
}

impl Tenant {
    pub fn woodfine() -> Self {
        Self {
            module_id: "woodfine",
            site_title: "Woodfine Capital Projects",
            wordmark_label: "Woodfine Capital Projects",
            // Masthead/drawer nav = credibility layer (behind the hamburger
            // on mobile — 2 taps). Task-destination items (Projects, BIM
            // Library, Location Intelligence) live in the page's button row
            // instead, which is the actual 1-tap mobile fast path — FABLE
            // nav-priority audit 2026-07-02. "Projects" moved OUT of here
            // (into the button row) to avoid re-creating a duplication.
            nav_links: vec![
                NavLink::external("Corporate", "Corporativo", "https://corporate.woodfinegroup.com/"),
                // Restored 2026-07-02 — present on the retired production
                // site's masthead nav, dropped when this chrome was rebuilt.
                NavLink::external("Newsroom", "Sala de prensa", "https://woodfinegroup.com/"),
                NavLink::internal("Contact Us", "Contáctenos", "/page/contact"),
                NavLink::internal("Disclaimer", "Aviso legal", "/page/disclaimer"),
            ],
            footer_nav: vec![
                NavLink::internal("Contact Us", "Contáctenos", "/page/contact"),
                NavLink::internal("Disclaimer", "Aviso legal", "/page/disclaimer"),
                NavLink::internal("Privacy", "Privacidad", "/page/privacy"),
            ],
            // Full external-destination list (masthead + button-row items
            // combined) so everything stays reachable from the footer
            // regardless of what's promoted to the 1-tap button row above —
            // FABLE nav-priority audit 2026-07-02. Hrefs duplicated from
            // content/home/page.yaml's button-row cards by necessity (footer
            // nav is tenant-wide Rust config, not per-page content).
            footer_network: vec![
                NavLink::external("Corporate", "Corporativo", "https://corporate.woodfinegroup.com/"),
                NavLink::external("Newsroom", "Sala de prensa", "https://woodfinegroup.com/"),
                NavLink::external("Projects", "Proyectos", "https://projects.woodfinegroup.com/"),
                NavLink::external("BIM Library", "Biblioteca BIM", "https://bim.woodfinegroup.com/"),
                NavLink::external("Location Intelligence", "Inteligencia de Localización", "https://gis.woodfinegroup.com/"),
                NavLink::external("Manifest", "Manifiesto", "https://github.com/woodfine/woodfine-fleet-deployment"),
            ],
            cities: vec!["Vancouver", "New York"],
            copyright_holder: "Woodfine Capital Projects Inc.",
            // Verbatim canonical sentence per TRADEMARK.md (2026-07-02 correction —
            // "Woodfine Management Corp\u{2122}" was wrong, canonical mark is
            // "MCorp\u{2122}"; "Capability Geometry\u{2122}" was missing entirely).
            trademark_line: "Woodfine Capital Projects\u{2122}, MCorp\u{2122}, \
                PointSav Digital Systems\u{2122}, Totebox Orchestration\u{2122}, Totebox \
                Archive\u{2122}, and Capability Geometry\u{2122} are trademarks of Woodfine \
                Capital Projects Inc., used in Canada, the United States, Latin America, and \
                Europe. All other trademarks are the property of their respective owners.",
            // Full legal text — moved here 2026-07-02 from a separate,
            // always-visible `type: prose` home-page section (operator
            // feedback: it read as a duplicate of this same accordion).
            // Markdown-rendered; see `DisclosureSlot::body`.
            //
            // Hyperscaler-grade legal pass 2026-07-02 (Opus agent, hyperscaler-
            // law-firm-counsel brief): converted the orphan numbered risk list
            // into the same bold-label-lead paragraph pattern PointSav's body
            // already used (one shared structural grammar across both);
            // reconciled "offering memorandum" to the canonical term "Private
            // Placement Memorandum" used throughout factory-release-engineering
            // (DISCLAIMER.md, HOMEPAGE-DISCLAIMER.md — this accordion had
            // drifted from it); added "Changes to this notice" and "Full
            // disclaimer" closers, both genuinely absent standard clauses.
            // body_es is a fresh translation matching every change — flagged
            // for a native legal-translator verification pass before this is
            // treated as final (same caveat as any AI-drafted Spanish legal
            // text this session).
            disclosure_slots: vec![DisclosureSlot {
                label: "Securities disclosure",
                label_es: "Divulgación de valores",
                body: "**Securities offering.** Woodfine Capital Projects Inc. (\u{201c}Woodfine\u{201d}) \
                    sponsors real-property direct-hold solutions. Interests in those solutions \
                    are offered only to investors who qualify under an applicable prospectus \
                    exemption — including the accredited-investor exemption under National \
                    Instrument 45-106 — Prospectus Exemptions, and equivalent exemptions in \
                    other applicable jurisdictions. The information on this page is provided for \
                    general informational purposes only and does not constitute an offer to \
                    sell, or a solicitation of an offer to buy, any security. Any offering is \
                    made exclusively by means of the applicable Private Placement Memorandum, \
                    which prospective investors should review, together with their own \
                    professional advisors, before investing.\n\n\
                    **Scope.** The information on this page describes Woodfine and its \
                    activities at a high level and is qualified in its entirety by the \
                    applicable Private Placement Memorandum and the governing documents of the \
                    relevant issuer.\n\n\
                    **Risk.** Investment in real-property direct-hold solutions involves \
                    significant risk, including possible loss of capital. Past performance is \
                    not indicative of future results. References to structural features such as \
                    advisory fees, transferability, and net asset value methodology describe the \
                    contractual terms of the direct-hold solutions and are not representations \
                    as to investment outcomes or returns.\n\n\
                    **Forward-looking statements.** Statements that are not historical facts may \
                    constitute forward-looking information within the meaning of applicable \
                    Canadian securities laws. Such statements are subject to known and unknown \
                    risks, uncertainties and assumptions, and actual results may differ \
                    materially. Woodfine undertakes no obligation to update such statements \
                    except as required by law.\n\n\
                    **Registration.** Registrable activities of Woodfine and its affiliates are \
                    conducted, where required, under the applicable registration categories \
                    prescribed by the British Columbia Securities Commission and other Canadian \
                    securities regulators. Specific registration details are available on \
                    request.\n\n\
                    **Changes to this notice.** Woodfine may update this notice from time to \
                    time; the version posted on this page governs.\n\n\
                    **Full disclaimer.** This notice supplements, and does not replace, the full \
                    Website Disclaimer at /page/disclaimer. In the event of any conflict, the \
                    full Website Disclaimer governs.",
                // Real human translation, ported verbatim from the retired
                // page.es.yaml prose section — not machine-translated here.
                body_es: "**Oferta de valores.** Woodfine Capital Projects Inc. \
                    (\u{201c}Woodfine\u{201d}) patrocina soluciones inmobiliarias de tenencia \
                    directa. Las participaciones en dichas soluciones se ofrecen únicamente a \
                    inversionistas que califiquen conforme a una exención de prospecto aplicable \
                    —incluida la exención de inversionista acreditado prevista en el Instrumento \
                    Nacional 45-106 — Exenciones de Prospecto— y a exenciones equivalentes en \
                    otras jurisdicciones aplicables. La información de esta página se proporciona \
                    únicamente con fines informativos generales y no constituye una oferta de \
                    venta ni una solicitud de oferta de compra de valor alguno. Toda oferta se \
                    realiza exclusivamente por medio del Memorando de Colocación Privada \
                    aplicable, que los posibles inversionistas deben revisar, junto con sus \
                    propios asesores profesionales, antes de invertir.\n\n\
                    **Alcance.** La información de esta página describe a Woodfine y sus \
                    actividades a alto nivel y queda calificada en su totalidad por el Memorando \
                    de Colocación Privada aplicable y por los documentos constitutivos del \
                    emisor correspondiente.\n\n\
                    **Riesgo.** La inversión en soluciones inmobiliarias de tenencia directa \
                    conlleva un riesgo significativo, incluida la posible pérdida de capital. El \
                    rendimiento pasado no es indicativo de resultados futuros. Las referencias a \
                    características estructurales tales como comisiones de asesoría, \
                    transferibilidad y metodología de valor neto de los activos describen los \
                    términos contractuales de las soluciones de tenencia directa y no \
                    constituyen declaraciones sobre resultados o rendimientos de la \
                    inversión.\n\n\
                    **Declaraciones prospectivas.** Las declaraciones que no sean hechos \
                    históricos pueden constituir información prospectiva en el sentido de la \
                    legislación de valores canadiense aplicable. Dichas declaraciones están \
                    sujetas a riesgos, incertidumbres y supuestos conocidos y desconocidos, y \
                    los resultados reales pueden diferir sustancialmente. Woodfine no asume \
                    obligación alguna de actualizar dichas declaraciones, salvo cuando lo exija \
                    la ley.\n\n\
                    **Registro.** Las actividades sujetas a registro de Woodfine y sus afiliadas \
                    se llevan a cabo, cuando así se requiera, bajo las categorías de registro \
                    aplicables prescritas por la British Columbia Securities Commission y otros \
                    reguladores de valores canadienses. Los detalles específicos de registro \
                    están disponibles a solicitud.\n\n\
                    **Cambios a este aviso.** Woodfine podrá actualizar este aviso \
                    periódicamente; rige la versión publicada en esta página.\n\n\
                    **Descargo completo.** Este aviso complementa, y no sustituye, el Descargo \
                    de responsabilidad completo del sitio web disponible en /page/disclaimer. \
                    En caso de cualquier conflicto, prevalece el Descargo de responsabilidad \
                    completo.",
            }],
            favicon_href: "/static/graphics/woodfine/favicon.svg",
            canonical_base: "https://home.woodfinegroup.com",
            og_site_name: "Woodfine Capital Projects",
            ld_json_type: "Organization",
            ld_json_description: "A real property developer with more than 35 years\u{2019} \
                experience in the procurement, development, and management of real property.",
        }
    }

    pub fn pointsav() -> Self {
        Self {
            module_id: "pointsav",
            site_title: "PointSav Digital Systems",
            wordmark_label: "PointSav Digital Systems",
            // Masthead/drawer nav = credibility layer. Documentation and
            // Software moved OUT of here (into the button row, the real
            // 1-tap mobile fast path) — they used to appear in BOTH the
            // masthead and the button row, an unflagged duplication FABLE's
            // nav-priority audit caught 2026-07-02.
            nav_links: vec![
                NavLink::external("Design System", "Sistema de diseño", "https://design.pointsav.com/"),
                // Restored 2026-07-02 — present on the retired production
                // site's masthead nav, dropped when this chrome was rebuilt.
                NavLink::external("Newsroom", "Sala de prensa", "https://pointsav.com/"),
                NavLink::internal("Contact Us", "Contáctenos", "/page/contact"),
                NavLink::internal("Disclaimer", "Aviso legal", "/page/disclaimer"),
            ],
            footer_nav: vec![
                NavLink::internal("Contact Us", "Contáctenos", "/page/contact"),
                NavLink::internal("Disclaimer", "Aviso legal", "/page/disclaimer"),
                NavLink::internal("Privacy", "Privacidad", "/page/privacy"),
            ],
            // Full external-destination list (masthead + button-row items
            // combined), same rationale as Woodfine's — FABLE nav-priority
            // audit 2026-07-02.
            footer_network: vec![
                NavLink::external("Documentation", "Documentación", "https://documentation.pointsav.com/"),
                NavLink::external("Software", "Software", "https://software.pointsav.com/"),
                NavLink::external("Design System", "Sistema de diseño", "https://design.pointsav.com/"),
                NavLink::external("Newsroom", "Sala de prensa", "https://pointsav.com/"),
                NavLink::external("Source", "Código fuente", "https://github.com/pointsav"),
            ],
            // Berlin dropped 2026-07-02 per operator call (production has it,
            // but the operator wants it off both sites going forward).
            cities: vec!["Vancouver", "New York"],
            copyright_holder: "Woodfine Capital Projects Inc.",
            // Full canonical roster per TRADEMARK.md, same as Woodfine's —
            // operator call 2026-07-02, superseding the earlier shorter-
            // subset design (architecture addendum 2026-06-24).
            trademark_line: "Woodfine Capital Projects\u{2122}, MCorp\u{2122}, \
                PointSav Digital Systems\u{2122}, Totebox Orchestration\u{2122}, Totebox \
                Archive\u{2122}, and Capability Geometry\u{2122} are trademarks of Woodfine \
                Capital Projects Inc., used in Canada, the United States, Latin America, and \
                Europe. All other trademarks are the property of their respective owners.",
            // Full legal text — moved here 2026-07-02 from a separate,
            // always-visible `type: prose` home-page section (same reason
            // as Woodfine's, see comment above).
            disclosure_slots: vec![DisclosureSlot {
                // Was "Product disclosure" — widened 2026-07-02 to "Company and
                // product disclosure" because the body now opens with the
                // corporate-structure and securities-boundary framing required
                // by factory-release-engineering/policies/DISCLAIMER.md ¶1 and
                // the "investment/technology boundary" rule in
                // tokens/legal-tokens-pointsav.yaml, not just product IP. Not
                // labelled "Securities disclosure" like Woodfine's: this page
                // explicitly does NOT offer securities, so that label would
                // overstate; "Company and product" states the true scope.
                //
                // Content rewrite 2026-07-02 (Opus agent): fixed a factually
                // broken "Intellectual property" product list (two invented
                // names, one internal-only ops codename, one misspelled mark
                // — verified against the real product catalog instead) and an
                // abbreviation inconsistency ("Woodfine" defined as shorthand
                // then not used). A canonical copy of this text has been
                // staged for factory-release-engineering as
                // policies/HOMEPAGE-DISCLAIMER-POINTSAV.md (admin-tier repo —
                // this session cannot commit there directly; handed off via
                // mailbox to Command Session). ui.rs is a derived copy, not
                // the source of truth — reconcile against the canonical file
                // once it lands.
                //
                // Hyperscaler-grade legal pass 2026-07-02 (Opus agent, hyperscaler-
                // law-firm-counsel brief, second review): the 4-item enumerated
                // IP product list from the first rewrite was ITSELF the wrong
                // pattern — a static list in an IP-ownership clause creates
                // expressio-unius exposure (implies un-listed products, of which
                // there are already 4+ more in the real catalog, sit outside the
                // asserted ownership scope) and requires a maintenance edit on
                // every product launch. Replaced with inclusive "all current and
                // future PointSav- and Totebox-branded products" scope language,
                // matching both the canonical HOMEPAGE-DISCLAIMER.md's own
                // pattern and how real hyperscalers (AWS/GCP/Azure) draft this
                // clause. Also added "Export compliance," "Changes to this
                // notice," and "Full disclaimer" — genuinely missing standard
                // clauses — and reconciled "offering memorandum" to the
                // canonical term "Private Placement Memorandum." One flagged
                // finding from that review was independently verified WRONG and
                // discarded: it claimed no Privacy page exists — false, see
                // `footer_nav` above and content/privacy — and was not acted on.
                // body_es is a fresh translation matching every change —
                // flagged for a native legal-translator verification pass
                // before this is treated as final.
                label: "Company and product disclosure",
                label_es: "Divulgación de la empresa y del producto",
                body: "**Corporate structure.** PointSav Digital Systems (\u{201c}PointSav\u{201d}) \
                    is a trade name of \
                    Woodfine Capital Projects Inc. (\u{201c}Woodfine\u{201d}). PointSav does not \
                    itself offer, sell, or solicit any security. Any securities offering \
                    associated with Woodfine\u{2019}s real-property direct-hold solutions is made \
                    exclusively by Woodfine, and only by means of the \
                    applicable Private Placement Memorandum.\n\n\
                    **No investment advice.** PointSav software and documentation are provided for \
                    operational, research, and development purposes. Nothing on this site \
                    constitutes investment advice or a solicitation to invest in any Woodfine \
                    partnership or direct-hold solution.\n\n\
                    **Intellectual property.** The PointSav name, trade name, wordmark, and \
                    marks, together with all current and future PointSav- and Totebox-branded \
                    products, services, and offerings — and the software, source code, \
                    documentation, design system, and all related materials — are proprietary to \
                    Woodfine and its affiliates, except for components identified as open \
                    source. No rights are granted except as expressly set out in a written \
                    license or agreement.\n\n\
                    **Open source components.** Portions of the platform are made available \
                    under permissive open-source licenses identified in the accompanying \
                    repository. Use of those components is governed by their respective license \
                    terms.\n\n\
                    **No warranty; informational use.** Information on this page is provided for \
                    general informational purposes only and does not constitute a \
                    representation, warranty, or commitment with respect to product \
                    functionality, availability, pricing, or roadmap. Product descriptions \
                    describe intended capabilities; actual feature availability may vary by \
                    release and partner agreement.\n\n\
                    **Export compliance.** PointSav products include infrastructure and \
                    cryptographic software that may be subject to export-control and sanctions \
                    laws. Customers are responsible for complying with all applicable export, \
                    re-export, and import requirements.\n\n\
                    **Changes to this notice.** PointSav may update this notice from time to \
                    time; the version posted on this page governs.\n\n\
                    **Full disclaimer.** This notice supplements, and does not replace, the full \
                    Disclaimer at /page/disclaimer. In the event of any conflict, the full \
                    Disclaimer governs.",
                // Real human translation, ported verbatim from the retired
                // page.es.yaml prose section — not machine-translated here.
                body_es: "**Estructura corporativa.** PointSav Digital Systems \
                    (\u{201c}PointSav\u{201d}) es un nombre \
                    comercial de Woodfine Capital Projects Inc. (\u{201c}Woodfine\u{201d}). \
                    PointSav no ofrece, vende ni solicita por sí mismo valor alguno. Toda oferta \
                    de valores asociada a las soluciones inmobiliarias de tenencia directa de \
                    Woodfine se realiza exclusivamente por parte de Woodfine, y únicamente por \
                    medio del Memorando de Colocación Privada aplicable.\n\n\
                    **Sin asesoría de inversión.** El software y la documentación de PointSav se \
                    proporcionan con fines operativos, de investigación y de desarrollo. Nada en \
                    este sitio constituye asesoría de inversión ni una solicitud para invertir \
                    en ninguna sociedad o solución de tenencia directa de Woodfine.\n\n\
                    **Propiedad intelectual.** El nombre, el nombre comercial, el logotipo \
                    (wordmark) y las marcas de PointSav, junto con todos los productos, \
                    servicios y ofertas actuales y futuros de las marcas PointSav y Totebox \
                    —así como el software, el código fuente, la documentación, el sistema de \
                    diseño y todos los materiales relacionados— son propiedad de Woodfine y sus \
                    afiliadas, salvo los componentes identificados como de código abierto. No se \
                    otorga derecho alguno salvo lo expresamente establecido en una licencia o \
                    acuerdo por escrito.\n\n\
                    **Componentes de código abierto.** Partes de la plataforma se ponen a \
                    disposición bajo licencias de código abierto permisivas identificadas en el \
                    repositorio correspondiente. El uso de dichos componentes se rige por sus \
                    respectivos términos de licencia.\n\n\
                    **Sin garantía; uso informativo.** La información de esta página se \
                    proporciona únicamente con fines informativos generales y no constituye una \
                    declaración, garantía ni compromiso respecto de la funcionalidad, \
                    disponibilidad, precio o hoja de ruta de los productos. Las descripciones de \
                    productos describen capacidades previstas; la disponibilidad real de \
                    funciones puede variar según la versión y el acuerdo con el socio.\n\n\
                    **Cumplimiento en materia de exportación.** Los productos de PointSav \
                    incluyen software de infraestructura y criptográfico que puede estar sujeto \
                    a leyes de control de exportaciones y de sanciones. Los clientes son \
                    responsables de cumplir con todos los requisitos aplicables de exportación, \
                    reexportación e importación.\n\n\
                    **Cambios a este aviso.** PointSav podrá actualizar este aviso \
                    periódicamente; rige la versión publicada en esta página.\n\n\
                    **Descargo completo.** Este aviso complementa, y no sustituye, el Descargo \
                    de responsabilidad completo disponible en /page/disclaimer. En caso de \
                    cualquier conflicto, prevalece el Descargo de responsabilidad completo.",
            }],
            favicon_href: "/static/graphics/pointsav/favicon.svg",
            canonical_base: "https://home.pointsav.com",
            og_site_name: "PointSav Digital Systems",
            ld_json_type: "SoftwareApplication",
            ld_json_description: "A fully transferable data management platform for the \
                procurement, development, and management of real properties.",
        }
    }

    pub fn by_module_id(id: &str) -> Self {
        match id {
            "pointsav" => Self::pointsav(),
            _ => Self::woodfine(),
        }
    }
}

fn render_nav(links: &[NavLink], class: &str, aria_label: &str, lang: &str) -> Markup {
    let new_tab_suffix = t(lang, " (opens in new tab)", " (se abre en una pestaña nueva)");
    html! {
        nav class=(class) aria-label=(aria_label) {
            @for link in links {
                @if link.external {
                    // Visual external-link glyph, not just an aria-label suffix
                    // (FABLE competitive-benchmark audit 2026-07-02: "a mobile
                    // visitor can't tell on-site pages from off-site network
                    // jumps" — sighted users had no indicator at all before this).
                    // Same ↗ glyph already used on card_link for consistency.
                    a href=(link.href) target="_blank" rel="noopener"
                        aria-label={ (link.label_for(lang)) (new_tab_suffix) } {
                        (link.label_for(lang))
                        span.m-nav__external-glyph aria-hidden="true" { "\u{2197}" }
                    }
                } @else {
                    a href=(link.href) { (link.label_for(lang)) }
                }
            }
        }
    }
}

fn masthead(tenant: &Tenant, lang: &str) -> Markup {
    let nav_landmark = t(lang, "Primary", "Principal");
    let open_menu = t(lang, "Open menu", "Abrir menú");
    html! {
        header.m-masthead {
            a.m-masthead__wordmark href="/" aria-label=(tenant.wordmark_label) {
                (tenant.site_title)
            }
            (render_nav(&tenant.nav_links, "m-masthead__nav", nav_landmark, lang))
            button.m-masthead__burger
                type="button"
                aria-label=(open_menu)
                aria-expanded="false"
                aria-controls="m-drawer"
                data-m-drawer-toggle {
                span.m-masthead__burger-bar {}
                span.m-masthead__burger-bar {}
                span.m-masthead__burger-bar {}
            }
        }
    }
}

fn drawer(tenant: &Tenant, lang: &str) -> Markup {
    let dialog_label = t(lang, "Site navigation", "Navegación del sitio");
    let close_menu = t(lang, "Close menu", "Cerrar menú");
    let nav_landmark = t(lang, "Mobile", "Móvil");
    html! {
        div.m-drawer-scrim data-m-drawer-scrim {}
        div #m-drawer .m-drawer role="dialog" aria-modal="true" aria-label=(dialog_label) hidden {
            div.m-drawer__header {
                span { (tenant.site_title) }
                button.m-drawer__close type="button" aria-label=(close_menu) data-m-drawer-toggle {
                    "\u{00d7}"
                }
            }
            (render_nav(&tenant.nav_links, "m-drawer__nav", nav_landmark, lang))
        }
    }
}

fn footer(tenant: &Tenant, lang: &str) -> Markup {
    let site_col_title = t(lang, "Site", "Sitio");
    let network_col_title = t(lang, "Network", "Red");
    let footer_landmark = t(lang, "Footer", "Pie de página");
    let network_landmark = t(lang, "Network", "Red");
    html! {
        footer.m-footer {
            div.m-footer__columns {
                div.m-footer__col {
                    p.m-footer__col-title { (site_col_title) }
                    (render_nav(&tenant.footer_nav, "m-footer__nav", footer_landmark, lang))
                }
                // Network column — the external off-site links duplicated from
                // the masthead nav so they're reachable from the footer without
                // opening the hamburger drawer (wiki footer pattern).
                @if !tenant.footer_network.is_empty() {
                    div.m-footer__col {
                        p.m-footer__col-title { (network_col_title) }
                        (render_nav(&tenant.footer_network, "m-footer__nav", network_landmark, lang))
                    }
                }
            }
            @if !tenant.disclosure_slots.is_empty() {
                // Collapsed by default — matches the retired site's
                // "IMPORTANT INFORMATION ▾" accordion. The content still
                // ships on every page load (no JS required to read it,
                // satisfying "clear and prominent"); collapsing it just
                // stops it from reading as a second copy of the trademark
                // paragraph sitting directly below.
                details.m-footer__disclosure {
                    summary.m-footer__disclosure-summary { (t(lang, "Important information", "Información importante")) }
                    @for slot in &tenant.disclosure_slots {
                        div.m-footer__slot {
                            p.m-footer__slot-label { (slot.label_for(lang)) }
                            div.m-footer__slot-body { (PreEscaped(crate::content::render_markdown(slot.body_for(lang)))) }
                        }
                    }
                }
            }
            div.m-footer__base {
                div.m-footer__meta {
                    div.m-footer__cities {
                        @for (i, city) in tenant.cities.iter().enumerate() {
                            @if i > 0 { span aria-hidden="true" { " | " } }
                            span { (city) }
                        }
                    }
                    p.m-footer__copyright {
                        // Matches canonical TRADEMARK.md ("Copyright © 2026 Woodfine
                        // Capital Projects Inc.") — not the "2011–2026" range the old,
                        // now-retired production engine renders; that range has no
                        // founding-year record in the DataGraph and isn't in the
                        // canonical doc, so it isn't carried into this rewrite
                        // (flagged 2026-07-02, reconciled in favor of TRADEMARK.md).
                        "\u{00a9} 2026 " (tenant.copyright_holder) " " (t(lang, "All rights reserved.", "Todos los derechos reservados."))
                    }
                    // Persistent one-line disclaimer — always visible regardless of
                    // whether the "Important information" accordion above is open or
                    // collapsed, so a screenshot or print of the page is never bare
                    // of any disclosure at all. Same wording register the sibling
                    // knowledge wikis already ship (project-knowledge relay,
                    // 2026-07-02, "Apollo Academy" pattern) — one legal voice across
                    // the site family. Only rendered if there's an accordion above to
                    // point to.
                    @if !tenant.disclosure_slots.is_empty() {
                        p.m-footer__notice {
                            (t(
                                lang,
                                "Provided for information only — not an offer, solicitation, or advice. See Important information above.",
                                "Proporcionado únicamente con fines informativos — no constituye una oferta, solicitud ni asesoramiento. Consulte Información importante arriba.",
                            ))
                        }
                    }
                    // Trademark line: same pending-professional-translation note as the
                    // disclosure slots above — verbatim legal text, not machine-localized.
                    p.m-footer__trademark { (tenant.trademark_line) }
                }
                div.m-footer__badges {
                    a.m-badge href="/page/mediakit" {
                        span.m-badge__glyph aria-hidden="true" {
                            svg viewBox="0 0 24 24" width="15" height="15" {
                                path fill="currentColor"
                                    d="M3 5.5A1.5 1.5 0 0 1 4.5 4h15A1.5 1.5 0 0 1 21 5.5v13A1.5 1.5 0 0 1 19.5 20h-15A1.5 1.5 0 0 1 3 18.5v-13zM6 8v8l3.2-2.4L6 8zm7 6.5h5V13h-5v1.5zm0-3h5V10h-5v1.5z" {}
                            }
                        }
                        span.m-badge__text {
                            span.m-badge__label { (t(lang, "Powered by", "Desarrollado con")) }
                            span.m-badge__name { "MediaKit" }
                        }
                    }
                }
            }
        }
    }
}

fn hero(section_headline: &str, section_subhead: Option<&str>) -> Markup {
    html! {
        section.m-hero {
            div.m-hero__inner {
                h1.m-hero__headline { (section_headline) }
                @if let Some(sub) = section_subhead {
                    p.m-hero__subhead { (sub) }
                }
            }
        }
    }
}

fn is_external(href: &str) -> bool {
    href.starts_with("http://") || href.starts_with("https://")
}

fn is_github(href: &str) -> bool {
    href.contains("github.com")
}

/// Minimal GitHub mark (the widely-used open-source "octocat" glyph) —
/// decorative only, `aria-hidden` since the button label already says
/// "Manifest"/"Source". Added 2026-07-02 so GitHub-bound buttons are
/// recognizable at a glance instead of reading identically to every other
/// button-row link.
fn github_icon() -> Markup {
    html! {
        svg.m-card__github-icon viewBox="0 0 16 16" aria-hidden="true" {
            path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 \
                0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 \
                1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 \
                0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 \
                1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 \
                3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 \
                8.013 0 0016 8c0-4.42-3.58-8-8-8z";
        }
    }
}

/// Renders a link consistently with the masthead/drawer convention: external
/// links get `target=_blank rel=noopener` plus an "(opens in new tab)"
/// aria-label suffix; internal links are plain. GitHub-bound links also get
/// the octocat mark ahead of the label.
fn card_link(href: &str, label: &str) -> Markup {
    html! {
        @if is_external(href) {
            a href=(href) target="_blank" rel="noopener" aria-label={ (label) " (opens in new tab)" } {
                @if is_github(href) {
                    (github_icon())
                }
                (label)
                span.m-card__link-glyph aria-hidden="true" { "\u{2197}" }
            }
        } @else {
            a href=(href) { (label) }
        }
    }
}

fn card_grid(columns: u8, cards: &[crate::content::Card], style: Option<&str>) -> Markup {
    let is_buttons = style == Some("buttons");
    let grid_class = if is_buttons {
        "m-card-grid m-card-grid--buttons"
    } else {
        "m-card-grid"
    };
    // `--m-button-count` (the real card count, not the manifest's `columns`
    // hint) drives the button row's own 1-col/N-col fix — see
    // .m-card-grid--buttons in app.css. Only set for the buttons style;
    // the plain informational grid doesn't need it (its fluid auto-fit
    // columns were never the orphan-prone kind).
    let grid_style = if is_buttons {
        format!("--m-grid-cols: {columns}; --m-button-count: {}", cards.len())
    } else {
        format!("--m-grid-cols: {columns}")
    };
    // Progressive disclosure (2026-07-07 mobile redesign): only the plain
    // informational grid ever needs this — button rows are short
    // navigation lists, never long enough to warrant collapsing. Ships
    // fully visible with a `hidden` reveal button; app.js only activates
    // it below 500px (round 11: the grid renders 2 columns from ~497px up,
    // fitting all cards without collapse), and only ever hides cards
    // beyond the first 4 that
    // AREN'T a cross-site linked card (Digital Systems / Real Property
    // Infrastructure stay visible unconditionally — they're navigation to
    // the sibling site, not informational content to defer).
    const VISIBLE_COUNT: usize = 4;
    let collapsible = !is_buttons && cards.len() > VISIBLE_COUNT;
    html! {
        section class=(grid_class) style=(grid_style) {
            @for (i, card) in cards.iter().enumerate() {
                @let linked = card.href.is_some();
                @let card_class = match (is_buttons, linked) {
                    (true, _) => "m-card m-card--button",
                    (false, true) => "m-card m-card--linked",
                    (false, false) => "m-card",
                };
                @let is_extra = collapsible && i >= VISIBLE_COUNT && !linked;
                div class=(card_class) data-m-card-extra[is_extra] {
                    @if linked && !is_buttons {
                        // Cross-site handoff kicker — see .m-card--linked in
                        // app.css for why this card is styled to stand out
                        // rather than blend in with the seven around it.
                        p.m-card__kicker {
                            svg.m-card__kicker-icon viewBox="0 0 24 24" aria-hidden="true" {
                                path fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"
                                    d="M9 3H4a1 1 0 00-1 1v16a1 1 0 001 1h16a1 1 0 001-1v-5M14 3h7v7M21 3l-9 9";
                            }
                            "Also part of the family"
                        }
                    }
                    h2.m-card__title {
                        @if let Some(href) = &card.href {
                            (card_link(href, &card.title))
                        } @else {
                            (card.title.clone())
                        }
                    }
                    @if let Some(body) = &card.body {
                        p.m-card__body { (body) }
                    }
                }
            }
            @if collapsible {
                // "View all" + inline down-chevron (round 11): institutional
                // reference sites (Blackstone "Load More", Brookfield "See
                // All News", Digital Realty "See More", Equinix "View all")
                // use a short verb phrase with a directional glyph, not a
                // "Show all N noun" construction. The chevron signals in-place
                // expansion. Chevron matches the .m-card__kicker-icon stroke
                // convention (viewBox 0 0 24 24, stroke-width 1.6, round caps).
                // No rotation logic is needed: app.js removes the button
                // outright once the cards are revealed, so the glyph is static.
                button type="button" class="m-card-grid__more" data-m-card-grid-more hidden {
                    "View all"
                    svg.m-card-grid__more-icon viewBox="0 0 24 24" aria-hidden="true" {
                        path fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"
                            d="M6 9l6 6 6-6";
                    }
                }
            }
        }
    }
}

fn icon_strip(icons: &[crate::content::IconTile]) -> Markup {
    html! {
        section.m-icon-strip {
            // `--m-icon-count` drives the desktop full-row column count in
            // app.css. Only ever 1 column (mobile) or icons.len() columns
            // (desktop) — no in-between column count is used, because a
            // middle count only avoids an orphaned row when it evenly
            // divides the item count, and that's not true in general (3
            // icons has no clean 2-column split). See app.css for detail.
            div.m-icon-strip__inner style={ "--m-icon-count: " (icons.len()) } {
                @for icon in icons {
                    // 2026-07-07 mobile redesign, round 10 revision: caption
                    // beside the icon on mobile (<700px), below it on
                    // desktop/tablet (>=700px, see app.css). Either way the
                    // title is real visible text, not just invisible alt
                    // text, so the image itself is decorative (empty alt)
                    // to avoid a screen reader announcing the label twice.
                    div.m-icon-strip__item {
                        // No hardcoded width/height attributes (round 11):
                        // sizing is height-driven in CSS (.m-icon-strip__img)
                        // and each retrimmed SVG has its own native aspect
                        // ratio, so a fixed 200x200 square hint would be
                        // inaccurate and provoke a layout shift on load. The
                        // SVGs carry their own intrinsic size via viewBox.
                        img.m-icon-strip__img src=(icon.src) alt="" aria-hidden="true" loading="lazy";
                        div.m-icon-strip__text {
                            h3.m-icon-strip__title { (icon.alt) }
                            @if let Some(body) = &icon.body {
                                p.m-icon-strip__body { (body) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn prose(body: &str) -> Markup {
    let rendered = crate::content::render_markdown(body);
    html! {
        section.m-prose {
            (PreEscaped(rendered))
        }
    }
}

fn render_section(section: &Section, seen_h1: &mut bool) -> Markup {
    match section {
        Section::Hero { headline, subhead } => {
            let markup = if *seen_h1 {
                // WCAG: exactly one <h1> per page. A second hero section
                // (none currently exist, but content is data — guard anyway)
                // demotes to h2 rather than emitting a duplicate <h1>.
                html! {
                    section.m-hero {
                        div.m-hero__inner {
                            h2.m-hero__headline { (headline) }
                            @if let Some(sub) = subhead {
                                p.m-hero__subhead { (sub) }
                            }
                        }
                    }
                }
            } else {
                *seen_h1 = true;
                hero(headline, subhead.as_deref())
            };
            markup
        }
        Section::CardGrid { columns, cards, style } => {
            card_grid(*columns, cards, style.as_deref())
        }
        Section::Prose { body } => prose(body),
        Section::IconStrip { icons } => icon_strip(icons),
    }
}

/// Render a complete HTML document: skip-link + masthead + sections + footer
/// + drawer. No client-side bundler/template DOM-swap — fully server-rendered.
///
/// `en_path` is the page's English path (e.g. `/page/contact`). `es_path` is
/// its Spanish variant path if one is actually routed (`None` means this
/// page has no `/es` route — e.g. `home`, operator call 2026-07-02 — and no
/// `hreflang="es"`/`x-default` alternate is emitted for it). `google_verify`
/// comes from `SERVICE_MARKETING_GOOGLE_VERIFY` at startup, per-instance.
pub fn page_shell(
    tenant: &Tenant,
    page: &Page,
    module_id: &str,
    en_path: &str,
    es_path: Option<&str>,
    google_verify: Option<&str>,
) -> Markup {
    // Empty page.title (home pages only, by content-file convention) means
    // "this is the site's own front door" -- skip the "Page — Site" prefix
    // and show just the site name in the tab, matching standard practice
    // (operator feedback 2026-07-02: "Home —" read as redundant/uninformative
    // once the favicon already carries tenant identity). Same
    // empty-means-fall-back-to-tenant-default pattern already used for
    // page.description a few lines below.
    let page_title = if page.title.is_empty() {
        tenant.site_title.to_string()
    } else {
        format!("{} \u{2014} {}", page.title, tenant.site_title)
    };
    let self_path = if page.lang == "es" {
        es_path.unwrap_or(en_path)
    } else {
        en_path
    };
    let canonical_url = format!("{}{}", tenant.canonical_base, self_path);
    let en_url = format!("{}{}", tenant.canonical_base, en_path);
    let es_url = es_path.map(|p| format!("{}{}", tenant.canonical_base, p));
    let ld_description = if page.description.is_empty() {
        tenant.ld_json_description
    } else {
        page.description.as_str()
    };
    let ld_json = format!(
        r#"{{"@context":"https://schema.org","@type":"{}","name":"{}","url":"{}","description":"{}"}}"#,
        tenant.ld_json_type, tenant.og_site_name, tenant.canonical_base, ld_description,
    );
    let mut seen_h1 = false;
    html! {
        (DOCTYPE)
        html lang=(page.lang) data-brand=(module_id) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (page_title) }
                meta name="description" content=(page.description);
                link rel="canonical" href=(canonical_url);
                link rel="alternate" hreflang="en" href=(en_url);
                @if let Some(es_url) = &es_url {
                    link rel="alternate" hreflang="es" href=(es_url);
                }
                link rel="alternate" hreflang="x-default" href=(en_url);
                link rel="icon" type="image/svg+xml" href=(tenant.favicon_href);
                meta name="robots" content="index, follow";
                meta property="og:type" content="website";
                meta property="og:site_name" content=(tenant.og_site_name);
                meta property="og:title" content=(page_title);
                meta property="og:description" content=(page.description);
                meta property="og:url" content=(canonical_url);
                meta name="twitter:card" content="summary";
                meta name="twitter:title" content=(page_title);
                meta name="twitter:description" content=(page.description);
                script type="application/ld+json" { (PreEscaped(&ld_json)) }
                @if let Some(token) = google_verify {
                    meta name="google-site-verification" content=(token);
                }
                link rel="stylesheet" href="/static/tokens.css";
                link rel="stylesheet" href="/static/fonts.css";
                link rel="stylesheet" href="/static/app.css";
            }
            body {
                a.m-skiplink href="#m-main" { (t(&page.lang, "Skip to content", "Saltar al contenido")) }
                (masthead(tenant, &page.lang))
                main #m-main {
                    @for section in &page.sections {
                        (render_section(section, &mut seen_h1))
                    }
                }
                (footer(tenant, &page.lang))
                (drawer(tenant, &page.lang))
                script src="/static/app.js" {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::load_page;

    #[test]
    fn both_tenants_carry_the_full_canonical_trademark_roster() {
        // Both brands carry the identical canonical sentence per TRADEMARK.md
        // (operator call 2026-07-02, superseding the earlier shorter-subset
        // design for PointSav). "MCorp" is the correct mark, not "Woodfine
        // Management Corp"; "Capability Geometry" must be present on both.
        let w = Tenant::woodfine();
        let p = Tenant::pointsav();
        assert_eq!(w.trademark_line, p.trademark_line);
        for line in [w.trademark_line, p.trademark_line] {
            assert!(line.contains("MCorp"));
            assert!(line.contains("Capability Geometry"));
            assert!(line.contains("PointSav Digital Systems"));
            assert!(line.contains("Woodfine Capital Projects"));
            assert!(!line.contains("Woodfine Management Corp"));
        }
    }

    #[test]
    fn both_tenants_share_the_same_copyright_holder() {
        // Per TRADEMARK.md v1.1: the copyright holder is always Woodfine
        // Capital Projects Inc., even on the PointSav-branded site.
        let w = Tenant::woodfine();
        let p = Tenant::pointsav();
        assert_eq!(w.copyright_holder, "Woodfine Capital Projects Inc.");
        assert_eq!(p.copyright_holder, "Woodfine Capital Projects Inc.");
    }

    #[test]
    fn renders_exactly_one_h1_even_with_multiple_hero_sections() {
        let dir = tempfile::tempdir().unwrap();
        let page_dir = dir.path().join("home");
        std::fs::create_dir_all(&page_dir).unwrap();
        std::fs::write(
            page_dir.join("page.yaml"),
            r#"
title: Home
slug: home
description: Test.
sections:
  - type: hero
    headline: First
  - type: hero
    headline: Second
"#,
        )
        .unwrap();
        let page = load_page(dir.path(), "home", None).unwrap();
        let html = page_shell(&Tenant::woodfine(), &page, "woodfine", "/", Some("/es"), None).into_string();
        assert_eq!(html.matches("<h1").count(), 1);
        assert!(html.contains("<h2"));
    }

    #[test]
    fn page_shell_has_no_bundler_dom_swap_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let page_dir = dir.path().join("home");
        std::fs::create_dir_all(&page_dir).unwrap();
        std::fs::write(
            page_dir.join("page.yaml"),
            "title: Home\nslug: home\ndescription: Test.\nsections:\n  - type: hero\n    headline: Hi\n",
        )
        .unwrap();
        let page = load_page(dir.path(), "home", None).unwrap();
        let html = page_shell(&Tenant::woodfine(), &page, "woodfine", "/", Some("/es"), None).into_string();
        assert!(!html.contains("__bundler"));
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains(r#"lang="en""#));
        assert!(html.contains(r#"data-brand="woodfine""#));
    }

    #[test]
    fn masthead_has_no_search_bar() {
        // Per DESIGN-SYSTEM.md: marketing has no search corpus, so unlike
        // the wiki masthead there is deliberately no search input here.
        let html = masthead(&Tenant::woodfine(), "en").into_string();
        assert!(!html.contains(r#"type="search""#));
        assert!(!html.contains("role=\"search\""));
    }

    #[test]
    fn footer_badge_links_to_about() {
        let html = footer(&Tenant::woodfine(), "en").into_string();
        assert!(html.contains("Powered by"));
        assert!(html.contains("MediaKit"));
        assert!(html.contains(r#"href="/page/mediakit""#));
    }

    #[test]
    fn footer_badge_label_localizes_to_spanish() {
        let html = footer(&Tenant::woodfine(), "es").into_string();
        assert!(html.contains("Desarrollado con"));
        assert!(html.contains("MediaKit"));
    }

    #[test]
    fn nav_landmarks_have_distinct_aria_labels() {
        // axe-core landmark-unique: every <nav> needs a distinct accessible
        // name when more than one is present on a page.
        let masthead_nav = render_nav(&[], "m-masthead__nav", "Primary", "en").into_string();
        let footer_nav = render_nav(&[], "m-footer__nav", "Footer", "en").into_string();
        let drawer_nav = render_nav(&[], "m-drawer__nav", "Mobile", "en").into_string();
        assert!(masthead_nav.contains(r#"aria-label="Primary""#));
        assert!(footer_nav.contains(r#"aria-label="Footer""#));
        assert!(drawer_nav.contains(r#"aria-label="Mobile""#));
    }

    #[test]
    fn drawer_root_is_not_a_nav_element() {
        // axe-core aria-allowed-role: role="dialog" is not permitted on a
        // <nav> (a navigation landmark can't also be a dialog widget).
        let html = drawer(&Tenant::woodfine(), "en").into_string();
        assert!(html.contains(r#"role="dialog""#));
        assert!(!html.contains(r#"<nav id="m-drawer""#));
        assert!(!html.contains(r#"<nav #m-drawer"#));
    }

    #[test]
    fn nav_link_labels_localize_to_spanish() {
        let html = render_nav(&Tenant::woodfine().nav_links, "m-masthead__nav", "Primary", "es")
            .into_string();
        assert!(html.contains("Contáctenos"));
        assert!(!html.contains("Contact Us"));
    }

    #[test]
    fn card_titles_are_h2_not_h3() {
        // axe-core heading-order: card-grid follows the hero's h1 directly
        // with no intermediate h2, so card titles must be h2, not h3.
        let cards = vec![crate::content::Card {
            title: "Example".to_string(),
            body: None,
            href: None,
        }];
        let html = card_grid(4, &cards, None).into_string();
        assert!(html.contains("<h2"));
        assert!(!html.contains("<h3"));
    }

    #[test]
    fn card_grid_collapses_beyond_four_but_never_the_linked_card() {
        let mut cards: Vec<crate::content::Card> = (1..=7)
            .map(|i| crate::content::Card {
                title: format!("Term {i}"),
                body: Some("Body.".to_string()),
                href: None,
            })
            .collect();
        cards.push(crate::content::Card {
            title: "Digital Systems".to_string(),
            body: Some("Cross-site.".to_string()),
            href: Some("https://home.pointsav.com".to_string()),
        });
        let html = card_grid(4, &cards, None).into_string();
        // Reveal button present, ships hidden (no-JS-safe default).
        assert!(html.contains("data-m-card-grid-more"));
        // Round 11: static "View all" wording (no count) + inline chevron.
        assert!(html.contains("View all"));
        assert!(html.contains("m-card-grid__more-icon"));
        // Exactly 3 cards marked extra (Terms 5-7) — the linked 8th card,
        // despite being past the visible-count threshold, is never marked.
        assert_eq!(html.matches("data-m-card-extra").count(), 3);
    }

    #[test]
    fn card_grid_does_not_collapse_at_or_under_four_cards() {
        let cards: Vec<crate::content::Card> = (1..=4)
            .map(|i| crate::content::Card {
                title: format!("Term {i}"),
                body: None,
                href: None,
            })
            .collect();
        let html = card_grid(4, &cards, None).into_string();
        assert!(!html.contains("data-m-card-grid-more"));
        assert!(!html.contains("data-m-card-extra"));
    }

    #[test]
    fn button_style_cards_never_collapse_regardless_of_count() {
        let cards: Vec<crate::content::Card> = (1..=6)
            .map(|i| crate::content::Card {
                title: format!("Button {i}"),
                body: None,
                href: Some("https://example.com".to_string()),
            })
            .collect();
        let html = card_grid(6, &cards, Some("buttons")).into_string();
        assert!(!html.contains("data-m-card-grid-more"));
        assert!(html.contains("--m-button-count: 6"));
    }

    #[test]
    fn button_style_cards_get_button_class() {
        let cards = vec![crate::content::Card {
            title: "Manifest".to_string(),
            body: None,
            href: Some("https://example.com".to_string()),
        }];
        let html = card_grid(3, &cards, Some("buttons")).into_string();
        assert!(html.contains("m-card--button"));
        assert!(!html.contains("m-card--linked"));
    }

    #[test]
    fn linked_informational_card_gets_linked_class_not_button_class() {
        let cards = vec![crate::content::Card {
            title: "Digital Systems".to_string(),
            body: Some("cross-site link".to_string()),
            href: Some("https://home.pointsav.com".to_string()),
        }];
        let html = card_grid(4, &cards, None).into_string();
        assert!(html.contains("m-card--linked"));
        assert!(!html.contains("m-card--button"));
    }

    #[test]
    fn external_card_link_gets_new_tab_affordance() {
        let cards = vec![crate::content::Card {
            title: "Digital Systems".to_string(),
            body: None,
            href: Some("https://home.pointsav.com".to_string()),
        }];
        let html = card_grid(4, &cards, None).into_string();
        assert!(html.contains(r#"target="_blank""#));
        assert!(html.contains(r#"rel="noopener""#));
        assert!(html.contains("opens in new tab"));
    }

    #[test]
    fn internal_card_link_has_no_new_tab_affordance() {
        let cards = vec![crate::content::Card {
            title: "Contact".to_string(),
            body: None,
            href: Some("/page/contact".to_string()),
        }];
        let html = card_grid(3, &cards, Some("buttons")).into_string();
        assert!(!html.contains("target=\"_blank\""));
    }

    #[test]
    fn icon_strip_renders_visible_title_and_decorative_image() {
        let icons = vec![crate::content::IconTile {
            src: "/static/graphics/woodfine/class-1.svg".to_string(),
            alt: "Professional Centres".to_string(),
            body: Some("Test descriptor.".to_string()),
        }];
        let html = icon_strip(&icons).into_string();
        // Title is now visible text (h3), not just alt — image becomes
        // decorative (empty alt) since the visible title carries the same
        // information, avoiding a double screen-reader announcement.
        assert!(html.contains("Professional Centres"));
        assert!(html.contains("Test descriptor."));
        assert!(html.contains(r#"alt="""#));
        assert!(html.contains(r#"src="/static/graphics/woodfine/class-1.svg""#));
    }

    #[test]
    fn icon_strip_renders_without_optional_body() {
        let icons = vec![crate::content::IconTile {
            src: "/static/graphics/woodfine/class-1.svg".to_string(),
            alt: "Professional Centres".to_string(),
            body: None,
        }];
        let html = icon_strip(&icons).into_string();
        assert!(html.contains("Professional Centres"));
        assert!(!html.contains("m-icon-strip__body"));
    }
}
