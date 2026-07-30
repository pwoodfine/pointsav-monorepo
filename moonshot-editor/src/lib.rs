//! moonshot-editor — file-tree virtualization core.
//!
//! Replaces react-arborist / react-window (tree virtualization) and the current
//! `app-privategit-workbench` file tree, which creates one DOM node per file and
//! re-fetches every expanded directory on each toggle — the source of the "massive
//! delay" navigating large trees.
//!
//! The fix has two parts, both modelled here as dependency-free, WASM-ready logic:
//!
//! 1. **Lazy loading.** A directory's children are fetched only when it is first
//!    expanded ([`FileTree::expand`] returns whether a fetch is needed), not eagerly
//!    for the whole tree.
//! 2. **Virtualization.** Instead of materialising every visible node, the UI asks
//!    for the small window of rows that intersect the scroll viewport
//!    ([`FileTree::visible_window`]). Rendering cost becomes O(viewport), not O(tree).
//!
//! The tree is stored in an arena (`Vec<Node>` indexed by `usize` ids) so it is
//! cheap to clone, has no borrow-checker friction, and maps directly onto a flat
//! buffer when compiled to WebAssembly. The same core drives the browser prototype
//! today and the native os-workplace surface later.

/// File or directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Dir,
    File,
}

/// Lazy-load state of a directory's children. Files are always `Loaded`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    /// Children not fetched yet — expanding triggers a fetch.
    Unloaded,
    /// Fetch in flight.
    Loading,
    /// Children present (possibly zero).
    Loaded,
}

/// A node in the arena. Identified by its index in [`FileTree`].
#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub kind: NodeKind,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub expanded: bool,
    pub load_state: LoadState,
}

/// A flattened, visible row: a node plus its indentation depth. The flat list is
/// what a virtualized list renders from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub id: usize,
    pub depth: usize,
}

/// The window of rows to render for a scroll viewport, plus the spacer/extent the
/// UI needs to keep the scrollbar honest without materialising hidden rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// First flat-row index to render (inclusive).
    pub start: usize,
    /// One past the last flat-row index to render (exclusive).
    pub end: usize,
    /// Total visible rows in the flattened tree.
    pub total_rows: usize,
    /// Pixel offset of the first rendered row (top spacer height).
    pub pad_top: usize,
    /// Full scroll height in pixels (`total_rows * row_height`).
    pub total_height: usize,
}

/// An arena-backed, lazily-loaded, virtualizable file tree.
#[derive(Debug, Clone, Default)]
pub struct FileTree {
    nodes: Vec<Node>,
    roots: Vec<usize>,
    /// Rows above and below the viewport to render for smooth scrolling.
    overscan: usize,
}

