// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_sorted_by_order_and_absent_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        // Deliberately out of order; loader must sort by `order`.
        std::fs::write(
            dir.path().join("categories.yaml"),
            "wiki: docs\ncategories:\n  - id: services\n    name: \"Platform Services\"\n    order: 5\n  - id: architecture\n    name: \"How It's Built\"\n    order: 1\n",
        )
        .unwrap();
        let cats = load_categories(dir.path());
        assert_eq!(cats.len(), 2);
        assert_eq!(cats[0].id, "architecture"); // order 1 first
        assert_eq!(cats[0].name, "How It's Built");
        assert_eq!(cats[1].id, "services");

        // Absent file → empty (graceful fallback).
        let empty = tempfile::tempdir().unwrap();
        assert!(load_categories(empty.path()).is_empty());
    }

    #[test]
    fn redirects_map_from_to() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("redirects.yaml"),
            "redirects:\n  - from: /old-path\n    to: https://example.com/new\n",
        )
        .unwrap();
        let map = load_redirects(dir.path());
        assert_eq!(map.get("/old-path").map(String::as_str), Some("https://example.com/new"));
        assert!(load_redirects(tempfile::tempdir().unwrap().path()).is_empty());
    }
}
