use wasm_bindgen::prelude::*;
use crate::{History, Op};

/// WASM-exposed edit-history wrapper. Owns the text and its undo/redo stack.
#[wasm_bindgen]
pub struct WasmHistory {
    inner: History,
}

#[wasm_bindgen]
impl WasmHistory {
    /// Create a new history with initial `text`.
    pub fn new(text: &str) -> WasmHistory {
        WasmHistory {
            inner: History::new(text),
        }
    }

    /// The current text.
    pub fn text(&self) -> String {
        self.inner.text().to_string()
    }

    /// The current monotonic version id.
    pub fn version(&self) -> u64 {
        self.inner.version().0
    }

    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }

    /// Commit an edit: at byte offset `at`, replace `removed` with `inserted`.
    /// Clears the redo stack.
    pub fn commit(&mut self, at: u32, removed: &str, inserted: &str) {
        let op = Op::replace(at as usize, removed, inserted);
        self.inner.commit(op);
    }

    /// Undo the most recent commit. Returns `true` if there was something to undo.
    pub fn undo(&mut self) -> bool {
        self.inner.undo()
    }

    /// Redo the most recently undone commit. Returns `true` if there was something to redo.
    pub fn redo(&mut self) -> bool {
        self.inner.redo()
    }
}
