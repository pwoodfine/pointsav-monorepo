//! Phase 3.1 — claim extraction from claim-annotated TOPIC markdown.
//!
//! Implements `~/Foundry/conventions/claim-authoring-convention.md`
//! (doctrine claim #54). A *claim* is a span of body prose delimited by
//! paired HTML-comment markers:
//!
//! ```text
//! <!--claim id=… cites=[…] confidence=…-->
//! …claim prose…
//! <!--/claim-->
//! ```
//!
//! The markers render inert on the live engine — proven by
//! `render::tests::claim_markers_pass_through_inert` (the convention's
//! Engine Verification Gate). This module recovers the structured `Claim`
//! objects from that markup.
//!
//! Extraction is **lenient**: a malformed or incomplete marker yields a
//! warning (for the editorial linter — convention §9) rather than an
//! error, and never panics. The corpus is small; the scan is plain
//! string search.

use serde::Serialize;
use std::collections::BTreeSet;

const OPEN_PREFIX: &str = "<!--claim";
const CLOSE_MARKER: &str = "<!--/claim-->";
const MARKER_END: &str = "-->";

/// The epistemic grade of a claim — convention §4.4 (closed enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Established,
    Reported,
    Projected,
    Contested,
    Structural,
}

impl Confidence {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "established" => Some(Self::Established),
            "reported" => Some(Self::Reported),
            "projected" => Some(Self::Projected),
            "contested" => Some(Self::Contested),
            "structural" => Some(Self::Structural),
            _ => None,
        }
    }

    /// The canonical lowercase token for this grade.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Established => "established",
            Self::Reported => "reported",
            Self::Projected => "projected",
            Self::Contested => "contested",
            Self::Structural => "structural",
        }
    }
}

/// A claim extracted from TOPIC markdown — convention §4 (authored fields)
/// + §5 (engine-derived fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Claim {
    /// Global address — `<topic-slug>:<local-id>` (convention §7).
    pub id: String,
    /// Local id as authored — kebab-case, unique within the file.
    pub local_id: String,
    /// Citation-registry IDs (convention §4.3 `cites`). Resolved against
    /// `citations.yaml` in Phase 3.2.
    pub cites: Vec<String>,
    /// Epistemic grade (convention §4.4).
    pub confidence: Confidence,
    /// When the asserted fact began applying — authored, optional
    /// (convention §4.3 `valid_at`). `None` ⇒ timeless.
    pub valid_at: Option<String>,
    /// Claims this one depends on — bare `<id>` (same file) or global
    /// `<slug>:<id>` (cross-file). Resolved against the claim graph in
    /// Phase 3.3.
    pub depends_on: Vec<String>,
    /// The claim prose between the markers, whitespace-normalised.
    pub text: String,
    /// blake3 hex of the normalised claim text — derived (convention §5).
    /// A changed hash means the claim text changed.
    pub content_hash: String,
    /// Containing TOPIC slug — derived (convention §5).
    pub topic_slug: String,
    /// First 1-based line of this claim's content span within `body_md`.
    /// Used by `history::blame_published_at` to compute `published_at`.
    /// Includes the opening marker's closing `-->` newline so that edits
    /// to the marker fields (cites, confidence) also update the timestamp.
    pub line_start: u32,
    /// Last 1-based line of this claim's content span within `body_md`
    /// (inclusive). Covers through the final character before `<!--/claim-->`.
    pub line_end: u32,
    /// Committer timestamp (RFC 3339 UTC) of the newest commit whose blame
    /// touches `line_start..=line_end` — engine-derived (convention §5).
    /// `None` until `history::blame_published_at` is called, or when the
    /// content directory has no git history for this file.
    pub published_at: Option<String>,
}

/// The result of extracting claims from one TOPIC body.
#[derive(Debug, Default)]
pub struct Extraction {
    /// Every well-formed claim found, in document order.
    pub claims: Vec<Claim>,
    /// Human-readable diagnostics for the editorial linter (convention §9).
    pub warnings: Vec<String>,
}

