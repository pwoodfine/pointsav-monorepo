pub mod bundle;
pub mod component;
pub mod research;
pub mod token;

use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub enum SchemaType {
    Component,
    Token,
    Research,
    Bundle,
    Unknown,
}

/// Detect the schema type from the `type:` or `artifact:` frontmatter field.
pub fn detect(frontmatter: &HashMap<String, String>) -> SchemaType {
    let type_field = frontmatter
        .get("type")
        .or_else(|| frontmatter.get("artifact"))
        .map(|s| s.to_lowercase());

    match type_field.as_deref() {
        Some(s) if s.contains("component") => SchemaType::Component,
        Some(s) if s.contains("token") => SchemaType::Token,
        Some(s) if s.contains("research") => SchemaType::Research,
        Some(s) if s.contains("bundle") => SchemaType::Bundle,
        _ => SchemaType::Unknown,
    }
}

/// Dispatch rendering to the appropriate schema renderer.
pub fn render(schema: SchemaType, frontmatter: &HashMap<String, String>, body: &str) -> String {
    let fm = if frontmatter.is_empty() {
        None
    } else {
        Some(frontmatter)
    };
    match schema {
        SchemaType::Component => component::render(fm, body),
        SchemaType::Token => token::render(fm, body),
        SchemaType::Research => research::render(fm, body),
        SchemaType::Bundle => bundle::render(fm, body),
        SchemaType::Unknown => crate::render::render_markdown(body),
    }
}
