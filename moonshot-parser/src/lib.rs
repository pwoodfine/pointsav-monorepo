//! moonshot-parser — span-based source tokenizer.
//!
//! Replaces tree-sitter and the hand-rolled JavaScript regex tokenizer in
//! `app-privategit-workbench` (which is brittle on nested constructs and bypasses
//! highlighting above a fixed file size). This v0 is the tokenizer foundation:
//! a single pass that classifies the source into [`Token`]s, each carrying its
//! exact byte [`Span`]. The token stream drives syntax highlighting — every token
//! maps to one colored span — and feeds source ranges to the document model.
//!
//! Two invariants, matching the source-anchoring philosophy of `moonshot-docengine`:
//!
//! 1. **Contiguous coverage.** Every byte of the source belongs to exactly one
//!    token (whitespace and newlines are tokens too), so concatenating the token
//!    spans reproduces the input — highlighting is exact and reversible.
//! 2. **UTF-8 safe.** Token boundaries land only on ASCII delimiters or full-char
//!    boundaries; multibyte content is spanned, never split.
//!
//! Zero dependencies, WASM-ready. Incremental re-lex (re-tokenizing only the lines
//! an edit touched, until the lexer state reconverges) is the documented next
//! increment; the single-pass [`tokenize`] here is its correctness oracle.

/// A byte range `[start, end)` into the source. (Mirrors `moonshot-docengine::Span`;
/// a shared span crate is a planned consolidation once the cores integrate.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }
}

/// Token classification, sufficient to drive a highlighter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    Ident,
    Number,
    Str,
    Comment,
    Punct,
    Whitespace,
}

/// A classified token plus its exact source span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// Source language. `Generic` skips keyword classification (everything word-like is
/// an `Ident`); add languages by extending [`is_keyword`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Rust,
    Generic,
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];

