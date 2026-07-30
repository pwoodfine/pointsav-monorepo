//! app-workplace-aibridge — the AI section-edit bridge core.
//!
//! Implements the workbench's headline feature (req #4): highlight a section, hand
//! **only that section** to an external AI session, then apply the AI's replacement —
//! instead of re-running a whole file through a model. It composes
//! [`moonshot_docengine`] (snap a selection to its enclosing section, address the
//! source by byte span) and [`moonshot_crdt`] (apply the replacement as an undoable,
//! version-bumping edit). This is the deterministic Rust core the MCP server wraps;
//! the MCP wire protocol and the live Claude-session connection are the integration
//! layer (operator-verified, not headless-testable).
//!
//! **SYS-ADR-07.** Structured fiduciary/geometric schemas (proforma, schedule, GIS,
//! BIM) must never be routed through an AI layer — [`Bridge`] refuses those at every
//! tool entry point. Only prose/code/presentation content is eligible.

use moonshot_crdt::{History, Op, VersionId};
use moonshot_docengine::{Document, Span};

/// The schema of the buffer being edited. AI eligibility follows SYS-ADR-07.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schema {
    Prose,
    Code,
    Presentation,
    Proforma,
    Schedule,
    Gis,
    Bim,
    Pdf,
}

impl Schema {
    /// Whether this schema's content may be sent to an AI session. Structured
    /// fiduciary/geometric data may not (SYS-ADR-07).
    pub fn ai_eligible(self) -> bool {
        matches!(self, Schema::Prose | Schema::Code | Schema::Presentation)
    }

    pub fn name(self) -> &'static str {
        match self {
            Schema::Prose => "prose",
            Schema::Code => "code",
            Schema::Presentation => "presentation",
            Schema::Proforma => "proforma",
            Schema::Schedule => "schedule",
            Schema::Gis => "gis",
            Schema::Bim => "bim",
            Schema::Pdf => "pdf",
        }
    }
}

/// The isolated section handed to the AI: the snapped span plus its exact text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub span: Span,
    pub text: String,
}

/// Why a bridge operation was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// The schema is not AI-eligible (SYS-ADR-07); carries the schema name.
    SchemaForbidden(&'static str),
    /// The span is out of bounds or inverted relative to the current text.
    OutOfBounds,
}

/// A workbench buffer the AI can edit section-by-section, with full undo/redo.
pub struct Bridge {
    schema: Schema,
    history: History,
}

impl Bridge {
    pub fn new(schema: Schema, source: impl Into<String>) -> Self {
        Self {
            schema,
            history: History::new(source),
        }
    }

    pub fn schema(&self) -> Schema {
        self.schema
    }
    pub fn text(&self) -> &str {
        self.history.text()
    }
    pub fn version(&self) -> VersionId {
        self.history.version()
    }
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
    pub fn undo(&mut self) -> bool {
        self.history.undo()
    }
    pub fn redo(&mut self) -> bool {
        self.history.redo()
    }

    fn guard(&self) -> Result<(), BridgeError> {
        if self.schema.ai_eligible() {
            Ok(())
        } else {
            Err(BridgeError::SchemaForbidden(self.schema.name()))
        }
    }

    fn check_span(&self, span: Span) -> Result<(), BridgeError> {
        let len = self.history.text().len();
        if span.start <= span.end && span.end <= len {
            Ok(())
        } else {
            Err(BridgeError::OutOfBounds)
        }
    }

    /// **Tool `read_selection`.** Snap an arbitrary editor selection to its enclosing
    /// document section and return the isolated text — the exact unit handed to the
    /// AI. Refused for non-AI schemas.
    pub fn read_selection(&self, selection: Span) -> Result<Section, BridgeError> {
        self.guard()?;
        let doc = Document::parse(self.history.text());
        let span = doc.section_span(selection);
        self.check_span(span)?;
        Ok(Section {
            text: span.text(self.history.text()).to_string(),
            span,
        })
    }

    /// **Tool `propose_edit`.** Compute the buffer text that *would* result from
    /// replacing `span` with `new_text`, without committing. Lets the UI preview the
    /// AI's proposal before the operator accepts it.
    pub fn propose_edit(&self, span: Span, new_text: &str) -> Result<String, BridgeError> {
        self.guard()?;
        self.check_span(span)?;
        let text = self.history.text();
        let mut out = String::with_capacity(text.len() + new_text.len());
        out.push_str(&text[..span.start]);
        out.push_str(new_text);
        out.push_str(&text[span.end..]);
        Ok(out)
    }

