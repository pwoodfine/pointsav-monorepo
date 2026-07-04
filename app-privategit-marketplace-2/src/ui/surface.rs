//! Per-surface identity for the software.pointsav.com chrome.
//!
//! Mirrors the wiki/marketing `Tenant` enum pattern (app-mediakit-knowledge-2
//! `src/ui/tenant.rs`) but the dimension of variation here is the *binary
//! surface*, not the brand — software.pointsav.com is always PointSav-brand.
//!
//! Only `Marketplace` is constructed. `app-privategit-source-2` serves no HTML
//! (verified in the token-reconciliation research §d: zero `text/html` responses
//! in that crate — it is a pure machine surface), so a `Source` variant would be
//! dead code and is intentionally omitted rather than stubbed with a dead arm.

/// A navigation entry rendered in the masthead sub-bar and the mobile drawer.
#[derive(Clone, Copy)]
pub struct NavLink {
    pub label: &'static str,
    pub href: &'static str,
}

/// The served surfaces of software.pointsav.com.
///
/// `Source` is intentionally absent — see the module docs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SoftwareSurface {
    Marketplace,
}

impl SoftwareSurface {
    /// Accessible label / wordmark text for the masthead.
    pub fn home_label(self) -> &'static str {
        match self {
            SoftwareSurface::Marketplace => "PointSav Software",
        }
    }

    /// Primary masthead sub-bar links (also drive the mobile drawer).
    ///
    /// Phase 3 (nav restructuring): stays a flat list — no dropdown/mega-menu is
    /// needed for a single, small, two-tier product catalog (the public storefront
    /// sells os-* products only; the earlier 4-family dropdown design from the
    /// original audit was corrected once the ratified three-path model was
    /// checked). `Downloads` used to anchor at `#downloads`, a section id that no
    /// longer exists now that the catalog groups by license tier (`#commercial`/
    /// `#fsl`) rather than free/paid status — points at the bare page instead.
    pub fn nav_links(self) -> &'static [NavLink] {
        match self {
            SoftwareSurface::Marketplace => &[
                NavLink {
                    label: "Products",
                    href: "/software",
                },
                NavLink {
                    label: "Pricing",
                    href: "/pricing",
                },
                NavLink {
                    label: "Licensing",
                    href: "/licensing",
                },
                NavLink {
                    label: "Documentation",
                    href: "https://documentation.pointsav.com/",
                },
            ],
        }
    }

    /// The marketplace carries an account / license-status control; a source
    /// surface would not. Kept as a method so `Source` can diverge without forking.
    pub fn show_account_nav(self) -> bool {
        match self {
            SoftwareSurface::Marketplace => true,
        }
    }

    /// Verbatim trademark line — the actual canonical mark list from `TRADEMARK.md`
    /// (repo root): `PointSav™`, `Foundry™`, `ToteboxOS™`, `ConsoleOS™`,
    /// `OrchestrationOS™`, `WorkplaceOS™`, `WoodfineGroup™`. **Correction, 2026-07-04
    /// live-site audit:** a 2026-07-02 change here (msg-id
    /// `command-20260702-important-information-footer-structure-w`) replaced the
    /// original PointSav-only subset with a *different* six-mark set
    /// ("Woodfine Capital Projects™, MCorp™, PointSav Digital Systems™, Totebox
    /// Orchestration™, Totebox Archive™, Capability Geometry™") that its own doc
    /// comment claimed was "per the current TRADEMARK.md" — `git log -- TRADEMARK.md`
    /// shows that file has never contained any of those six strings. Several of them
    /// read as internal Foundry-workspace session/architecture vocabulary (see
    /// `AGENT.md` §11), not product trademarks. This restores the line to what
    /// `TRADEMARK.md` actually says. Do not paraphrase or shorten again without
    /// reading `TRADEMARK.md` directly — a claimed citation is not a substitute for
    /// checking the file.
    pub fn trademark_line(self) -> &'static str {
        "PointSav\u{2122}, Foundry\u{2122}, ToteboxOS\u{2122}, ConsoleOS\u{2122}, \
         OrchestrationOS\u{2122}, WorkplaceOS\u{2122}, and WoodfineGroup\u{2122} \
         are trademarks of Woodfine Capital Projects Inc. and Woodfine Management Corp. \
         All other trademarks are the property of their respective owners."
    }

    /// Copyright holder — the parent company, for every surface.
    pub fn copyright_holder(self) -> &'static str {
        "Woodfine Capital Projects Inc."
    }

    /// Office cities for the footer line (BRIEF footer anatomy).
    pub fn cities(self) -> &'static [&'static str] {
        &["Vancouver", "New York", "Berlin"]
    }

    /// Label for the single "Important information" disclosure slot in the
    /// footer accordion (see `layout::footer`). Matches the
    /// `app-mediakit-marketing-2` `DisclosureSlot` pattern (operator-directed
    /// 2026-07-02, "use the current footer setup like on the wiki/home
    /// sites") — a native `<details>` accordion, collapsed by default,
    /// on-page rather than hidden behind a link ("clear and prominent"),
    /// containing the one disclosure specific to what this site actually
    /// does: sell software licenses paid for in on-chain USDC.
    ///
    /// Supersedes the Checkpoint-3a `disclaimer_citation()` fix (2026-07-02)
    /// — that fix restored a citation line pointing at
    /// `factory-release-engineering/policies/DISCLAIMER.md`, an LP
    /// investment-offering document not applicable to a software
    /// marketplace. This site now has its own self-contained disclaimer
    /// page (`/page/disclaimer`, see `ui::disclaimer`) instead of citing
    /// someone else's.
    pub fn disclosure_label(self) -> &'static str {
        "Payment and licensing disclosure"
    }
}
