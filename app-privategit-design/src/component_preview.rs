// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// P1.1 — live component preview rendered from recipe.json's variant HTML + CSS.
// Sandboxed via iframe srcdoc so the component's own CSS never leaks into (or is
// polluted by) the portal's own chrome styles.
use serde_json::Value;
use std::path::Path;

pub fn render_preview(vault: &Path, slug: &str) -> Option<String> {
    let path = vault.join("components").join(slug).join("recipe.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let recipe: Value = serde_json::from_str(&raw).ok()?;
    let variants = recipe.get("variants")?.as_array()?;
    let css = recipe.get("css").and_then(|v| v.as_str()).unwrap_or("");

    let mut out = String::from(
        "<div class=\"component-preview\">\n<h2>Live preview</h2>\n<div class=\"cp-variants\">\n",
    );
    for variant in variants {
        let name = variant
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("variant");
        let html = variant.get("html").and_then(|v| v.as_str()).unwrap_or("");
        let filled = html
            .replace("{{label}}", "Label")
            .replace("{{ label }}", "Label");
        let doc = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><style>body{{margin:0;padding:24px;display:flex;align-items:center;justify-content:center;min-height:100%;font-family:system-ui,sans-serif;box-sizing:border-box}}{css}</style></head><body>{filled}</body></html>"
        );
        out.push_str(&format!(
            "<div class=\"cp-variant\"><p class=\"cp-variant-name\">{}</p><iframe class=\"cp-frame\" title=\"{} variant preview\" srcdoc=\"{}\" loading=\"lazy\" sandbox=\"allow-same-origin\"></iframe></div>\n",
            html_escape(name),
            html_escape(name),
            attr_escape(&doc),
        ));
    }
    out.push_str("</div>\n</div>\n");
    Some(out)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn attr_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;")
}