    /// **Tool `commit_edit`.** Replace `span` with `new_text` as an undoable,
    /// version-bumping edit. Returns the new version id. Refused for non-AI schemas
    /// or stale spans.
    pub fn commit_edit(
        &mut self,
        span: Span,
        new_text: impl Into<String>,
    ) -> Result<VersionId, BridgeError> {
        self.guard()?;
        self.check_span(span)?;
        let removed = self.history.text()[span.start..span.end].to_string();
        self.history
            .commit(Op::replace(span.start, removed, new_text));
        Ok(self.history.version())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_eligibility_follows_sys_adr_07() {
        for s in [Schema::Prose, Schema::Code, Schema::Presentation] {
            assert!(s.ai_eligible(), "{} should be AI-eligible", s.name());
        }
        for s in [
            Schema::Proforma,
            Schema::Schedule,
            Schema::Gis,
            Schema::Bim,
            Schema::Pdf,
        ] {
            assert!(!s.ai_eligible(), "{} must be refused", s.name());
        }
    }

    #[test]
    fn read_selection_snaps_to_the_enclosing_section() {
        let src = "# Title\n\nfirst paragraph here\n\nsecond\n";
        let b = Bridge::new(Schema::Prose, src);
        let at = src.find("paragraph").unwrap();
        let section = b.read_selection(Span::new(at, at + 4)).unwrap();
        // Snapped to the whole "first paragraph here\n" block.
        assert_eq!(section.text, "first paragraph here\n");
    }

    #[test]
    fn structured_schemas_are_refused_at_every_entry() {
        let b = Bridge::new(Schema::Proforma, "{\"income\": 1}");
        assert_eq!(
            b.read_selection(Span::new(0, 1)),
            Err(BridgeError::SchemaForbidden("proforma"))
        );
        assert_eq!(
            b.propose_edit(Span::new(0, 1), "x"),
            Err(BridgeError::SchemaForbidden("proforma"))
        );
        let mut b = b;
        assert_eq!(
            b.commit_edit(Span::new(0, 1), "x"),
            Err(BridgeError::SchemaForbidden("proforma"))
        );
    }

    #[test]
    fn propose_edit_previews_without_changing_the_buffer() {
        let b = Bridge::new(Schema::Code, "let x = 1;");
        let preview = b.propose_edit(Span::new(8, 9), "42").unwrap();
        assert_eq!(preview, "let x = 42;");
        assert_eq!(b.text(), "let x = 1;"); // unchanged
        assert_eq!(b.version(), VersionId(0));
    }

    #[test]
    fn commit_edit_applies_bumps_version_and_is_undoable() {
        let src = "# Title\n\nold body\n";
        let mut b = Bridge::new(Schema::Prose, src);
        let section = b
            .read_selection(Span::new(
                src.find("old").unwrap(),
                src.find("old").unwrap(),
            ))
            .unwrap();
        let v = b.commit_edit(section.span, "new body\n").unwrap();
        assert!(b.text().contains("new body"));
        assert!(!b.text().contains("old body"));
        assert_eq!(v, VersionId(1));
        assert!(b.undo());
        assert_eq!(b.text(), src); // section edit fully reverted
    }

    #[test]
    fn out_of_bounds_span_is_rejected() {
        let mut b = Bridge::new(Schema::Code, "abc");
        assert_eq!(
            b.propose_edit(Span::new(0, 99), "x"),
            Err(BridgeError::OutOfBounds)
        );
        assert_eq!(
            b.commit_edit(Span::new(2, 1), "x"),
            Err(BridgeError::OutOfBounds)
        );
    }

    #[test]
    fn full_round_trip_read_then_commit_then_undo() {
        let src = "intro\n\ntarget paragraph\n\noutro\n";
        let mut b = Bridge::new(Schema::Prose, src);
        let at = src.find("target").unwrap();
        let section = b.read_selection(Span::new(at, at + 1)).unwrap();
        assert_eq!(section.text, "target paragraph\n");
        b.commit_edit(section.span, "REWRITTEN paragraph\n")
            .unwrap();
        assert!(b.text().contains("REWRITTEN"));
        b.undo();
        assert_eq!(b.text(), src);
    }
}
