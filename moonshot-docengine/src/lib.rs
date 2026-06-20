//! moonshot-docengine — source-anchored document model.
//!
//! Replaces ProseMirror / Lexical / TipTap and the hand-rolled JavaScript markdown
//! parser. The design goal is a model that gives **exact bidirectional mapping**
//! between a rendered (WYSIWYG) view and the canonical source text, which is the
//! foundation for Figma-style "edit in either view" and for isolating a highlighted
//! section to hand to an external AI session.
//!
//! The key idea: the model is a *structured index over the source bytes*, not a
//! separate copy. Every [`Block`] and [`Inline`] node carries the exact [`Span`]
//! (byte range) it came from. Two properties follow for free:
//!
//! 1. **Exact round-trip.** Concatenating every block's source slice reproduces the
//!    input byte-for-byte ([`Document::to_source`]). There is no lossy re-serialization.
//! 2. **Exact bidirectional mapping.** A click in the rendered view maps to a node,
//!    whose `span` is the precise source range to highlight; a cursor offset in the
//!    source maps back to the enclosing node ([`Document::block_at`]). This replaces
//!    the previous fuzzy text-matching synchronization.
//!
//! The crate is `no_std`-friendly in spirit (only `alloc`/`std` `Vec`/`String`) and
//! has zero dependencies, so it compiles cleanly to WebAssembly to drive the browser
//! prototype today and the native os-workplace surface later from the same core.
//!
//! The markdown subset parsed here (ATX headings, fenced code, blockquotes, paragraphs,
//! and `**strong**` / `*emphasis*` / `` `code` `` inline runs) is intentionally small
//! and is the documented extension point — new node kinds are added without changing
//! the source-anchoring invariant.

/// A byte range `[start, end)` into a document's source string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
    /// True if `offset` falls within `[start, end)`.
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }
    /// The source slice this span points at.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }
}

/// A block-level node. `Heading`, `Paragraph` and `BlockQuote` carry a `content`
/// span (the inline text, with the block marker and trailing newline excluded) so
/// inline parsing and rendering operate on exactly the right bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Heading {
        level: u8,
        content: Span,
    },
    Paragraph {
        content: Span,
    },
    /// Fenced code block. `info` is the language/info string after the opening
    /// fence; `body` is the code between the fences.
    CodeBlock {
        info: Span,
        body: Span,
    },
    BlockQuote {
        content: Span,
    },
    /// One or more consecutive blank lines (preserved so round-trip is exact).
    Blank,
}

/// A block node plus the full source span it covers (marker + content + newline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub span: Span,
}

/// Inline node kind within a text block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineKind {
    Text,
    Emphasis,
    Strong,
    Code,
}

/// An inline node. `span` covers the whole run including delimiters (for selecting
/// it in the source); `content` is the inner text (for rendering).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inline {
    pub kind: InlineKind,
    pub span: Span,
    pub content: Span,
}

/// A parsed document: the owned source plus a source-anchored block index.
#[derive(Debug, Clone)]
pub struct Document {
    source: String,
    blocks: Vec<Block>,
}

