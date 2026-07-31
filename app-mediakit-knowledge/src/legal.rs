// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Canonical legal copy (trademark, copyright) loaded at startup from
//! `factory-release-engineering/tokens/legal-tokens-{brand}.yaml` — the
//! counsel-governed source of truth, per `Site.brand` ("pointsav" | "woodfine").
//!
//! Falls back to `LegalTokens::default()` (today's known-correct hardcoded
//! values) whenever the file is absent or malformed, so a missing/broken
//! token file degrades to the previous behavior rather than breaking the
//! footer. See `BRIEF-knowledge-ng-rewrite.md` P9 for the tracked caveat: the
//! canonical files are mid-revision (a trademark-naming fix is drafted but not
//! yet committed), so consuming them live may temporarily surface stale text
//! until that lands — accepted tradeoff, not a bug here.

use std::path::Path;

use serde::Deserialize;

/// Default location of the canonical token files on the workspace VM.
pub const DEFAULT_LEGAL_TOKENS_DIR: &str =
    "/srv/foundry/vendor/factory-release-engineering/tokens";

/// Parsed `legal-tokens-{brand}.yaml` (schema `foundry-legal-tokens-v1`).
#[derive(Debug, Clone, Deserialize)]
pub struct LegalTokens {
    pub copyright: Copyright,
    pub trademarks: Trademarks,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Copyright {
    pub holder: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Trademarks {
    /// Full trademark notice, English — already-composed prose, no substitution needed.
    pub statement: String,
}

impl Default for LegalTokens {
    /// Today's known-correct hardcoded values (verified 2026-07-11: matches the
    /// ratified MCorp™ rename, commit 062b29e in factory-release-engineering).
    /// Used whenever the token file can't be loaded, and as the literal source
    /// this module replaces in `ui::layout::footer()`.
    fn default() -> Self {
        LegalTokens {
            copyright: Copyright {
                holder: "Woodfine Capital Projects Inc.".to_string(),
            },
            trademarks: Trademarks {
                statement: "Woodfine Capital Projects\u{2122}, MCorp\u{2122}, PointSav Digital Systems\u{2122}, Totebox Orchestration\u{2122}, Totebox Archive\u{2122}, and Capability Geometry\u{2122} are trademarks of Woodfine Capital Projects Inc., used in Canada, the United States, Latin America, and Europe. All other trademarks are the property of their respective owners.".to_string(),
            },
        }
    }
}

/// Load `legal-tokens-{brand}.yaml` from `dir`. Malformed or absent → `None`
/// (caller falls back to `LegalTokens::default()`).
pub fn load(dir: &Path, brand: &str) -> Option<LegalTokens> {
    let path = dir.join(format!("legal-tokens-{brand}.yaml"));
    let text = std::fs::read_to_string(path).ok()?;
    serde_yaml::from_str(&text).ok()
}

/// Load using the default canonical directory.
pub fn load_default(brand: &str) -> Option<LegalTokens> {
    load(Path::new(DEFAULT_LEGAL_TOKENS_DIR), brand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_known_correct_trademark_text() {
        let d = LegalTokens::default();
        assert!(d.trademarks.statement.contains("MCorp\u{2122}"));
        assert!(d.trademarks.statement.contains("Capability Geometry\u{2122}"));
        assert_eq!(d.copyright.holder, "Woodfine Capital Projects Inc.");
    }

    #[test]
    fn loads_real_shape() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("legal-tokens-pointsav.yaml"),
            "schema: foundry-legal-tokens-v1\nbrand: pointsav\ncopyright:\n  holder: \"Woodfine Capital Projects Inc.\"\ntrademarks:\n  statement: \"Test statement.\"\n",
        )
        .unwrap();
        let loaded = load(dir.path(), "pointsav").unwrap();
        assert_eq!(loaded.copyright.holder, "Woodfine Capital Projects Inc.");
        assert_eq!(loaded.trademarks.statement, "Test statement.");
    }

    #[test]
    fn missing_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path(), "pointsav").is_none());
    }

    #[test]
    fn malformed_yaml_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("legal-tokens-woodfine.yaml"), ":::not valid:::").unwrap();
        assert!(load(dir.path(), "woodfine").is_none());
    }
}
