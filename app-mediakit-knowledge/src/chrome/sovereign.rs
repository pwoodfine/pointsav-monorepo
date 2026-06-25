//! Sovereign Editorial chrome — unified page shell for all three wiki instances.
//!
//! Consolidates four legacy chrome paths (home_handlers, wiki_handlers,
//! misc_handlers, chrome/mod.rs) into two reusable primitives:
//!   sovereign_nav()    — dark navy #164679 masthead, wordmark left, search centre, controls right
//!   sovereign_footer() — near-black #0e1117 footer, three-column, correct WCP Inc. legal text
//!   sovereign_page()   — full document wrapper for simple cases (no locale alternates, no JSON-LD)
//!
//! Per-tenant variation is dispatched through the Tenant enum; structural markup
//! is identical. Sites start similar and diverge via Tenant methods — never by forking.

use maud::{html, Markup, PreEscaped, DOCTYPE};

// ── Wordmark SVGs ─────────────────────────────────────────────────────────────
// Moved from server/home_handlers.rs. CSS applies filter: brightness(0) invert(1)
// on .s-topnav .s-wordmark .logo-svg to render white-on-navy.
const WORDMARK_SVG_WOODFINE: &str = r##"<svg class="logo-svg" aria-label="Woodfine Capital Projects" role="img" viewBox="0 0 144 36" width="320" height="80" version="1.1" xmlns="http://www.w3.org/2000/svg"><g class="institutional-fill" fill="#111827" fill-rule="evenodd"><path d="M 16.069 29.8287 L 16.069 31.0989 L 26.233 31.0989 L 26.233 29.8287 Z"></path><path d="M 118.051 29.8287 L 118.051 31.0989 L 128.216 31.0989 L 128.216 29.8287 Z"></path><path d="M 19.7019 23.1944 L 23.3651 23.1944 L 26.3901 9.48497 L 26.4456 9.48497 L 29.2485 23.1944 L 32.9395 23.1944 L 37.3244 3.15753 L 33.7444 3.15753 L 31.0802 16.2565 L 31.0247 16.2565 L 28.2495 3.15753 L 24.5585 3.15753 L 21.811 16.2565 L 21.7555 16.2565 L 19.0636 3.15753 L 15.5113 3.15753 L 19.7019 23.1944 Z M 38.5452 16.756 C 38.5452 21.6958 41.1539 23.472 44.8727 23.472 C 48.5914 23.472 51.2001 21.6958 51.2001 16.756 L 51.2001 9.59598 C 51.2001 4.65613 48.5914 2.88 44.8727 2.88 C 41.1539 2.88 38.5452 4.65613 38.5452 9.59598 L 38.5452 16.756 Z M 42.375 9.09645 C 42.375 6.87629 43.3463 6.26575 44.8727 6.26575 C 46.399 6.26575 47.3704 6.87629 47.3704 9.09645 L 47.3704 17.2555 C 47.3704 19.4757 46.399 20.0862 44.8727 20.0862 C 43.3463 20.0862 42.375 19.4757 42.375 17.2555 L 42.375 9.09645 Z M 53.4203 16.756 C 53.4203 21.6958 56.029 23.472 59.7477 23.472 C 63.4665 23.472 66.0752 21.6958 66.0752 16.756 L 66.0752 9.59598 C 66.0752 4.65613 63.4665 2.88 59.7477 2.88 C 56.029 2.88 53.4203 4.65613 53.4203 9.59598 L 53.4203 16.756 Z M 57.25 9.09645 C 57.25 6.87629 58.2214 6.26575 59.7477 6.26575 C 61.2741 6.26575 62.2454 6.87629 62.2454 9.09645 L 62.2454 17.2555 C 62.2454 19.4757 61.2741 20.0862 59.7477 20.0862 C 58.2214 20.0862 57.25 19.4757 57.25 17.2555 L 57.25 9.09645 Z M 68.4896 23.1944 L 73.818 23.1944 C 78.2028 23.1944 80.6449 21.3073 80.8114 16.2565 L 80.8114 10.0955 C 80.6449 5.04466 78.2028 3.15753 73.818 3.15753 L 68.4896 3.15753 L 68.4896 23.1944 Z M 72.3194 6.54326 L 73.6514 6.54326 C 76.0381 6.54326 76.9817 7.70885 76.9817 10.5395 L 76.9817 15.8124 C 76.9817 18.8096 75.7606 19.8087 73.6514 19.8087 L 72.3194 19.8087 L 72.3194 6.54326 Z M 86.9169 23.1944 L 86.9169 14.5358 L 91.7457 14.5358 L 91.7457 11.1501 L 86.9169 11.1501 L 86.9169 6.54326 L 93.0778 6.54326 L 93.0778 3.15753 L 83.0871 3.15753 L 83.0871 23.1944 L 86.9169 23.1944 Z M 99.4603 23.1944 L 99.4603 3.15753 L 95.6306 3.15753 L 95.6306 23.1944 L 99.4603 23.1944 Z M 105.594 23.1944 L 105.594 10.262 L 105.649 10.262 L 111.505 23.1944 L 115.168 23.1944 L 115.168 3.15753 L 111.671 3.15753 L 111.671 15.0354 L 111.616 15.0354 L 106.287 3.15753 L 102.097 3.15753 L 102.097 23.1944 L 105.594 23.1944 Z M 128.489 23.1944 L 128.489 19.8087 L 121.551 19.8087 L 121.551 14.5358 L 126.629 14.5358 L 126.629 11.1501 L 121.551 11.1501 L 121.551 6.54326 L 128.211 6.54326 L 128.211 3.15753 L 117.721 3.15753 L 117.721 23.1944 L 128.489 23.1944 Z"></path></g></svg>"##;