impl Document {
    /// Parse `source` into a block index. The parse contiguously covers every byte
    /// of the source, which is what guarantees [`to_source`](Document::to_source)
    /// is exact.
    pub fn parse(source: &str) -> Document {
        let blocks = parse_blocks(source);
        Document {
            source: source.to_string(),
            blocks,
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// Reconstruct the source from the block index. Always byte-for-byte equal to
    /// the original input — the round-trip invariant.
    pub fn to_source(&self) -> String {
        let mut out = String::with_capacity(self.source.len());
        for b in &self.blocks {
            out.push_str(b.span.text(&self.source));
        }
        out
    }

    /// Map a source byte offset to the index of the block that contains it
    /// (editor cursor -> rendered node). Returns `None` past the end of source.
    pub fn block_at(&self, offset: usize) -> Option<usize> {
        self.blocks.iter().position(|b| b.span.contains(offset))
    }

    /// Parse the inline runs of a text block (rendered-view children, each carrying
    /// its exact source span). Non-text blocks return a single `Text` node over
    /// their content/body so callers can treat the result uniformly.
    pub fn inlines(&self, block: &Block) -> Vec<Inline> {
        let content = match &block.kind {
            BlockKind::Heading { content, .. } => *content,
            BlockKind::Paragraph { content } => *content,
            BlockKind::BlockQuote { content } => *content,
            BlockKind::CodeBlock { body, .. } => {
                return vec![Inline {
                    kind: InlineKind::Text,
                    span: *body,
                    content: *body,
                }];
            }
            BlockKind::Blank => return Vec::new(),
        };
        parse_inlines(&self.source, content)
    }

    /// Snap an arbitrary source selection to a clean "section" span: the union of
    /// every block the selection touches. This is the stable section handle the AI
    /// bridge operates on — a partial highlight in the viewer becomes the full
    /// enclosing block(s) in the source. An empty selection snaps to its block.
    pub fn section_span(&self, selection: Span) -> Span {
        let rs = selection.start;
        let re = selection.end.max(selection.start);
        let mut first: Option<usize> = None;
        let mut last: Option<usize> = None;
        for (idx, b) in self.blocks.iter().enumerate() {
            let overlaps = if rs == re {
                b.span.contains(rs)
            } else {
                b.span.start < re && b.span.end > rs
            };
            if overlaps {
                if first.is_none() {
                    first = Some(idx);
                }
                last = Some(idx);
            }
        }
        match (first, last) {
            (Some(f), Some(l)) => Span::new(self.blocks[f].span.start, self.blocks[l].span.end),
            _ => selection,
        }
    }
}

// ---- edits ------------------------------------------------------------------

/// A text edit: replace the bytes in `range` (a span in the current source) with
/// `replacement`. Insertion is an empty `range`; deletion is an empty `replacement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub range: Span,
    pub replacement: String,
}

impl Edit {
    pub fn new(range: Span, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
        }
    }

    /// Signed change in byte length the edit introduces.
    pub fn delta(&self) -> isize {
        self.replacement.len() as isize - self.range.len() as isize
    }
}

/// Remap a span from old-source to new-source coordinates across `edit`: a span
/// entirely before the edit is unchanged; one entirely after shifts by the edit's
/// delta; one overlapping the edited range is invalidated (`None`) and must be
/// re-derived. This is the shared primitive that makes incremental updates cheap —
/// `moonshot-parser` tokens and `moonshot-editor` rows remap with the *same* call,
/// because all three cores address the source by byte span. Reuse every span this
/// keeps; re-derive only the ones it drops.
pub fn remap_span(span: Span, edit: &Edit) -> Option<Span> {
    if span.end <= edit.range.start {
        Some(span)
    } else if span.start >= edit.range.end {
        let shift = |x: usize| (x as isize + edit.delta()) as usize;
        Some(Span::new(shift(span.start), shift(span.end)))
    } else {
        None
    }
}

impl Document {
    /// The source string after applying `edit` (a pure splice; does not parse).
    pub fn source_after(&self, edit: &Edit) -> String {
        let s = &self.source;
        let cap = (s.len() as isize + edit.delta()).max(0) as usize;
        let mut out = String::with_capacity(cap);
        out.push_str(&s[..edit.range.start]);
        out.push_str(&edit.replacement);
        out.push_str(&s[edit.range.end..]);
        out
    }

    /// Apply `edit` and return the re-parsed document. The re-parse is full for now;
    /// [`remap_span`] is the seam for incremental re-derivation (reuse remappable
    /// blocks/tokens, re-derive only those overlapping the edit).
    pub fn apply(&self, edit: &Edit) -> Document {
        Document::parse(&self.source_after(edit))
    }
}

// ---- parsing internals ------------------------------------------------------

/// Source spans of each line, including the trailing `\n`. The final line has no
/// `\n` if the source does not end with one. Contiguously covers the whole source.
fn line_spans(src: &str) -> Vec<Span> {
    let bytes = src.as_bytes();
    let mut spans = Vec::new();
    let mut start = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            spans.push(Span::new(start, i + 1));
            start = i + 1;
        }
    }
    if start < bytes.len() {
        spans.push(Span::new(start, bytes.len()));
    }
    spans
}

/// The line's text with a single trailing `\n` (and preceding `\r`) removed.
fn line_content(src: &str, line: Span) -> &str {
    let t = line.text(src);
    let t = t.strip_suffix('\n').unwrap_or(t);
    t.strip_suffix('\r').unwrap_or(t)
}