/// Extract every claim from a TOPIC body. `topic_slug` namespaces the
/// global claim ids (`<topic-slug>:<local-id>`).
///
/// Never panics. Malformed markup becomes a `warning`; well-formed claims
/// are still recovered around it.
pub fn extract_claims(body_md: &str, topic_slug: &str) -> Extraction {
    let mut out = Extraction::default();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut cursor = 0usize;

    while let Some(rel) = body_md[cursor..].find(OPEN_PREFIX) {
        let open_start = cursor + rel;
        let after_prefix = open_start + OPEN_PREFIX.len();

        // Locate the end of the opening marker.
        let Some(end_rel) = body_md[after_prefix..].find(MARKER_END) else {
            out.warnings.push(format!(
                "unterminated claim marker at byte {open_start} (no `-->`)"
            ));
            break;
        };
        let marker_inner = &body_md[after_prefix..after_prefix + end_rel];
        let content_start = after_prefix + end_rel + MARKER_END.len();

        // The opening marker must be followed by a closing marker, with no
        // second opening marker before it — claims must not nest or
        // overlap (convention §4.1).
        let Some(close_rel) = body_md[content_start..].find(CLOSE_MARKER) else {
            out.warnings.push(format!(
                "claim opened at byte {open_start} is never closed \
                 (`<!--/claim-->` missing)"
            ));
            break;
        };
        if let Some(open_rel) = body_md[content_start..].find(OPEN_PREFIX) {
            if open_rel < close_rel {
                out.warnings.push(format!(
                    "claim opened at byte {open_start} overlaps or nests another \
                     claim — claims must not nest (convention §4.1); skipped"
                ));
                cursor = content_start;
                continue;
            }
        }
        let text_raw = &body_md[content_start..content_start + close_rel];
        let close_end = content_start + close_rel + CLOSE_MARKER.len();
        cursor = close_end;

        // §3.5: line range of the claim's content span within body_md.
        // line_start covers the opening marker's trailing newline so that
        // changes to marker fields (id, cites, confidence) are captured
        // by blame_published_at.
        let line_start = byte_to_line(body_md, content_start);
        let line_end = if close_rel == 0 {
            line_start
        } else {
            byte_to_line(body_md, content_start + close_rel - 1)
        };

        match parse_opening(marker_inner) {
            Ok(fields) => {
                if !seen.insert(fields.id.clone()) {
                    out.warnings.push(format!(
                        "duplicate claim id `{}` in `{topic_slug}` — ids must be \
                         unique within a file (convention §7); later one skipped",
                        fields.id
                    ));
                    continue;
                }
                if fields.cites.is_empty() && fields.confidence != Confidence::Structural {
                    out.warnings.push(format!(
                        "claim `{}` has empty `cites` but confidence is `{}` — only \
                         `structural` claims may omit citations (convention §4.3)",
                        fields.id,
                        fields.confidence.as_str()
                    ));
                }
                let text = normalise(text_raw);
                let content_hash = blake3::hash(text.as_bytes()).to_hex().to_string();
                out.claims.push(Claim {
                    id: format!("{topic_slug}:{}", fields.id),
                    local_id: fields.id,
                    cites: fields.cites,
                    confidence: fields.confidence,
                    valid_at: fields.valid_at,
                    depends_on: fields.depends_on,
                    text,
                    content_hash,
                    topic_slug: topic_slug.to_string(),
                    line_start,
                    line_end,
                    published_at: None,
                });
            }
            Err(why) => out.warnings.push(format!(
                "malformed claim marker at byte {open_start}: {why}"
            )),
        }
    }

    out
}

/// The authored fields parsed from one opening marker's interior.
struct OpeningFields {
    id: String,
    cites: Vec<String>,
    confidence: Confidence,
    valid_at: Option<String>,
    depends_on: Vec<String>,
}

/// Parse the text between `<!--claim` and `-->` into the authored fields.
/// Whitespace-separated `key=value` tokens; a marker may span lines.
/// Unknown keys are ignored (forward-compatible — convention §4.2).
fn parse_opening(inner: &str) -> Result<OpeningFields, String> {
    let mut id = None;
    let mut cites = None;
    let mut confidence = None;
    let mut valid_at = None;
    let mut depends_on = Vec::new();

    for tok in inner.split_whitespace() {
        let Some((key, value)) = tok.split_once('=') else {
            return Err(format!("field `{tok}` is not `key=value`"));
        };
        match key {
            "id" => id = Some(parse_bareword(value)?),
            "confidence" => {
                let v = parse_bareword(value)?;
                confidence =
                    Some(Confidence::parse(&v).ok_or_else(|| format!("unknown confidence `{v}`"))?);
            }
            "valid_at" => valid_at = Some(parse_bareword(value)?),
            "cites" => cites = Some(parse_list(value)?),
            "depends_on" => depends_on = parse_list(value)?,
            // Unknown key — forward-compatible: ignored here, reported by
            // the editorial linter (convention §4.2 / §9).
            _ => {}
        }
    }

    Ok(OpeningFields {
        id: id.ok_or("missing required field `id`")?,
        cites: cites.ok_or("missing required field `cites`")?,
        confidence: confidence.ok_or("missing required field `confidence`")?,
        valid_at,
        depends_on,
    })
}