fn is_keyword(lang: Lang, word: &str) -> bool {
    match lang {
        Lang::Rust => RUST_KEYWORDS.contains(&word),
        Lang::Generic => false,
    }
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

fn is_ident_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Byte length of the UTF-8 char starting at `i` (1 if `i` is out of range or not a
/// char boundary head — defensive).
fn char_len(src: &str, i: usize) -> usize {
    src[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1)
}

/// Tokenize `source` in a single pass. The result contiguously covers the whole
/// source (see module invariants).
pub fn tokenize(source: &str, lang: Lang) -> Vec<Token> {
    let b = source.as_bytes();
    let n = b.len();
    let mut out = Vec::new();
    let mut i = 0;

    while i < n {
        let start = i;
        let c = b[i];

        if is_ws(c) {
            while i < n && is_ws(b[i]) {
                i += 1;
            }
            push(&mut out, TokenKind::Whitespace, start, i);
        } else if c == b'/' && i + 1 < n && b[i + 1] == b'/' {
            while i < n && b[i] != b'\n' {
                i += 1;
            }
            push(&mut out, TokenKind::Comment, start, i);
        } else if c == b'/' && i + 1 < n && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < n && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = if i + 1 < n { i + 2 } else { n }; // consume closing */ or run to EOF
            push(&mut out, TokenKind::Comment, start, i);
        } else if c == b'"' {
            i += 1;
            while i < n {
                if b[i] == b'\\' && i + 1 < n {
                    i += 2;
                    continue;
                }
                if b[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            push(&mut out, TokenKind::Str, start, i);
        } else if c.is_ascii_digit() {
            while i < n && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_') {
                i += 1;
            }
            push(&mut out, TokenKind::Number, start, i);
        } else if source[i..].chars().next().is_some_and(is_ident_start) {
            while let Some(ch) = source[i..].chars().next() {
                if is_ident_continue(ch) {
                    i += ch.len_utf8();
                } else {
                    break;
                }
            }
            let kind = if is_keyword(lang, &source[start..i]) {
                TokenKind::Keyword
            } else {
                TokenKind::Ident
            };
            push(&mut out, kind, start, i);
        } else {
            i += char_len(source, i); // single (possibly multibyte) punctuation char
            push(&mut out, TokenKind::Punct, start, i);
        }
    }
    out
}

fn push(out: &mut Vec<Token>, kind: TokenKind, start: usize, end: usize) {
    out.push(Token {
        kind,
        span: Span::new(start, end),
    });
}

/// Incrementally re-tokenize after an edit, reusing the tokens before it.
///
/// `old_tokens` are the tokens of the source *before* the edit; `new_source` is the
/// source *after*; `edit_start` is the byte offset where the edit began (the two
/// sources are identical below it). The result is identical to a full [`tokenize`]
/// of `new_source`, but the unchanged prefix is reused rather than re-lexed.
///
/// Correctness rests on the tokenizer being context-free at token boundaries: a
/// token's class depends only on the bytes from its own start, so any token boundary
/// at or before the edit is a safe restart point. v1 reuses the prefix and re-lexes
/// from the restart point to the end — which correctly captures forward-propagating
/// edits (e.g. typing `/*`). Reconvergence-based suffix reuse and a streaming lexer
/// are the documented next refinement; this function is paired with
/// `moonshot-docengine`'s `remap_span` seam.
pub fn retokenize(
    old_tokens: &[Token],
    new_source: &str,
    lang: Lang,
    edit_start: usize,
) -> Vec<Token> {
    // Restart at the start of the token that contains (or begins at) edit_start;
    // every earlier token lies in the unchanged region and is reused verbatim.
    let pre = old_tokens
        .iter()
        .position(|t| t.span.end > edit_start)
        .unwrap_or(old_tokens.len());
    let restart = old_tokens
        .get(pre)
        .map(|t| t.span.start)
        .unwrap_or(edit_start)
        .min(edit_start);

    let mut out: Vec<Token> = old_tokens[..pre].to_vec();
    for mut t in tokenize(&new_source[restart..], lang) {
        t.span.start += restart;
        t.span.end += restart;
        out.push(t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(toks: &[Token]) -> Vec<TokenKind> {
        toks.iter().map(|t| t.kind).collect()
    }

    /// Concatenating token spans must reproduce the source exactly.
    fn assert_covers(src: &str, lang: Lang) {
        let toks = tokenize(src, lang);
        let rebuilt: String = toks.iter().map(|t| t.span.text(src)).collect();
        assert_eq!(rebuilt, src, "token coverage is not contiguous for {src:?}");
    }

    #[test]
    fn classifies_rust_tokens() {
        let src = "let x = 42; // hi";
        let toks = tokenize(src, Lang::Rust);
        // let, ws, x, ws, =, ws, 42, ;, ws, //hi
        assert_eq!(toks[0].kind, TokenKind::Keyword);
        assert_eq!(toks[0].span.text(src), "let");
        assert!(toks
            .iter()
            .any(|t| t.kind == TokenKind::Number && t.span.text(src) == "42"));
        let comment = toks.iter().find(|t| t.kind == TokenKind::Comment).unwrap();
        assert_eq!(comment.span.text(src), "// hi");
        assert_covers(src, Lang::Rust);
    }

    #[test]
    fn block_comment_spans_multiple_lines() {
        let src = "a /* one\n two */ b";
        let toks = tokenize(src, Lang::Rust);
        let comment = toks.iter().find(|t| t.kind == TokenKind::Comment).unwrap();
        assert_eq!(comment.span.text(src), "/* one\n two */");
        assert_covers(src, Lang::Rust);
    }

    #[test]
    fn unterminated_block_comment_runs_to_eof() {
        let src = "x /* never closed";
        let toks = tokenize(src, Lang::Rust);
        let comment = toks.iter().find(|t| t.kind == TokenKind::Comment).unwrap();
        assert_eq!(comment.span.text(src), "/* never closed");
        assert_covers(src, Lang::Rust);
    }

    #[test]
    fn strings_handle_escaped_quotes() {
        let src = r#"s = "he\"llo";"#;
        let toks = tokenize(src, Lang::Rust);
        let string = toks.iter().find(|t| t.kind == TokenKind::Str).unwrap();
        assert_eq!(string.span.text(src), r#""he\"llo""#);
        assert_covers(src, Lang::Rust);
    }

    #[test]
    fn generic_lang_does_not_classify_keywords() {
        let src = "fn x";
        let rust = tokenize(src, Lang::Rust);
        let generic = tokenize(src, Lang::Generic);
        assert_eq!(rust[0].kind, TokenKind::Keyword);
        assert_eq!(generic[0].kind, TokenKind::Ident); // "fn" is just a word in Generic
    }

    #[test]
    fn utf8_content_is_spanned_not_split() {
        // The é and ☕ are multibyte; tokens must stay on char boundaries.
        let src = "let café = \"☕\"; // déjà";
        let toks = tokenize(src, Lang::Rust);
        assert_covers(src, Lang::Rust); // would panic on a mid-char slice
        assert!(toks
            .iter()
            .any(|t| t.kind == TokenKind::Ident && t.span.text(src) == "café"));
        let _ = kinds(&toks);
    }

    #[test]
    fn numbers_cover_hex_and_separators_and_suffixes() {
        for num in ["0xFF", "3.14", "1_000", "42u32"] {
            let toks = tokenize(num, Lang::Rust);
            assert_eq!(toks.len(), 1);
            assert_eq!(toks[0].kind, TokenKind::Number);
            assert_eq!(toks[0].span.text(num), num);
        }
    }

    /// The incremental result must always equal a full re-tokenize of the new source.
    fn assert_incremental_eq(old_src: &str, new_src: &str, edit_start: usize) {
        let old = tokenize(old_src, Lang::Rust);
        let inc = retokenize(&old, new_src, Lang::Rust, edit_start);
        let full = tokenize(new_src, Lang::Rust);
        assert_eq!(inc, full, "incremental != full for edit at {edit_start}");
    }

    #[test]
    fn retokenize_append_reuses_prefix() {
        let old_src = "let x = 1;";
        let new_src = "let x = 1; let y = 2;";
        assert_incremental_eq(old_src, new_src, old_src.len());
        // The unchanged prefix tokens are reused verbatim (object-equal to the old run).
        let old = tokenize(old_src, Lang::Rust);
        let inc = retokenize(&old, new_src, Lang::Rust, old_src.len());
        assert_eq!(&inc[..old.len()], &old[..]);
    }

    #[test]
    fn retokenize_mid_edit_matches_full() {
        let old_src = "let x = 1;\nlet y = 2;";
        // Replace the "1" with "111".
        let at = old_src.find('1').unwrap();
        let new_src = "let x = 111;\nlet y = 2;";
        assert_incremental_eq(old_src, new_src, at);
    }

    #[test]
    fn retokenize_at_start_is_still_correct() {
        // Restart point is 0 — nothing reused, but the result must still be right.
        assert_incremental_eq("abc def", "xabc def", 0);
    }

    #[test]
    fn retokenize_handles_forward_propagating_edit() {
        // Inserting "/* " turns the remainder into an (unterminated) block comment.
        // Re-lexing from before the edit to the end must capture that.
        let old_src = "a b c";
        let new_src = "a /* b c";
        let at = old_src.find('b').unwrap(); // edit begins where "b" was
        assert_incremental_eq(old_src, new_src, at);
        // Sanity: the new full tokenization really does have a trailing comment.
        let full = tokenize(new_src, Lang::Rust);
        assert!(full.iter().any(|t| t.kind == TokenKind::Comment));
    }
}

#[cfg(feature = "wasm")]
pub mod wasm;