/// ATX heading level (1–6) if the line content is a heading, else `None`.
fn heading_level(content: &str) -> Option<u8> {
    let bytes = content.as_bytes();
    let mut n = 0usize;
    while n < bytes.len() && bytes[n] == b'#' {
        n += 1;
    }
    if (1..=6).contains(&n) && (n == bytes.len() || bytes[n] == b' ') {
        Some(n as u8)
    } else {
        None
    }
}

/// Move `end` back past a single trailing newline so a block's `content` span
/// excludes it.
fn trim_trailing_newline(src: &str, end: usize) -> usize {
    let b = src.as_bytes();
    let mut e = end;
    if e > 0 && b[e - 1] == b'\n' {
        e -= 1;
        if e > 0 && b[e - 1] == b'\r' {
            e -= 1;
        }
    }
    e
}

fn is_blank(content: &str) -> bool {
    content.trim().is_empty()
}

fn is_fence(content: &str) -> bool {
    content.trim_start().starts_with("```")
}

fn is_quote(content: &str) -> bool {
    content.trim_start().starts_with('>')
}

fn parse_blocks(src: &str) -> Vec<Block> {
    let lines = line_spans(src);
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let content = line_content(src, line);

        if is_blank(content) {
            let start = line.start;
            let mut end = line.end;
            i += 1;
            while i < lines.len() && is_blank(line_content(src, lines[i])) {
                end = lines[i].end;
                i += 1;
            }
            blocks.push(Block {
                kind: BlockKind::Blank,
                span: Span::new(start, end),
            });
        } else if let Some(level) = heading_level(content) {
            let mut k = level as usize;
            let cbytes = content.as_bytes();
            while k < cbytes.len() && cbytes[k] == b' ' {
                k += 1;
            }
            let content_span = Span::new(line.start + k, line.start + content.len());
            blocks.push(Block {
                kind: BlockKind::Heading {
                    level,
                    content: content_span,
                },
                span: line,
            });
            i += 1;
        } else if is_fence(content) {
            let start = line.start;
            let lead = content.len() - content.trim_start().len();
            let info = Span::new(line.start + lead + 3, line.start + content.len());
            let body_start = line.end;
            i += 1;
            let mut body_end = body_start;
            let mut end = line.end;
            let mut closed = false;
            while i < lines.len() {
                let l = lines[i];
                if is_fence(line_content(src, l)) {
                    body_end = l.start;
                    end = l.end;
                    i += 1;
                    closed = true;
                    break;
                }
                end = l.end;
                i += 1;
            }
            if !closed {
                body_end = end;
            }
            blocks.push(Block {
                kind: BlockKind::CodeBlock {
                    info,
                    body: Span::new(body_start, body_end),
                },
                span: Span::new(start, end),
            });
        } else if is_quote(content) {
            let start = line.start;
            let mut end = line.end;
            i += 1;
            while i < lines.len() && is_quote(line_content(src, lines[i])) {
                end = lines[i].end;
                i += 1;
            }
            blocks.push(Block {
                kind: BlockKind::BlockQuote {
                    content: Span::new(start, trim_trailing_newline(src, end)),
                },
                span: Span::new(start, end),
            });
        } else {
            let start = line.start;
            let mut end = line.end;
            i += 1;
            while i < lines.len() {
                let c = line_content(src, lines[i]);
                if is_blank(c) || heading_level(c).is_some() || is_fence(c) || is_quote(c) {
                    break;
                }
                end = lines[i].end;
                i += 1;
            }
            blocks.push(Block {
                kind: BlockKind::Paragraph {
                    content: Span::new(start, trim_trailing_newline(src, end)),
                },
                span: Span::new(start, end),
            });
        }
    }
    blocks
}

fn find_byte(bytes: &[u8], target: u8, from: usize, end: usize) -> Option<usize> {
    (from..end).find(|&i| bytes[i] == target)
}

/// Find the start index of the next `target target` pair in `[from, end)`.
fn find_pair(bytes: &[u8], target: u8, from: usize, end: usize) -> Option<usize> {
    if end == 0 {
        return None;
    }
    (from..end.saturating_sub(1)).find(|&i| bytes[i] == target && bytes[i + 1] == target)
}