/// A bareword value — `[A-Za-z0-9._:-]+`, no spaces, no quotes
/// (convention §4.2).
fn parse_bareword(v: &str) -> Result<String, String> {
    if v.is_empty() {
        return Err("empty value".into());
    }
    if v.starts_with('[') {
        return Err(format!("expected a bareword, found a list `{v}`"));
    }
    if !v
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
    {
        return Err(format!(
            "value `{v}` has characters outside [A-Za-z0-9._:-]"
        ));
    }
    Ok(v.to_string())
}

/// A list value — `[a,b,c]` or `[]` (convention §4.2).
fn parse_list(v: &str) -> Result<Vec<String>, String> {
    let inner = v
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| format!("list value `{v}` must be bracketed `[…]`"))?;
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    inner.split(',').map(parse_bareword).collect()
}

/// Strip `<!--claim …-->` open markers and `<!--/claim-->` close markers
/// from an HTML string, leaving only the inner prose. Called on the final
/// rendered HTML before it is served to readers. The markers are inert for
/// browsers but visible in page source and bloat payloads; removing them
/// keeps public HTML clean without touching the extraction pipeline.
///
/// The function is a plain string-scan: it removes every occurrence of
/// `<!--claim…-->` (including multi-line markers) and `<!--/claim-->`.
/// It does not parse claim structure — `extract_claims` has already done
/// that upstream.
pub fn strip_claim_markers(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find("<!--claim") {
        out.push_str(&rest[..pos]);
        // Find the end of this HTML comment (could be open or close marker).
        if let Some(end_rel) = rest[pos..].find("-->") {
            rest = &rest[pos + end_rel + 3..];
        } else {
            // Unclosed comment — emit the remainder as-is and stop.
            out.push_str(&rest[pos..]);
            return out;
        }
    }
    out.push_str(rest);
    // Strip closing claim markers (<!--/claim-->) which have a different
    // prefix than the opening markers handled by the loop above.
    out.replace("<!--/claim-->", "")
}

