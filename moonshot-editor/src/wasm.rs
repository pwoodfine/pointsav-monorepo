use wasm_bindgen::prelude::*;
use crate::{FileTree, NodeKind};

/// WASM-exposed arena-backed, virtualizable file tree.
#[wasm_bindgen]
pub struct WasmFileTree {
    inner: FileTree,
}

#[wasm_bindgen]
impl WasmFileTree {
    /// Create an empty file tree.
    pub fn new() -> WasmFileTree {
        WasmFileTree {
            inner: FileTree::new(),
        }
    }

    /// Add a top-level root node. Pass `is_dir = true` for directories.
    /// Returns the node id.
    pub fn add_root(&mut self, name: &str, is_dir: bool) -> u32 {
        let kind = if is_dir { NodeKind::Dir } else { NodeKind::File };
        self.inner.add_root(name, kind) as u32
    }

    /// Add a child under `parent`. Returns the new node id.
    pub fn add_child(&mut self, parent: u32, name: &str, is_dir: bool) -> u32 {
        let kind = if is_dir { NodeKind::Dir } else { NodeKind::File };
        self.inner.add_child(parent as usize, name, kind) as u32
    }

    /// Expand a directory. Returns `true` if a lazy fetch is needed (children
    /// not yet loaded). Returns `false` for files or already-loaded dirs.
    pub fn expand(&mut self, id: u32) -> bool {
        self.inner.expand(id as usize)
    }

    /// Collapse a directory.
    pub fn collapse(&mut self, id: u32) {
        self.inner.collapse(id as usize);
    }

    /// The visible window of rows for the current scroll position.
    /// Returns a JSON string: `{start, end, total_rows, pad_top, total_height}`.
    /// Call `JSON.parse()` in JS.
    pub fn visible_window(&self, scroll_top: u32, viewport_height: u32, row_height: u32) -> String {
        let w = self.inner.visible_window(
            scroll_top as usize,
            viewport_height as usize,
            row_height as usize,
        );
        format!(
            r#"{{"start":{},"end":{},"total_rows":{},"pad_top":{},"total_height":{}}}"#,
            w.start, w.end, w.total_rows, w.pad_top, w.total_height
        )
    }

    /// The name of the node with the given id.
    pub fn node_name(&self, id: u32) -> String {
        self.inner.node(id as usize).name.clone()
    }

    /// Whether the node is a directory.
    pub fn node_is_dir(&self, id: u32) -> bool {
        self.inner.node(id as usize).kind == crate::NodeKind::Dir
    }

    /// Total rows in the flattened visible tree.
    pub fn total_rows(&self) -> u32 {
        self.inner.flatten().len() as u32
    }
}