/// Parse inline runs within `span`, emitting nodes with absolute source spans.
/// Delimiters are all ASCII, so byte scanning is UTF-8 safe (multibyte content is
/// only ever spanned, never split mid-character).
fn parse_inlines(src: &str, span: Span) -> Vec<Inline> {
    let bytes = src.as_bytes();
    let (s, e) = (span.start, span.end);
    let mut out: Vec<Inline> = Vec::new();
    let mut pos = s;
    let mut text_start = s;

    fn flush(out: &mut Vec<Inline>, start: usize, end: usize) {
        if end > start {
            out.push(Inline {
                kind: InlineKind::Text,
                span: Span::new(start, end),
                content: Span::new(start, end),
            });
        }
    }

    while pos < e {
        let b = bytes[pos];
        if b == b'`' {
            if let Some(j) = find_byte(bytes, b'`', pos + 1, e) {
                flush(&mut out, text_start, pos);
                out.push(Inline {
                    kind: InlineKind::Code,
                    span: Span::new(pos, j + 1),
                    content: Span::new(pos + 1, j),
                });
                pos = j + 1;
                text_start = pos;
                continue;
            }
        } else if b == b'*' {
            if pos + 1 < e && bytes[pos + 1] == b'*' {
                if let Some(j) = find_pair(bytes, b'*', pos + 2, e) {
                    flush(&mut out, text_start, pos);
                    out.push(Inline {
                        kind: InlineKind::Strong,
                        span: Span::new(pos, j + 2),
                        content: Span::new(pos + 2, j),
                    });
                    pos = j + 2;
                    text_start = pos;
                    continue;
                }
            } else if let Some(j) = find_byte(bytes, b'*', pos + 1, e) {
                flush(&mut out, text_start, pos);
                out.push(Inline {
                    kind: InlineKind::Emphasis,
                    span: Span::new(pos, j + 1),
                    content: Span::new(pos + 1, j),
                });
                pos = j + 1;
                text_start = pos;
                continue;
            }
        }
        pos += 1;
    }
    flush(&mut out, text_start, e);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(s: &str) -> Document {
        Document::parse(s)
    }

    #[test]
    fn round_trip_is_byte_exact() {
        let samples = [
            "# Title\n\nA paragraph with **bold**, *em* and `code`.\n\n```rust\nfn main() {}\n```\n",
            "para one\ncontinues here\n\n> a quote\n> second line\n",
            "no trailing newline",
            "",
            "\n\n\n",
            "# café — unicode ☕ stays intact\n",
        ];
        for s in samples {
            assert_eq!(doc(s).to_source(), *s, "round-trip failed for {s:?}");
        }
    }

    #[test]
    fn classifies_block_kinds() {
        let d = doc("# H1\n\npara line\n\n```\ncode\n```\n");
        assert_eq!(d.blocks().len(), 5);
        assert!(matches!(
            d.blocks()[0].kind,
            BlockKind::Heading { level: 1, .. }
        ));
        assert!(matches!(d.blocks()[1].kind, BlockKind::Blank));
        assert!(matches!(d.blocks()[2].kind, BlockKind::Paragraph { .. }));
        assert!(matches!(d.blocks()[3].kind, BlockKind::Blank));
        assert!(matches!(d.blocks()[4].kind, BlockKind::CodeBlock { .. }));
    }

    #[test]
    fn heading_content_excludes_marker() {
        let s = "## Hello World\n";
        let d = doc(s);
        if let BlockKind::Heading { level, content } = d.blocks()[0].kind {
            assert_eq!(level, 2);
            assert_eq!(content.text(s), "Hello World");
        } else {
            panic!("expected heading");
        }
    }

    #[test]
    fn block_at_maps_source_offset_to_block() {
        let s = "# Title\n\nbody text here\n";
        let d = doc(s);
        let off = s.find("body").unwrap();
        let bi = d.block_at(off).expect("offset in a block");
        assert!(matches!(d.blocks()[bi].kind, BlockKind::Paragraph { .. }));
    }

    #[test]
    fn inline_runs_carry_exact_source_spans() {
        let s = "x **bold** y `code` z *em*";
        let d = doc(s);
        let inl = d.inlines(&d.blocks()[0]);
        let strong = inl.iter().find(|i| i.kind == InlineKind::Strong).unwrap();
        assert_eq!(strong.span.text(s), "**bold**");
        assert_eq!(strong.content.text(s), "bold");
        let code = inl.iter().find(|i| i.kind == InlineKind::Code).unwrap();
        assert_eq!(code.content.text(s), "code");
        let em = inl.iter().find(|i| i.kind == InlineKind::Emphasis).unwrap();
        assert_eq!(em.content.text(s), "em");
        // Plain text runs are preserved too, so the inline list round-trips.
        let rebuilt: String = inl.iter().map(|i| i.span.text(s)).collect();
        assert_eq!(rebuilt, s);
    }

    #[test]
    fn section_span_snaps_partial_selection_to_block() {
        let s = "# Title\n\nfirst paragraph here\n\nsecond\n";
        let d = doc(s);
        let start = s.find("paragraph").unwrap();
        let selection = Span::new(start, start + 4); // mid-paragraph, 4 bytes
        let section = d.section_span(selection);
        let para = d
            .blocks()
            .iter()
            .find(|b| matches!(b.kind, BlockKind::Paragraph { .. }))
            .unwrap();
        assert_eq!(section, para.span);
    }

    #[test]
    fn section_span_unions_blocks_a_selection_crosses() {
        let s = "alpha\n\nbravo\n";
        let d = doc(s);
        // Selection from inside block 0 to inside block 2 (the two paragraphs).
        let sel = Span::new(1, s.find("bravo").unwrap() + 1);
        let section = d.section_span(sel);
        assert_eq!(section.start, 0);
        assert_eq!(section.end, s.len());
    }

    #[test]
    fn apply_splices_insert_delete_replace() {
        let d = doc("hello world");
        assert_eq!(
            d.source_after(&Edit::new(Span::new(6, 11), "rust")),
            "hello rust"
        );
        assert_eq!(
            d.source_after(&Edit::new(Span::new(11, 11), "!")),
            "hello world!"
        );
        assert_eq!(d.source_after(&Edit::new(Span::new(0, 6), "")), "world");
    }

    #[test]
    fn apply_preserves_round_trip() {
        let d = doc("# Title\n\nbody\n");
        let e = Edit::new(Span::new(0, 0), "draft\n\n");
        let d2 = d.apply(&e);
        assert_eq!(d2.to_source(), d.source_after(&e));
    }

    #[test]
    fn remap_span_before_after_and_overlap() {
        // Replace bytes [5,8) (len 3) with 1 byte => delta -2.
        let e = Edit::new(Span::new(5, 8), "x");
        assert_eq!(e.delta(), -2);
        assert_eq!(remap_span(Span::new(0, 4), &e), Some(Span::new(0, 4))); // before
        assert_eq!(remap_span(Span::new(10, 12), &e), Some(Span::new(8, 10))); // after, shifted
        assert_eq!(remap_span(Span::new(6, 9), &e), None); // overlaps
        assert_eq!(remap_span(Span::new(2, 5), &e), Some(Span::new(2, 5))); // touches start = before
    }

    #[test]
    fn incremental_remap_matches_full_reparse_for_in_block_edit() {
        // Typing inside one paragraph must leave earlier blocks byte-identical and
        // shift later blocks by exactly the edit delta — i.e. remapping the old
        // block spans equals a full re-parse of the new source.
        let s = "# Title\n\nfirst para\n\nsecond para\n\nthird para\n";
        let d = doc(s);
        let at = s.find("second").unwrap() + "second".len();
        let e = Edit::new(Span::new(at, at), " EDIT"); // insert inside "second para"
        let reparsed = d.apply(&e);
        assert_eq!(reparsed.blocks().len(), d.blocks().len()); // no new boundaries
        for (idx, old) in d.blocks().iter().enumerate() {
            match remap_span(old.span, &e) {
                Some(rs) => assert_eq!(reparsed.blocks()[idx].span, rs), // reused 1:1
                None => assert!(reparsed.blocks()[idx]
                    .span
                    .text(reparsed.source())
                    .contains("EDIT")), // the edited block
            }
        }
    }
}

#[cfg(feature = "wasm")]
pub mod wasm;
