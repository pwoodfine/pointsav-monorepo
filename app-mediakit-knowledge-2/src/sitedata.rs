//! Per-wiki data loaded from the content repo root at startup:
//! `categories.yaml` (the canonical category nav — id, display name, order) and
//! `redirects.yaml` (Hugo-style `from → to` 301s). Both are optional; missing or
//! malformed files degrade to empty, so the engine falls back to its config.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

/// A category from `categories.yaml` — `id` is the dir/route, `name` is display.
#[derive(Debug, Clone)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub order: i64,
}

#[derive(Deserialize)]
struct CategoriesFile {
    #[serde(default)]
    categories: Vec<CatEntry>,
}

#[derive(Deserialize)]
struct CatEntry {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    order: i64,
}

/// Load `categories.yaml` from the mount root, sorted by `order`. Empty if absent.
pub fn load_categories(root: &Path) -> Vec<Category> {
    let Ok(text) = std::fs::read_to_string(root.join("categories.yaml")) else {
        return Vec::new();
    };
    let Ok(file) = serde_yaml::from_str::<CategoriesFile>(&text) else {
        return Vec::new();
    };
    let mut cats: Vec<Category> = file
        .categories
        .into_iter()
        .map(|c| Category {
            id: c.id,
            name: c.name,
            order: c.order,
        })
        .collect();
    cats.sort_by_key(|c| c.order);
    cats
}

#[derive(Deserialize)]
struct RedirectsFile {
    #[serde(default)]
    redirects: Vec<Redirect>,
}

#[derive(Deserialize)]
struct Redirect {
    from: String,
    to: String,
}

/// Load `redirects.yaml` from the mount root → `from → to` map. Empty if absent.
pub fn load_redirects(root: &Path) -> HashMap<String, String> {
    let Ok(text) = std::fs::read_to_string(root.join("redirects.yaml")) else {
        return HashMap::new();
    };
    match serde_yaml::from_str::<RedirectsFile>(&text) {
        Ok(file) => file.redirects.into_iter().map(|r| (r.from, r.to)).collect(),
        Err(_) => HashMap::new(),
    }
}