const WORDMARK_SVG_POINTSAV: &str = r##"<svg class="logo-svg" role="img" aria-label="PointSav, Inc." viewBox="0 0 144 36" xmlns="http://www.w3.org/2000/svg" fill="#111827"><g transform="translate(-28.8654, -10.2516) scale(1.3911)"><defs id="defs2"></defs><path d="M 36.629983,15.88 V 12.4 h 0.7 q 1.44,0 1.44,1.74 0,0.9 -0.38,1.32 -0.36,0.42 -1.06,0.42 z m -3.72,-6.16 V 24 h 3.72 v -5.44 h 1.3 q 2.3,0 3.48,-1.1 1.2,-1.12 1.2,-3.34 0,-0.94 -0.24,-1.74 -0.24,-0.8 -0.76,-1.38 -0.5,-0.6 -1.28,-0.94 -0.78,-0.34 -1.84,-0.34 z m 14.300013,7.14 q 0,-1.44 0.04,-2.38 0.04,-0.96 0.16,-1.52 0.14,-0.56 0.38,-0.78 0.26,-0.22 0.68,-0.22 0.42,0 0.66,0.22 0.26,0.22 0.38,0.78 0.14,0.56 0.18,1.52 0.04,0.94 0.04,2.38 0,1.44 -0.04,2.4 -0.04,0.94 -0.18,1.5 -0.12,0.56 -0.38,0.78 -0.24,0.22 -0.66,0.22 -0.42,0 -0.68,-0.22 -0.24,-0.22 -0.38,-0.78 -0.12,-0.56 -0.16,-1.5 -0.04,-0.96 -0.04,-2.4 z m -3.84,0 q 0,2 0.26,3.42 0.26,1.4 0.86,2.3 0.6,0.88 1.58,1.26 0.98,0.38 2.4,0.38 1.42,0 2.4,-0.38 0.98,-0.38 1.58,-1.26 0.6,-0.9 0.86,-2.3 0.26,-1.42 0.26,-3.42 0,-2 -0.26,-3.4 -0.26,-1.42 -0.86,-2.3 -0.6,-0.9 -1.58,-1.3 -0.98,-0.42 -2.4,-0.42 -1.42,0 -2.4,0.42 -0.98,0.4 -1.58,1.3 -0.6,0.88 -0.86,2.3 -0.26,1.4 -0.26,3.4 z M 54.769991,9.72 V 24 h 3.72 V 9.72 Z m 5.340005,0 V 24 h 3.48 v -8.82 h 0.04 l 2.48,8.82 h 4.08 V 9.72 h -3.48 v 8.8 h -0.04 l -2.4,-8.8 z m 13.720019,3.16 V 24 h 3.72 V 12.88 h 2.8 V 9.72 h -9.28 v 3.16 z m 12.919998,0.96 h 3.48 q 0,-2.3 -1.1,-3.34 -1.08,-1.06 -3.52,-1.06 -2.36,0 -3.52,1.14 -1.16,1.14 -1.16,3.32 0,1.26 0.42,2.04 0.44,0.78 1.08,1.26 0.66,0.48 1.42,0.78 0.76,0.3 1.4,0.6 0.66,0.28 1.08,0.7 0.44,0.4 0.44,1.1 0,0.58 -0.32,0.98 -0.3,0.4 -0.88,0.4 -0.54,0 -0.88,-0.36 -0.34,-0.38 -0.34,-1.3 v -0.34 h -3.6 v 0.5 q 0,1.12 0.32,1.88 0.32,0.76 0.92,1.24 0.62,0.46 1.5,0.64 0.9,0.2 2.06,0.2 2.46,0 3.76,-1.02 1.3,-1.04 1.3,-3.32 0,-1.3 -0.46,-2.1 -0.44,-0.82 -1.12,-1.32 -0.68,-0.5 -1.46,-0.8 -0.78,-0.32 -1.46,-0.62 -0.68,-0.3 -1.14,-0.7 -0.44,-0.42 -0.44,-1.12 0,-0.48 0.28,-0.86 0.28,-0.4 0.88,-0.4 0.54,0 0.8,0.46 0.26,0.44 0.26,1.08 z m 9.840013,-1.2 -1.02,6.06 h 2.08 l -1.02,-6.06 z m 2.36,-2.92 3.480004,14.28 h -3.960004 l -0.38,-2.5 h -2.96 l -0.38,2.5 h -3.9 l 3.42,-14.28 z m 2.259984,0 3.02,14.28 h 4.8 l 3.08,-14.28 h -3.96 l -1.5,10.76 h -0.04 l -1.5,-10.76 z" id="text1" style="font-weight:900;font-size:20px;font-family:'Helvetica Neue', Helvetica, Arial, sans-serif;text-anchor:middle;fill:#111827;fill-rule:evenodd" aria-label="POINTSAV"></path><path d="m 36.767496,30.4389 v -2.25 h 0.56 q 0.29,0 0.485,0.085 0.2,0.08 0.32,0.235 0.12,0.155 0.17,0.375 0.055,0.215 0.055,0.485 0,0.295 -0.075,0.5 -0.075,0.205 -0.2,0.335 -0.125,0.125 -0.285,0.18 -0.16,0.055 -0.33,0.055 z m -0.785,-2.91 v 3.57 h 1.54 q 0.41,0 0.71,-0.135 0.305,-0.14 0.505,-0.38 0.205,-0.24 0.305,-0.57 0.1,-0.33 0.1,-0.72 0,-0.445 -0.125,-0.775 -0.12,-0.33 -0.34,-0.55 -0.215,-0.22 -0.515,-0.33 -0.295,-0.11 -0.64,-0.11 z m 5.704996,0 v 3.57 h 0.785 v -3.57 z m 6.055,3.165 0.08,0.405 h 0.5 v -1.93 h -1.5 v 0.585 h 0.79 q -0.035,0.375 -0.25,0.575 -0.21,0.195 -0.6,0.195 -0.265,0 -0.45,-0.1 -0.185,-0.105 -0.3,-0.275 -0.115,-0.17 -0.17,-0.38 -0.05,-0.215 -0.05,-0.44 0,-0.235 0.05,-0.455 0.055,-0.22 0.17,-0.39 0.115,-0.175 0.3,-0.275 0.185,-0.105 0.45,-0.105 0.285,0 0.485,0.15 0.2,0.15 0.27,0.45 h 0.75 q -0.03,-0.305 -0.165,-0.54 -0.135,-0.235 -0.345,-0.395 -0.205,-0.16 -0.465,-0.24 -0.255,-0.085 -0.53,-0.085 -0.41,0 -0.74,0.145 -0.325,0.145 -0.55,0.4 -0.225,0.255 -0.345,0.6 -0.12,0.34 -0.12,0.74 0,0.39 0.12,0.73 0.12,0.335 0.345,0.585 0.225,0.25 0.55,0.395 0.33,0.14 0.74,0.14 0.26,0 0.515,-0.105 0.255,-0.11 0.465,-0.38 z m 3.215004,-3.165 v 3.57 h 0.785 v -3.57 z m 4.265001,0.66 v 2.91 h 0.785 v -2.91 h 1.07 v -0.66 h -2.925 v 0.66 z m 4.705005,1.53 0.465,-1.31 h 0.01 l 0.45,1.31 z m 0.075,-2.19 -1.35,3.57 h 0.79 l 0.28,-0.795 h 1.335 l 0.27,0.795 h 0.815 l -1.335,-3.57 z m 4.449997,0 v 3.57 h 2.525 v -0.66 h -1.74 v -2.91 z m 8.890002,2.385 h -0.76 q -0.005,0.33 0.12,0.57 0.125,0.24 0.335,0.395 0.215,0.155 0.49,0.225 0.28,0.075 0.575,0.075 0.365,0 0.64,-0.085 0.28,-0.085 0.465,-0.235 0.19,-0.155 0.285,-0.365 0.095,-0.21 0.095,-0.455 0,-0.3 -0.13,-0.49 -0.125,-0.195 -0.3,-0.31 -0.175,-0.115 -0.355,-0.165 -0.175,-0.055 -0.275,-0.075 -0.335,-0.085 -0.545,-0.14 -0.205,-0.055 -0.325,-0.11 -0.115,-0.055 -0.155,-0.12 -0.04,-0.065 -0.04,-0.17 0,-0.115 0.05,-0.19 0.05,-0.075 0.125,-0.125 0.08,-0.05 0.175,-0.07 0.095,-0.02 0.19,-0.02 0.145,0 0.265,0.025 0.125,0.025 0.22,0.085 0.095,0.06 0.15,0.165 0.06,0.105 0.07,0.265 h 0.76 q 0,-0.31 -0.12,-0.525 -0.115,-0.22 -0.315,-0.36 -0.2,-0.14 -0.46,-0.2 -0.255,-0.065 -0.535,-0.065 -0.24,0 -0.48,0.065 -0.24,0.065 -0.43,0.2 -0.19,0.135 -0.31,0.34 -0.115,0.2 -0.115,0.475 0,0.245 0.09,0.42 0.095,0.17 0.245,0.285 0.15,0.115 0.34,0.19 0.19,0.07 0.39,0.12 0.195,0.055 0.385,0.1 0.19,0.045 0.34,0.105 0.15,0.06 0.24,0.15 0.095,0.09 0.095,0.235 0,0.135 -0.07,0.225 -0.07,0.085 -0.175,0.135 -0.105,0.05 -0.225,0.07 -0.12,0.015 -0.225,0.015 -0.155,0 -0.3,-0.035 -0.145,-0.04 -0.255,-0.115 -0.105,-0.08 -0.17,-0.205 -0.065,-0.125 -0.065,-0.305 z m 5.635002,-0.205 v 1.39 h 0.785 v -1.37 l 1.325,-2.2 h -0.875 l -0.83,1.41 -0.835,-1.41 h -0.88 z m 4.944999,0.205 h -0.76 q -0.005,0.33 0.12,0.57 0.125,0.24 0.335,0.395 0.215,0.155 0.49,0.225 0.28,0.075 0.575,0.075 0.365,0 0.64,-0.085 0.28,-0.085 0.465,-0.235 0.19,-0.155 0.285,-0.365 0.095,-0.21 0.095,-0.455 0,-0.3 -0.13,-0.49 -0.125,-0.195 -0.3,-0.31 -0.175,-0.115 -0.355,-0.165 -0.175,-0.055 -0.275,-0.075 -0.335,-0.085 -0.545,-0.14 -0.205,-0.055 -0.325,-0.11 -0.115,-0.055 -0.155,-0.12 -0.04,-0.065 -0.04,-0.17 0,-0.115 0.05,-0.19 0.05,-0.075 0.125,-0.125 0.08,-0.05 0.175,-0.07 0.095,-0.02 0.19,-0.02 0.145,0 0.265,0.025 0.125,0.025 0.22,0.085 0.095,0.06 0.15,0.165 0.06,0.105 0.07,0.265 h 0.76 q 0,-0.31 -0.12,-0.525 -0.115,-0.22 -0.315,-0.36 -0.2,-0.14 -0.46,-0.2 -0.255,-0.065 -0.535,-0.065 -0.24,0 -0.48,0.065 -0.24,0.065 -0.43,0.2 -0.19,0.135 -0.31,0.34 -0.115,0.2 -0.115,0.475 0,0.245 0.09,0.42 0.095,0.17 0.245,0.285 0.15,0.115 0.34,0.19 0.19,0.07 0.39,0.12 0.195,0.055 0.385,0.1 0.19,0.045 0.34,0.105 0.15,0.06 0.24,0.15 0.095,0.09 0.095,0.235 0,0.135 -0.07,0.225 -0.07,0.085 -0.175,0.135 -0.105,0.05 -0.225,0.07 -0.12,0.015 -0.225,0.015 -0.155,0 -0.3,-0.035 -0.145,-0.04 -0.255,-0.115 -0.105,-0.08 -0.17,-0.205 -0.065,-0.125 -0.065,-0.305 z m 5.499999,-1.725 v 2.91 h 0.785 v -2.91 h 1.07 v -0.66 h -2.925 v 0.66 z m 4.265001,-0.66 v 3.57 h 2.71 v -0.66 h -1.925 v -0.875 h 1.73 v -0.61 h -1.73 v -0.765 h 1.885 v -0.66 z m 5.240001,0 v 3.57 h 0.735 v -2.505 h 0.01 l 0.874997,2.505 h 0.605 l 0.875,-2.53 h 0.01 v 2.53 h 0.735 v -3.57 h -1.105 l -0.79,2.455 h -0.01 l -0.835,-2.455 z m 7.070007,2.385 h -0.76 q -0.005,0.33 0.12,0.57 0.125,0.24 0.335,0.395 0.215,0.155 0.49,0.225 0.28,0.075 0.575,0.075 0.365,0 0.64,-0.085 0.28,-0.085 0.465,-0.235 0.19,-0.155 0.285,-0.365 0.095,-0.21 0.095,-0.455 0,-0.3 -0.13,-0.49 -0.125,-0.195 -0.3,-0.31 -0.175,-0.115 -0.355,-0.165 -0.175,-0.055 -0.275,-0.075 -0.335,-0.085 -0.545,-0.14 -0.205,-0.055 -0.325,-0.11 -0.115,-0.055 -0.155,-0.12 -0.04,-0.065 -0.04,-0.17 0,-0.115 0.05,-0.19 0.05,-0.075 0.125,-0.125 0.08,-0.05 0.175,-0.07 0.095,-0.02 0.19,-0.02 0.145,0 0.265,0.025 0.125,0.025 0.22,0.085 0.095,0.06 0.15,0.165 0.06,0.105 0.07,0.265 h 0.76 q 0,-0.31 -0.12,-0.525 -0.115,-0.22 -0.315,-0.36 -0.2,-0.14 -0.46,-0.2 -0.255,-0.065 -0.535,-0.065 -0.24,0 -0.48,0.065 -0.24,0.065 -0.43,0.2 -0.19,0.135 -0.31,0.34 -0.115,0.2 -0.115,0.475 0,0.245 0.09,0.42 0.095,0.17 0.245,0.285 0.15,0.115 0.34,0.19 0.19,0.07 0.39,0.12 0.195,0.055 0.385,0.1 0.19,0.045 0.34,0.105 0.15,0.06 0.24,0.15 0.095,0.09 0.095,0.235 0,0.135 -0.07,0.225 -0.07,0.085 -0.175,0.135 -0.105,0.05 -0.225,0.07 -0.12,0.015 -0.225,0.015 -0.155,0 -0.3,-0.035 -0.145,-0.04 -0.255,-0.115 -0.105,-0.08 -0.17,-0.205 -0.065,-0.125 -0.065,-0.305 z" id="text2" style="font-weight:700;font-size:5px;font-family:'Helvetica Neue', Helvetica, Arial, sans-serif;letter-spacing:2;text-anchor:middle;fill:#111827;fill-rule:evenodd" aria-label="DIGITAL SYSTEMS"></path></g></svg>"##;

