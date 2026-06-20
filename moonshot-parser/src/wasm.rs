use wasm_bindgen::prelude::*;
use crate::{tokenize, retokenize, Lang, TokenKind};

fn lang_from_str(s: &str) -> Lang {
    match s {
        "rust" => Lang::Rust,
        _ => Lang::Generic,
    }
}

fn kind_str(k: TokenKind) -> &'static str {
    match k {
        TokenKind::Keyword => "Keyword",
        TokenKind::Ident => "Ident",
        TokenKind::Number => "Number",
        TokenKind::Str => "Str",
        TokenKind::Comment => "Comment",
        TokenKind::Punct => "Punct",
        TokenKind::Whitespace => "Whitespace",
    }
}

fn tokens_to_json(tokens: &[crate::Token]) -> String {
    let mut out = String::from("[");
    for (i, t) in tokens.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            r#"{{"kind":"{}","start":{},"end":{}}}"#,
            kind_str(t.kind),
            t.span.start,
            t.span.end
        ));
    }
    out.push(']');
    out
}

/// Tokenize `src` in `lang` ("rust" | "generic").
/// Returns a JSON string: `[{kind, start, end}, ...]` — call `JSON.parse()` in JS.
#[wasm_bindgen]
pub fn wasm_tokenize(src: &str, lang: &str) -> String {
    let tokens = tokenize(src, lang_from_str(lang));
    tokens_to_json(&tokens)
}

/// Incremental retokenize after an edit. `old_src` is the source before the edit,
/// `new_src` is after, `edit_start` is where the edit began.
/// Returns the same JSON format as `wasm_tokenize`.
#[wasm_bindgen]
pub fn wasm_retokenize(old_src: &str, new_src: &str, lang: &str, edit_start: u32) -> String {
    let l = lang_from_str(lang);
    let old_tokens = tokenize(old_src, l);
    let tokens = retokenize(&old_tokens, new_src, l, edit_start as usize);
    tokens_to_json(&tokens)
}