/// Whitespace-normalise a claim span so `content_hash` is stable across
/// markdown reflow (line wrapping) but changes on any word edit.
fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 1-based line number of the character at `byte` in `text`.
/// Counts `\n` characters in `text[..byte]`.
fn byte_to_line(text: &str, byte: usize) -> u32 {
    (text[..byte.min(text.len())].matches('\n').count() + 1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_structural_claim_with_empty_cites() {
        let body = "<!--claim id=derived-state confidence=structural cites=[]-->\n\
            The search index is derived state.\n\
            <!--/claim-->";
        let ex = extract_claims(body, "topic-x");
        assert_eq!(ex.warnings, Vec::<String>::new());
        assert_eq!(ex.claims.len(), 1);
        let c = &ex.claims[0];
        assert_eq!(c.id, "topic-x:derived-state");
        assert_eq!(c.local_id, "derived-state");
        assert_eq!(c.confidence, Confidence::Structural);
        assert!(c.cites.is_empty());
        assert_eq!(c.text, "The search index is derived state.");
        assert_eq!(c.topic_slug, "topic-x");
    }

    #[test]
    fn extracts_established_claim_with_cites_and_valid_at() {
        let body = "<!--claim id=tile-format cites=[c2sp-tlog-tiles] valid_at=2024 \
            confidence=established-->\n\
            The on-disk tile format follows the C2SP tlog-tiles specification.\n\
            <!--/claim-->";
        let ex = extract_claims(body, "worm-ledger-design");
        assert_eq!(ex.warnings, Vec::<String>::new());
        assert_eq!(ex.claims.len(), 1);
        let c = &ex.claims[0];
        assert_eq!(c.id, "worm-ledger-design:tile-format");
        assert_eq!(c.cites, vec!["c2sp-tlog-tiles"]);
        assert_eq!(c.valid_at.as_deref(), Some("2024"));
        assert_eq!(c.confidence, Confidence::Established);
    }

    #[test]
    fn extracts_inline_claim_with_multiline_marker_and_depends_on() {
        // Mirrors the convention §8 inline example — the opening marker
        // wraps across two lines.
        let body = "Because checkpoints are externally anchored, \
            <!--claim id=audit-without-operator\n\
            confidence=established cites=[rfc-9162] depends_on=[monthly-anchor]-->an \
            auditor can confirm integrity<!--/claim--> independently.";
        let ex = extract_claims(body, "topic-anchor");
        assert_eq!(ex.warnings, Vec::<String>::new());
        assert_eq!(ex.claims.len(), 1);
        let c = &ex.claims[0];
        assert_eq!(c.local_id, "audit-without-operator");
        assert_eq!(c.depends_on, vec!["monthly-anchor"]);
        assert_eq!(c.text, "an auditor can confirm integrity");
    }

    #[test]
    fn extracts_multiple_claims_in_document_order() {
        let body = "<!--claim id=a confidence=structural cites=[]-->First.<!--/claim-->\n\n\
            middle prose\n\n\
            <!--claim id=b confidence=structural cites=[]-->Second.<!--/claim-->";
        let ex = extract_claims(body, "t");
        assert_eq!(ex.claims.len(), 2);
        assert_eq!(ex.claims[0].local_id, "a");
        assert_eq!(ex.claims[1].local_id, "b");
    }

    #[test]
    fn content_hash_is_stable_across_whitespace_but_not_words() {
        let a = extract_claims(
            "<!--claim id=x confidence=structural cites=[]-->one two three<!--/claim-->",
            "t",
        );
        let b = extract_claims(
            "<!--claim id=x confidence=structural cites=[]-->one   two\n  three<!--/claim-->",
            "t",
        );
        let c = extract_claims(
            "<!--claim id=x confidence=structural cites=[]-->one two four<!--/claim-->",
            "t",
        );
        assert_eq!(a.claims[0].content_hash, b.claims[0].content_hash);
        assert_ne!(a.claims[0].content_hash, c.claims[0].content_hash);
    }

    #[test]
    fn warns_on_missing_required_field() {
        let ex = extract_claims(
            "<!--claim id=x confidence=structural-->no cites field<!--/claim-->",
            "t",
        );
        assert!(ex.claims.is_empty());
        assert_eq!(ex.warnings.len(), 1);
        assert!(ex.warnings[0].contains("missing required field `cites`"));
    }

    #[test]
    fn warns_on_empty_cites_for_non_structural_claim() {
        let ex = extract_claims(
            "<!--claim id=x confidence=established cites=[]-->thin<!--/claim-->",
            "t",
        );
        assert_eq!(ex.claims.len(), 1);
        assert_eq!(ex.warnings.len(), 1);
        assert!(ex.warnings[0].contains("only `structural`"));
    }

    #[test]
    fn warns_on_duplicate_id() {
        let body = "<!--claim id=x confidence=structural cites=[]-->one<!--/claim-->\n\
            <!--claim id=x confidence=structural cites=[]-->two<!--/claim-->";
        let ex = extract_claims(body, "t");
        assert_eq!(ex.claims.len(), 1);
        assert!(ex.warnings.iter().any(|w| w.contains("duplicate claim id")));
    }

    #[test]
    fn warns_on_unclosed_claim() {
        let ex = extract_claims(
            "<!--claim id=x confidence=structural cites=[]-->never closed",
            "t",
        );
        assert!(ex.claims.is_empty());
        assert!(ex.warnings.iter().any(|w| w.contains("never closed")));
    }

    #[test]
    fn warns_on_nested_claims() {
        let body = "<!--claim id=outer confidence=structural cites=[]-->outer \
            <!--claim id=inner confidence=structural cites=[]-->inner<!--/claim-->";
        let ex = extract_claims(body, "t");
        // The outer marker is skipped (overlap); the inner one is well-formed.
        assert!(ex.warnings.iter().any(|w| w.contains("overlaps or nests")));
        assert_eq!(ex.claims.len(), 1);
        assert_eq!(ex.claims[0].local_id, "inner");
    }

    #[test]
    fn warns_on_unknown_confidence() {
        let ex = extract_claims(
            "<!--claim id=x confidence=gospel cites=[]-->x<!--/claim-->",
            "t",
        );
        assert!(ex.claims.is_empty());
        assert!(ex.warnings.iter().any(|w| w.contains("unknown confidence")));
    }

    #[test]
    fn body_with_no_claims_yields_nothing() {
        let ex = extract_claims("Just ordinary prose with no markers.", "t");
        assert!(ex.claims.is_empty());
        assert!(ex.warnings.is_empty());
    }

    #[test]
    fn tracks_line_numbers_for_blame() {
        // Body has 7 lines; claim occupies lines 3–5 (opening marker line
        // through last prose line, inclusive).
        let body = concat!(
            "Line 1\n",
            "Line 2\n",
            "<!--claim id=x confidence=structural cites=[]-->\n",
            "Claim line 1\n",
            "Claim line 2\n",
            "<!--/claim-->\n",
            "Line 7",
        );
        let ex = extract_claims(body, "t");
        assert_eq!(ex.warnings, Vec::<String>::new());
        assert_eq!(ex.claims.len(), 1);
        let c = &ex.claims[0];
        // Opening marker is on line 3; content_start points to the '\n'
        // that ends that line → line_start = 3.
        assert_eq!(c.line_start, 3);
        // Last char of content is the '\n' after "Claim line 2" → line_end = 5.
        assert_eq!(c.line_end, 5);
        // published_at is None until blame_published_at is called.
        assert_eq!(c.published_at, None);
    }
}