/// Per-instance tenant. Derived from SiteConfig.instance at startup.
/// Instances start similar; diverge by adding Tenant methods — never by forking this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tenant {
    Documentation,
    Projects,
    Corporate,
}

impl Tenant {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "projects" => Self::Projects,
            "corporate" => Self::Corporate,
            _ => Self::Documentation,
        }
    }

    pub fn brand(self) -> &'static str {
        match self {
            Self::Documentation => "pointsav",
            Self::Projects | Self::Corporate => "woodfine",
        }
    }

    pub fn is_woodfine(self) -> bool {
        matches!(self, Self::Projects | Self::Corporate)
    }

    pub fn instance_str(self) -> &'static str {
        match self {
            Self::Documentation => "documentation",
            Self::Projects => "projects",
            Self::Corporate => "corporate",
        }
    }

    /// Verbatim trademark text — TRADEMARK.md v1.1 (2026-05-16).
    /// Copyright holder is Woodfine Capital Projects Inc. for all instances.
    /// Trademark line kept separate from copyright line per TRADEMARK.md §7.
    pub fn trademark_line(self) -> &'static str {
        match self {
            Self::Documentation => {
                "PointSav Digital Systems\u{2122}, Totebox Orchestration\u{2122}, and \
                 Totebox Archive\u{2122} are trademarks of Woodfine Capital Projects Inc., \
                 used in Canada, the United States, Latin America, and Europe. \
                 All other trademarks are the property of their respective owners."
            }
            Self::Projects | Self::Corporate => {
                "Woodfine Capital Projects\u{2122}, Woodfine Management Corp\u{2122}, \
                 PointSav Digital Systems\u{2122}, Totebox Orchestration\u{2122}, and \
                 Totebox Archive\u{2122} are trademarks of Woodfine Capital Projects Inc., \
                 used in Canada, the United States, Latin America, and Europe. \
                 All other trademarks are the property of their respective owners."
            }
        }
    }

    fn wordmark_svg(self) -> &'static str {
        if self.is_woodfine() {
            WORDMARK_SVG_WOODFINE
        } else {
            WORDMARK_SVG_POINTSAV
        }
    }
}

