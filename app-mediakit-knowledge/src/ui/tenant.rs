// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Per-instance identity. The three wikis share one structure and differ only
//! by brand accent (CSS, via `[data-instance]`) and these factual strings.
//!
//! Legal strings (entity, seat, trademark, copyright) are verbatim facts, not
//! design — they must match the workspace record (TRADEMARK.md). The copyright
//! holder is Woodfine Capital Projects Inc. for every instance.

/// A sibling wiki surfaced in the sitenotice and footer.
#[derive(Clone, Copy)]
pub struct SiblingWiki {
    pub label: &'static str,
    pub url: &'static str,
}

/// One of the three served instances.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tenant {
    Documentation, // documentation.pointsav.com  — PointSav,  accent #1a4480
    Projects,      // projects.woodfinegroup.com   — Woodfine, accent #164679
    Corporate,     // corporate.woodfinegroup.com  — Woodfine, accent #164679
}

impl Tenant {
    /// Parse from the `[site].instance` config value; defaults to Documentation.
    pub fn from_instance(s: Option<&str>) -> Self {
        match s {
            Some("projects") => Tenant::Projects,
            Some("corporate") => Tenant::Corporate,
            _ => Tenant::Documentation,
        }
    }

    /// Value for the `data-instance` attribute on `<html>`.
    pub fn instance_str(&self) -> &'static str {
        match self {
            Tenant::Documentation => "documentation",
            Tenant::Projects => "projects",
            Tenant::Corporate => "corporate",
        }
    }

    /// Brand accent hex — CSS drives accent from `[data-instance]`; this is for
    /// `<meta name="theme-color">` only.
    pub fn accent(&self) -> &'static str {
        match self {
            Tenant::Documentation => "#1a4480",
            Tenant::Projects | Tenant::Corporate => "#164679",
        }
    }

    pub fn is_woodfine(&self) -> bool {
        matches!(self, Tenant::Projects | Tenant::Corporate)
    }

    /// The canonical Organization `@id` this instance's JSON-LD `publisher`/
    /// `author` fields reference, per the cross-site SEO standard
    /// (project-editorial's `BRIEF-seo-cross-site-strategy.md`): every
    /// property references the brand's apex-domain Organization node by
    /// `@id` rather than each declaring its own inline copy — the same split
    /// as `is_woodfine()`, deliberately, so the two never drift apart.
    pub fn organization_id(&self) -> &'static str {
        match self {
            Tenant::Documentation => "https://pointsav.com/#organization",
            Tenant::Projects | Tenant::Corporate => "https://woodfinegroup.com/#organization",
        }
    }

    /// Maintaining entity of record (uppercased in the sitenotice by CSS).
    pub fn entity_name(&self) -> &'static str {
        match self {
            Tenant::Documentation => "PointSav Digital Systems",
            // The home.woodfinegroup.com link carries the parent brand.
            Tenant::Projects | Tenant::Corporate => "Woodfine Capital Projects",
        }
    }

    /// Reporting-issuer entity for the record — used as the History/Diff author
    /// (no natural person is rendered; the EDGAR/SEDAR signatory/preparer split
    /// keeps editor identity internal to signed git commits). PointSav is treated
    /// as its own public company; the Woodfine tenants attribute to the parent
    /// reporting issuer.
    /// Only the documentation instance serves GUIDEs (how-to runbooks); projects
    /// and corporate are TOPIC-only, so their guide affordances are suppressed.
    pub fn serves_guides(&self) -> bool {
        matches!(self, Tenant::Documentation)
    }

    pub fn issuer(&self) -> &'static str {
        match self {
            Tenant::Documentation => "PointSav Digital Systems",
            Tenant::Projects | Tenant::Corporate => "Woodfine Capital Projects Inc.",
        }
    }

    /// Content licence name for the footer badge (per-tenant).
    pub fn license_name(&self) -> &'static str {
        match self {
            Tenant::Documentation => "CC BY 4.0", // open engineering library
            Tenant::Projects | Tenant::Corporate => "CC BY-ND 4.0", // verbatim disclosure record
        }
    }

    /// Content licence deed URL for the footer badge (per-tenant).
    pub fn license_url(&self) -> &'static str {
        match self {
            Tenant::Documentation => "https://creativecommons.org/licenses/by/4.0/",
            Tenant::Projects | Tenant::Corporate => "https://creativecommons.org/licenses/by-nd/4.0/",
        }
    }

    /// Whether the licence carries the No-Derivatives term (adds the ND badge icon).
    pub fn license_nd(&self) -> bool {
        matches!(self, Tenant::Projects | Tenant::Corporate)
    }

    /// Persistent one-line disclaimer for the footer base row (always visible).
    pub fn disclaimer_line(&self) -> &'static str {
        "Provided for information only — not an offer, solicitation, or advice. See Important Information."
    }

    /// Registered seat of the maintaining entity.
    pub fn seat(&self) -> &'static str {
        match self {
            Tenant::Documentation => "Vancouver, British Columbia",
            Tenant::Projects | Tenant::Corporate => "Toronto, Ontario",
        }
    }

    /// Home URL of this instance's marketing site (logo link target).
    pub fn home_url(&self) -> &'static str {
        match self {
            Tenant::Documentation => "https://documentation.pointsav.com/",
            Tenant::Projects => "https://projects.woodfinegroup.com/",
            Tenant::Corporate => "https://corporate.woodfinegroup.com/",
        }
    }

    /// Accessible label / wordmark text (brand + descriptor combined).
    pub fn home_label(&self) -> &'static str {
        match self {
            Tenant::Documentation => "PointSav Documentation",
            Tenant::Projects => "Woodfine Projects",
            Tenant::Corporate => "Woodfine Corporate",
        }
    }

    /// Brand word for the wordmark lockup (display face).
    pub fn brand_word(&self) -> &'static str {
        match self {
            Tenant::Documentation => "PointSav",
            Tenant::Projects | Tenant::Corporate => "Woodfine",
        }
    }

    /// Descriptor for the wordmark lockup (sans, lighter — the wiki's role).
    pub fn descriptor(&self) -> &'static str {
        match self {
            Tenant::Documentation => "Documentation",
            Tenant::Projects => "Projects",
            Tenant::Corporate => "Corporate",
        }
    }

    /// Sibling wiki cross-link. Org boundary: PointSav documentation (vendor)
    /// stands alone; the two Woodfine (customer) wikis cross-link to each other.
    pub fn sibling_wiki(&self) -> Option<SiblingWiki> {
        match self {
            Tenant::Documentation => None,
            Tenant::Projects => Some(SiblingWiki {
                label: "Corporate record",
                url: "https://corporate.woodfinegroup.com/",
            }),
            Tenant::Corporate => Some(SiblingWiki {
                label: "Projects record",
                url: "https://projects.woodfinegroup.com/",
            }),
        }
    }

    /// Copyright holder — the parent company, for every instance.
    pub fn copyright_holder(&self) -> &'static str {
        "Woodfine Capital Projects Inc."
    }

    /// The marketing home site (the "Home" cross-property link).
    pub fn marketing_home(&self) -> &'static str {
        match self {
            Tenant::Documentation => "https://home.pointsav.com/",
            Tenant::Projects | Tenant::Corporate => "https://home.woodfinegroup.com/",
        }
    }

    /// Cross-property links for the top utility strip — mirrors the marketing
    /// site's right-hand nav exactly. `(label, url)`, self-links omitted.
    /// (No separate GitHub entry — the marketing "Monorepo" link already leads
    /// to the source-of-record property, so a GitHub link would be redundant.)
    pub fn cross_property_links(&self) -> Vec<(&'static str, &'static str)> {
        match self {
            // "Home" is intentionally omitted — the entity label links to the
            // corporate home, and the wiki logo links to this wiki's front page.
            // Listing a third "Home" here conflates the two.
            // Property links — these sit in the top strip AND the footer.
            Tenant::Documentation => vec![
                ("GitHub", "https://github.com/pointsav/pointsav-monorepo"),
                ("Software", "https://software.pointsav.com/"),
                ("Design System", "https://design.pointsav.com/"),
            ],
            Tenant::Projects => vec![
                ("Corporate", "https://corporate.woodfinegroup.com/"),
                ("Newsroom", "https://newsroom.woodfinegroup.com/"),
                ("GitHub", "https://github.com/woodfine/woodfine-fleet-deployment"),
            ],
            Tenant::Corporate => vec![
                ("Projects", "https://projects.woodfinegroup.com/"),
                ("Newsroom", "https://newsroom.woodfinegroup.com/"),
                ("GitHub", "https://github.com/woodfine/woodfine-fleet-deployment"),
            ],
        }
    }

    /// The cross-company org link surfaced in the footer network only
    /// (`(label, url)`): PointSav ↔ Woodfine.
    pub fn other_org(&self) -> (&'static str, &'static str) {
        match self {
            Tenant::Documentation => ("Woodfine Capital Projects", "https://home.woodfinegroup.com/"),
            Tenant::Projects | Tenant::Corporate => {
                ("PointSav Digital Systems", "https://home.pointsav.com/")
            }
        }
    }

    /// Office cities for the footer line — mirrors the marketing footer.
    pub fn cities(&self) -> &'static [&'static str] {
        &["Vancouver", "New York"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organization_id_matches_is_woodfine_split() {
        for t in [Tenant::Documentation, Tenant::Projects, Tenant::Corporate] {
            assert_eq!(t.is_woodfine(), t.organization_id().contains("woodfinegroup.com"));
        }
        assert_eq!(Tenant::Documentation.organization_id(), "https://pointsav.com/#organization");
        assert_eq!(Tenant::Projects.organization_id(), "https://woodfinegroup.com/#organization");
        assert_eq!(Tenant::Corporate.organization_id(), "https://woodfinegroup.com/#organization");
    }
}