impl FileTree {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            roots: Vec::new(),
            overscan: 3,
        }
    }

    pub fn with_overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    pub fn node(&self, id: usize) -> &Node {
        &self.nodes[id]
    }

    pub fn roots(&self) -> &[usize] {
        &self.roots
    }

    /// Add a top-level node. Directories start `Unloaded`; files `Loaded`.
    pub fn add_root(&mut self, name: impl Into<String>, kind: NodeKind) -> usize {
        let id = self.push(name, kind, None);
        self.roots.push(id);
        id
    }

    /// Add a child under `parent`. Marks the parent `Loaded` (the act of adding
    /// children is the completion of a fetch).
    pub fn add_child(&mut self, parent: usize, name: impl Into<String>, kind: NodeKind) -> usize {
        let id = self.push(name, kind, Some(parent));
        self.nodes[parent].children.push(id);
        self.nodes[parent].load_state = LoadState::Loaded;
        id
    }

    fn push(&mut self, name: impl Into<String>, kind: NodeKind, parent: Option<usize>) -> usize {
        let id = self.nodes.len();
        let load_state = match kind {
            NodeKind::Dir => LoadState::Unloaded,
            NodeKind::File => LoadState::Loaded,
        };
        self.nodes.push(Node {
            name: name.into(),
            kind,
            parent,
            children: Vec::new(),
            expanded: false,
            load_state,
        });
        id
    }

    /// Expand a directory. Returns `true` if its children still need to be fetched
    /// (it was `Unloaded`) — the lazy-load hook for the caller. Returns `false` for
    /// files or already-loaded dirs.
    pub fn expand(&mut self, id: usize) -> bool {
        let node = &mut self.nodes[id];
        if node.kind != NodeKind::Dir {
            return false;
        }
        node.expanded = true;
        if node.load_state == LoadState::Unloaded {
            node.load_state = LoadState::Loading;
            true
        } else {
            false
        }
    }

    pub fn collapse(&mut self, id: usize) {
        self.nodes[id].expanded = false;
    }

    /// Toggle expansion. Returns `true` if expanding triggered a lazy fetch.
    pub fn toggle(&mut self, id: usize) -> bool {
        if self.nodes[id].expanded {
            self.collapse(id);
            false
        } else {
            self.expand(id)
        }
    }

    /// Flatten the tree into the visible row list (pre-order DFS), descending into a
    /// directory only when it is expanded and loaded. This is what removes the
    /// O(tree) DOM cost: collapsed and unloaded subtrees contribute nothing.
    pub fn flatten(&self) -> Vec<Row> {
        let mut out = Vec::new();
        for &root in &self.roots {
            self.flatten_into(root, 0, &mut out);
        }
        out
    }

    fn flatten_into(&self, id: usize, depth: usize, out: &mut Vec<Row>) {
        out.push(Row { id, depth });
        let node = &self.nodes[id];
        if node.kind == NodeKind::Dir && node.expanded && node.load_state == LoadState::Loaded {
            for &child in &node.children {
                self.flatten_into(child, depth + 1, out);
            }
        }
    }

    /// Compute the row window to render for a scroll viewport. `row_height` must be
    /// non-zero. The returned `[start, end)` covers every row intersecting the
    /// viewport plus `overscan` rows on each side, clamped to the flattened length.
    pub fn visible_window(
        &self,
        scroll_top: usize,
        viewport_height: usize,
        row_height: usize,
    ) -> Window {
        let total_rows = self.flatten().len();
        if row_height == 0 {
            return Window {
                start: 0,
                end: total_rows,
                total_rows,
                pad_top: 0,
                total_height: 0,
            };
        }
        let first_visible = scroll_top / row_height;
        let last_visible = (scroll_top + viewport_height) / row_height;
        let start = first_visible.saturating_sub(self.overscan).min(total_rows);
        let end = (last_visible + 1 + self.overscan)
            .min(total_rows)
            .max(start);
        Window {
            start,
            end,
            total_rows,
            pad_top: start * row_height,
            total_height: total_rows * row_height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// root/
    ///   src/        (dir)
    ///     lib.rs
    ///     main.rs
    ///   README.md
    fn sample() -> (FileTree, usize, usize) {
        let mut t = FileTree::new();
        let root = t.add_root("root", NodeKind::Dir);
        let src = t.add_child(root, "src", NodeKind::Dir);
        t.add_child(src, "lib.rs", NodeKind::File);
        t.add_child(src, "main.rs", NodeKind::File);
        t.add_child(root, "README.md", NodeKind::File);
        (t, root, src)
    }

    #[test]
    fn collapsed_dirs_hide_their_children() {
        let (mut t, root, _src) = sample();
        // Nothing expanded: only the root row is visible.
        assert_eq!(t.flatten().len(), 1);
        t.expand(root);
        // root expanded: src + README.md visible, but src's children still hidden.
        let rows = t.flatten();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[1].depth, 1);
    }

    #[test]
    fn expanding_loaded_dir_reveals_children_with_depth() {
        let (mut t, root, src) = sample();
        t.expand(root);
        t.expand(src);
        let rows = t.flatten();
        // root, src, lib.rs, main.rs, README.md
        assert_eq!(rows.len(), 5);
        let names: Vec<&str> = rows.iter().map(|r| t.node(r.id).name.as_str()).collect();
        assert_eq!(names, ["root", "src", "lib.rs", "main.rs", "README.md"]);
        // lib.rs / main.rs are depth 2.
        assert_eq!(rows[2].depth, 2);
        assert_eq!(rows[3].depth, 2);
    }

    #[test]
    fn expanding_unloaded_dir_signals_a_fetch_and_hides_children() {
        let mut t = FileTree::new();
        let root = t.add_root("root", NodeKind::Dir);
        // A dir we have not fetched children for yet.
        let lazy = t.add_child(root, "node_modules", NodeKind::Dir);
        t.expand(root);
        // Expanding the unloaded dir asks the caller to fetch.
        assert!(
            t.expand(lazy),
            "expanding an Unloaded dir must request a fetch"
        );
        // Until children arrive, the lazy dir contributes only itself.
        assert_eq!(t.flatten().len(), 2);
        // Children arrive (the fetch completes).
        t.add_child(lazy, "left-pad", NodeKind::Dir);
        assert_eq!(t.flatten().len(), 3);
        // A second expand of a now-loaded dir does not request another fetch.
        t.collapse(lazy);
        assert!(!t.expand(lazy));
    }

    #[test]
    fn visible_window_renders_only_the_viewport_plus_overscan() {
        // 100 flat rows, all visible.
        let mut t = FileTree::new().with_overscan(2);
        let root = t.add_root("root", NodeKind::Dir);
        for i in 0..99 {
            t.add_child(root, format!("f{i}"), NodeKind::File);
        }
        t.expand(root);
        assert_eq!(t.flatten().len(), 100);

        // viewport 100px, rows 20px => ~5 rows on screen; scrolled to 200px.
        let w = t.visible_window(200, 100, 20);
        // first_visible=10, last_visible=15, overscan 2 => [8, 18)
        assert_eq!((w.start, w.end), (8, 18));
        assert_eq!(w.total_rows, 100);
        assert_eq!(w.pad_top, 160); // 8 * 20
        assert_eq!(w.total_height, 2000); // 100 * 20
                                          // The rendered slice is tiny relative to the tree.
        assert!(w.end - w.start <= 12);
    }

    #[test]
    fn visible_window_clamps_at_both_ends() {
        let mut t = FileTree::new().with_overscan(3);
        let root = t.add_root("root", NodeKind::Dir);
        for i in 0..9 {
            t.add_child(root, format!("f{i}"), NodeKind::File);
        }
        t.expand(root);
        let total = t.flatten().len(); // 10
                                       // Scrolled past the end: window collapses to an empty range at the tail.
        let w = t.visible_window(100_000, 100, 20);
        assert!(w.start <= total && w.end <= total && w.start <= w.end);
        // At the very top, start is 0.
        let top = t.visible_window(0, 100, 20);
        assert_eq!(top.start, 0);
    }
}