/// Primary masthead: sticky 64px navy bar.
/// Layout: wordmark left | inline search centre | lang+theme right.
/// Replaces all four legacy `<header class="topnav">` blocks + their search panels.
/// `lang` is "en" or "es" (from `Locale::lang_attr()` in the server module).
pub fn sovereign_nav(tenant: Tenant, lang: &str, site_title: &str, lang_href: &str) -> Markup {
    let lang_label = if lang == "es" { "EN" } else { "ES" };
    let search_placeholder = if lang == "es" {
        "Buscar\u{2026}"
    } else {
        "Search\u{2026}"
    };
    html! {
        header class="topnav s-topnav" role="banner" {
            a class="s-wordmark" href="/" aria-label=(site_title) {
                (PreEscaped(tenant.wordmark_svg()))
            }
            form class="s-search topnav-search" action="/search" method="get" role="search" {
                div class="header-search-wrap" {
                    input type="search" name="q" id="header-search-q"
                          class="s-search__input"
                          placeholder=(search_placeholder)
                          autocomplete="off"
                          aria-label=(search_placeholder)
                          spellcheck="false";
                    div class="ac-dropdown" id="search-autocomplete-dropdown" {}
                }
                button type="submit" class="topnav-search-btn s-search__btn" aria-label="Search" {
                    (PreEscaped(r#"<svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true" width="16" height="16"><circle cx="8.5" cy="8.5" r="5"/><line x1="13" y1="13" x2="18" y2="18"/></svg>"#))
                }
            }
            div class="s-controls" {
                a class="lang-toggle s-lang" href=(lang_href) aria-label="Switch language" {
                    (lang_label)
                }
                button class="wiki-appearance-btn s-theme-btn" id="s-theme-btn"
                       aria-label="Toggle appearance" type="button" {
                    (PreEscaped(r#"<svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true"><circle cx="10" cy="10" r="4"/><path stroke="currentColor" stroke-width="1.5" fill="none" d="M10 1v2.5M10 16.5V19M1 10h2.5M16.5 10H19M3.64 3.64l1.77 1.77M14.59 14.59l1.77 1.77M3.64 16.36l1.77-1.77M14.59 5.41l1.77-1.77"/></svg>"#))
                }
                button class="s-hamburger" id="nav-toggle"
                       aria-label="Menu"
                       aria-expanded="false"
                       aria-controls="mobile-nav-drawer"
                       type="button" {
                    (PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="20" height="20" aria-hidden="true"><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="18" x2="21" y2="18"/></svg>"#))
                }
            }
        }
    }
}

/// Mobile nav drawer + overlay for pages that use sovereign_page().
/// Article pages provide their own `#mobile-nav-drawer` with per-article TOC — do not call this there.
pub fn sovereign_mobile_nav_drawer(tenant: Tenant, site_title: &str) -> Markup {
    html! {
        nav class="mobile-nav-drawer" id="mobile-nav-drawer" aria-hidden="true" {
            div class="mobile-nav-header" {
                a class="site-title" href="/" { (site_title) }
                button class="mobile-nav-close" id="mobile-nav-close" aria-label="Close navigation" {
                    (PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="20" height="20" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>"#))
                }
            }
            ul class="mobile-nav-list" {
                li { a href="/" { "Home" } }
                li { a href="/search" { "Search" } }
                li { a href="/random" { "Random article" } }
                li { a href="/special/all-pages" { "All pages" } }
                li { a href="/special/categories" { "Categories" } }
                li { a href="/special/recent-changes" { "Recent changes" } }
                @if tenant.is_woodfine() {
                    li { a href="/page/contact" { "Contact" } }
                }
                li { a href="/page/disclaimer" { "Disclaimer" } }
                li { a href="/page/privacy" { "Privacy" } }
            }
        }
        div class="mobile-nav-overlay" id="mobile-nav-overlay" aria-hidden="true" {}
    }
}

/// Site footer: near-black #0e1117 background, three-column layout, correct WCP Inc. legal text.
/// Copyright holder is Woodfine Capital Projects Inc. — NEVER the trade names.
/// No engine version string (never expose build internals in public HTML).
pub fn sovereign_footer(tenant: Tenant, view_source_slug: Option<&str>) -> Markup {
    html! {
        footer class="s-footer" role="contentinfo" {
            div class="s-footer__inner" {
                div class="s-footer__cols" {
                    div class="s-footer__col" {
                        span class="s-footer__col-head" { "Navigate" }
                        a href="/" { "Home" }
                        a href="/search" { "Search" }
                        @if tenant.is_woodfine() {
                            a href="/page/contact" { "Contact" }
                        }
                        @if let Some(slug) = view_source_slug {
                            a href={ "/git/" (slug) } { "View source" }
                        }
                    }
                    div class="s-footer__col" {
                        span class="s-footer__col-head" { "Legal" }
                        a href="/page/disclaimer" { "Disclaimer" }
                        a href="/page/privacy" { "Privacy" }
                    }
                    div class="s-footer__col" {
                        span class="s-footer__col-head" { "Platform" }
                        a href="/feed.atom" { "Atom feed" }
                        a href="/sitemap.xml" { "Sitemap" }
                    }
                }
                div class="s-footer__legal" {
                    p class="s-footer__copyright" {
                        "\u{00a9} 2026 Woodfine Capital Projects Inc. All rights reserved."
                    }
                    p class="s-footer__trademark" {
                        (tenant.trademark_line())
                    }
                }
            }
        }
    }
}

/// Full document wrapper for pages that do not need locale alternates or JSON-LD.
/// Used by misc_handlers chrome() and page_handler(), and chrome/mod.rs base_page().
/// Home and wiki handlers build their own document wrapper for hreflang/canonical/JSON-LD.
/// `lang` is "en" or "es" (from `Locale::lang_attr()` in the server module).
pub fn sovereign_page(
    title: &str,
    tenant: Tenant,
    lang: &str,
    site_title: &str,
    lang_href: &str,
    content: Markup,
) -> Markup {
    let brand = tenant.brand();
    let instance = tenant.instance_str();
    html! {
        (DOCTYPE)
        html lang=(lang) data-theme="light" data-brand=(brand) data-instance=(instance) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
                title { (title) }
                link rel="preload" as="font" type="font/woff2" crossorigin
                     href="/static/fonts/IBMPlexSans-Variable-latin.woff2";
                link rel="preload" as="font" type="font/woff2" crossorigin
                     href="/static/fonts/PlayfairDisplay-Variable-latin.woff2";
                link rel="stylesheet" href="/static/tokens.css";
                link rel="stylesheet" href="/static/style.css";
                @if brand == "woodfine" {
                    link rel="stylesheet" href="/static/tokens-woodfine.css";
                }
                script { (PreEscaped(r#"(function(){var t=localStorage.getItem('wiki-theme')||'light';document.documentElement.setAttribute('data-theme',t);var w=localStorage.getItem('wiki-width')||'standard';document.documentElement.setAttribute('data-width',w);}());"#)) }
                script { (PreEscaped(r#"document.addEventListener('DOMContentLoaded',function(){try{navigator.sendBeacon('/_beacon',JSON.stringify({u:location.pathname,t:Date.now()}));}catch(e){}});"#)) }
            }
            body {
                a class="skip-to-content" href="#main-content" { "Skip to content" }
                (sovereign_nav(tenant, lang, site_title, lang_href))
                (sovereign_mobile_nav_drawer(tenant, site_title))
                main class="site-main" id="main-content" {
                    (content)
                }
                (sovereign_footer(tenant, None))
                script src="/static/wiki.js" defer="true" {}
            }
        }
    }
}
