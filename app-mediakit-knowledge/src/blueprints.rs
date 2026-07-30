//! Phase 0 federation — content-type blueprints (Kirby-style schema registry).
//!
//! Promotes the free-text `type:` / `content_type:` frontmatter field into a real,
//! extensible schema. `topic` and `guide` ship built-in; a customer adds their own
//! (e.g. `regional-market` with structured infobox fields, `adr`, `changelog`) by
//! dropping `blueprints/<type>.yaml` files. A blueprint declares the frontmatter a
//! page of that type must carry, the nav section it lands in, its template, and the
//! typed relationships that drive cross-link rails (e.g. a TOPIC's "How-to guides"
//! rail ↔ a GUIDE's "Background / concepts" rail).
//!
//! This is the *infrastructure* layer (types + registry + validation, unit-tested).
//! Wiring blueprint-driven section routing and the cross-link rails into rendering
//! is a follow-on UI step. See `BRIEF-knowledge-platform-master.md` §11.2.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

/// A typed relationship from one content type to another, used to render
/// reciprocal cross-link rails (e.g. guide → topic, labelled "Background").
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Relation {
    /// The related content type (e.g. `topic`).
    pub r#type: String,
    /// Human label for the rail (e.g. "Background / concepts").
    #[serde(rename = "as")]
    pub as_label: String,
    /// How the relation is discovered. Default `backlink` (pages of `type` that
    /// link here). Other strategies are future work.
    #[serde(default = "default_via_backlink")]
    pub via: String,
}

fn default_via_backlink() -> String {
    "backlink".to_string()
}

/// A content-type definition.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Blueprint {
    /// The type name this blueprint defines (matches frontmatter `type:`).
    #[serde(rename = "type")]
    pub type_name: String,
    /// Frontmatter keys a page of this type must carry.
    #[serde(default)]
    pub required: Vec<String>,
    /// Nav section pages of this type land in.
    #[serde(default)]
    pub section: Option<String>,
    /// Template hint for rendering (engine maps this to a chrome variant).
    #[serde(default)]
    pub template: Option<String>,
    /// Typed relationships driving cross-link rails.
    #[serde(default)]
    pub relates_to: Vec<Relation>,
}

/// The built-in `topic` blueprint.
fn builtin_topic() -> Blueprint {
    Blueprint {
        type_name: "topic".to_string(),
        required: vec!["title".to_string(), "slug".to_string()],
        section: None,
        template: Some("article".to_string()),
        relates_to: vec![Relation {
            r#type: "guide".to_string(),
            as_label: "How-to guides".to_string(),
            via: "backlink".to_string(),
        }],
    }
}

/// The built-in `guide` blueprint.
fn builtin_guide() -> Blueprint {
    Blueprint {
        type_name: "guide".to_string(),
        required: vec!["title".to_string(), "slug".to_string()],
        section: Some("Operational guides".to_string()),
        template: Some("guide".to_string()),
        relates_to: vec![Relation {
            r#type: "topic".to_string(),
            as_label: "Background / concepts".to_string(),
            via: "link".to_string(),
        }],
    }
}

/// A blueprint registry: the built-ins, plus any customer blueprints loaded from
/// `blueprints/*.yaml` (which override a built-in of the same `type`).
#[derive(Debug, Clone)]
pub struct Registry {
    blueprints: Vec<Blueprint>,
}

impl Registry {
    /// Registry with only the built-in `topic` + `guide` blueprints.
    pub fn builtin() -> Self {
        Registry {
            blueprints: vec![builtin_topic(), builtin_guide()],
        }
    }

    /// Built-ins plus `<dir>/*.yaml` customer blueprints. A customer blueprint with
    /// the same `type` as a built-in replaces it; new types are appended. A missing
    /// dir or an unparseable file is skipped (the registry never fails to build).
    pub fn load(dir: &Path) -> Self {
        let mut reg = Registry::builtin();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return reg;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_yaml = path
                .extension()
                .map(|e| e == "yaml" || e == "yml")
                .unwrap_or(false);
            if !is_yaml {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(bp) = serde_yaml::from_str::<Blueprint>(&text) {
                    reg.insert(bp);
                }
            }
        }
        reg
    }

    /// Add or replace a blueprint by `type`.
    fn insert(&mut self, bp: Blueprint) {
        if let Some(existing) = self
            .blueprints
            .iter_mut()
            .find(|b| b.type_name == bp.type_name)
        {
            *existing = bp;
        } else {
            self.blueprints.push(bp);
        }
    }

    /// Look up a blueprint by type name.
    pub fn find(&self, type_name: &str) -> Option<&Blueprint> {
        self.blueprints.iter().find(|b| b.type_name == type_name)
    }

    /// Known type names (for diagnostics / build-gate listing).
    pub fn type_names(&self) -> Vec<&str> {
        self.blueprints
            .iter()
            .map(|b| b.type_name.as_str())
            .collect()
    }
}

/// Validate a page's frontmatter keys against a blueprint. Returns the list of
/// required keys that are missing (empty = valid). `present_keys` is the set of
/// frontmatter keys the page actually declares.
pub fn validate(present_keys: &BTreeSet<String>, bp: &Blueprint) -> Vec<String> {
    bp.required
        .iter()
        .filter(|req| !present_keys.contains(*req))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn builtin_registry_has_topic_and_guide() {
        let reg = Registry::builtin();
        assert!(reg.find("topic").is_some());
        assert!(reg.find("guide").is_some());
        assert!(reg.find("nonsuch").is_none());
        // guide lands in the Operational guides section and relates to topic.
        let guide = reg.find("guide").unwrap();
        assert_eq!(guide.section.as_deref(), Some("Operational guides"));
        assert_eq!(guide.relates_to[0].r#type, "topic");
    }

    #[test]
    fn validate_detects_missing_required() {
        let reg = Registry::builtin();
        let topic = reg.find("topic").unwrap();
        // has both required keys → valid
        assert!(validate(&keys(&["title", "slug", "category"]), topic).is_empty());
        // missing slug → reported
        let missing = validate(&keys(&["title"]), topic);
        assert_eq!(missing, vec!["slug".to_string()]);
    }

    #[test]
    fn load_merges_customer_yaml_over_builtin() {
        let dir = std::env::temp_dir().join(format!("bp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A new custom type + an override of the built-in topic's required set.
        std::fs::write(
            dir.join("regional-market.yaml"),
            "type: regional-market\nrequired: [title, slug, rank, score]\nsection: Regional markets\ntemplate: infobox\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("topic.yaml"),
            "type: topic\nrequired: [title, slug, category]\n",
        )
        .unwrap();
        let reg = Registry::load(&dir);
        // new type present
        let rm = reg.find("regional-market").unwrap();
        assert_eq!(rm.template.as_deref(), Some("infobox"));
        assert!(rm.required.contains(&"rank".to_string()));
        // built-in topic overridden (now requires category)
        let topic = reg.find("topic").unwrap();
        assert!(topic.required.contains(&"category".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_missing_dir_returns_builtins() {
        let reg = Registry::load(Path::new("/no/such/dir/xyz"));
        assert_eq!(reg.type_names().len(), 2);
    }
}
