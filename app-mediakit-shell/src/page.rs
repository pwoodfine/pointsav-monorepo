//! The page manifest — an ordered list of typed sections plus metadata.
//!
//! A page lives as a Git-tracked YAML file (`content/<slug>/page.yaml`). The
//! file *is* the manifest: metadata at the top, a `sections:` list of typed
//! sections. This is the structured contract an AI author emits and a human
//! approves (diff + F12) before it is committed.

use serde::{Deserialize, Serialize};

use crate::section::Section;

fn default_lang() -> String {
    "en".to_string()
}

/// A complete page composed from the typed section vocabulary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    /// Page title (browser tab, `<title>`, used in chrome).
    pub title: String,
    /// URL slug. Optional in the manifest; the loader derives it from the
    /// directory name when absent.
    #[serde(default)]
    pub slug: Option<String>,
    /// Meta description (SEO + social).
    #[serde(default)]
    pub description: Option<String>,
    /// ISO 639-1 language code. Default `en`.
    #[serde(default = "default_lang")]
    pub lang: String,
    /// Ordered list of typed sections.
    pub sections: Vec<Section>,
}

impl Page {
    /// Parse and structurally validate a manifest. A returned `Err` is the
    /// rejection an AI author (via MCP) or a human (via the editor) sees when
    /// the manifest does not conform to the section contract.
    pub fn from_yaml(text: &str) -> Result<Page, String> {
        serde_yaml::from_str::<Page>(text).map_err(|e| e.to_string())
    }

    /// Serialize back to canonical YAML (used when staging an AI-proposed
    /// manifest for the human approval diff).
    pub fn to_yaml(&self) -> Result<String, String> {
        serde_yaml::to_string(self).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
title: Home
slug: home
description: A test page.
sections:
  - type: hero
    headline: Hello
    subhead: World
    cta:
      label: Go
      href: /go
  - type: prose
    body: |
      ## Section
      Body text.
"#;

    #[test]
    fn parses_valid_manifest() {
        let page = Page::from_yaml(SAMPLE).expect("valid manifest");
        assert_eq!(page.title, "Home");
        assert_eq!(page.lang, "en"); // default applied
        assert_eq!(page.sections.len(), 2);
        assert_eq!(page.sections[0].kind(), "hero");
        assert_eq!(page.sections[1].kind(), "prose");
    }

    #[test]
    fn rejects_unknown_section_type() {
        let bad = "title: X\nsections:\n  - type: not-a-real-type\n    foo: bar\n";
        assert!(Page::from_yaml(bad).is_err());
    }

    #[test]
    fn rejects_missing_required_field() {
        // hero requires `headline`
        let bad = "title: X\nsections:\n  - type: hero\n    subhead: only\n";
        assert!(Page::from_yaml(bad).is_err());
    }

    #[test]
    fn round_trips_through_yaml() {
        let page = Page::from_yaml(SAMPLE).unwrap();
        let yaml = page.to_yaml().unwrap();
        let again = Page::from_yaml(&yaml).unwrap();
        assert_eq!(again.sections.len(), page.sections.len());
    }
}
