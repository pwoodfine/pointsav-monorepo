use wasm_bindgen::prelude::*;
use crate::{Document, Edit, Span};

/// WASM-exposed wrapper around [`Document`]. Owns the source and block index.
#[wasm_bindgen]
pub struct WasmDocument {
    inner: Document,
}

#[wasm_bindgen]
impl WasmDocument {
    /// Parse `src` into a block-indexed document.
    pub fn parse(src: &str) -> WasmDocument {
        WasmDocument {
            inner: Document::parse(src),
        }
    }

    /// The full source string (byte-exact round-trip).
    pub fn source(&self) -> String {
        self.inner.source().to_string()
    }

    /// Snap `[sel_start, sel_end)` to the enclosing block boundaries.
    /// Returns a two-element `Uint32Array` `[snapped_start, snapped_end]`.
    pub fn section_span(&self, sel_start: u32, sel_end: u32) -> Box<[u32]> {
        let sel = Span::new(sel_start as usize, sel_end as usize);
        let snapped = self.inner.section_span(sel);
        Box::new([snapped.start as u32, snapped.end as u32])
    }

    /// Apply an edit (replace bytes `[at, at+del_len)` with `ins`) and return
    /// the re-parsed document. The original is unchanged.
    pub fn apply(&self, at: u32, del_len: u32, ins: &str) -> WasmDocument {
        let range = Span::new(at as usize, (at + del_len) as usize);
        let edit = Edit::new(range, ins);
        WasmDocument {
            inner: self.inner.apply(&edit),
        }
    }

    /// Map a source byte offset to its block index, or `u32::MAX` if past the end.
    pub fn block_at(&self, offset: u32) -> u32 {
        match self.inner.block_at(offset as usize) {
            Some(i) => i as u32,
            None => u32::MAX,
        }
    }

    /// Number of blocks in the document.
    pub fn block_count(&self) -> u32 {
        self.inner.blocks().len() as u32
    }
}
