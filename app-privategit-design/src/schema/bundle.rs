// DESIGN-BUNDLE renderer — RESERVED pending Command ratification.
// See outbox: project-design-20260614-design-bundle-ratification-request
use std::collections::HashMap;

pub fn render(_frontmatter: Option<&HashMap<String, String>>, _body: &str) -> String {
    "<div class=\"schema-badge schema-badge--bundle\">DESIGN-BUNDLE</div>\
     <p class=\"schema-reserved\">Bundle renderer pending ratification — see NEXT.md.</p>"
        .to_string()
}
