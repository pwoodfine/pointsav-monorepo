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

    /// Verbatim trademark line.
    ///
    /// **Corrected 2026-07-07** — the 2026-07-04 "correction" documented in this
    /// function's prior version was itself wrong: it claimed `git log --
    /// TRADEMARK.md` showed the six-mark set it was replacing "has never contained
    /// any of those six strings." That check was mistaken. `TRADEMARK.md` was
    /// rewritten to the "Woodfine Capital Projects™, Woodfine Management Corp™,
    /// PointSav Digital Systems™, Totebox Orchestration™, Totebox Archive™" canonical
    /// notice format on **2026-05-16** (commit `925eaee`), and gained
    /// **Capability Geometry™** as a sixth mark on **2026-06-19** (commit `86f8b65`)
    /// — both well before the 2026-07-04 session ran its check, and unchanged since.
    /// Verified directly against the current file (`vendor/factory-release-
    /// engineering/TRADEMARK.md` §13, the canonical-notice section) — the prior
    /// seven-mark set here (`PointSav™, Foundry™, ToteboxOS™, ConsoleOS™,
    /// OrchestrationOS™, WorkplaceOS™, WoodfineGroup™`) has **never** appeared in
    /// `TRADEMARK.md` at any point since the April 2026 original; it predates even
    /// the May rewrite. The exact wording below (using the informal `MCorp™`
    /// abbreviation for Woodfine Management Corp, and folding Capability Geometry's
    /// unregistered-mark note into the single sentence rather than a separate one)
    /// matches `home.pointsav.com`'s real, live, served trademark line verbatim —
    /// checked directly against its HTML, not assumed. Do not paraphrase or shorten
    /// again without reading `TRADEMARK.md` §13 directly AND cross-checking a live
    /// family site — a claimed citation is not a substitute for checking either.
    pub fn trademark_line(self) -> &'static str {
        "Woodfine Capital Projects\u{2122}, MCorp\u{2122}, PointSav Digital Systems\u{2122}, \
         Totebox Orchestration\u{2122}, Totebox Archive\u{2122}, and Capability Geometry\u{2122} \
         are trademarks of Woodfine Capital Projects Inc., used in Canada, the United States, \
         Latin America, and Europe. All other trademarks are the property of their respective \
         owners."
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
