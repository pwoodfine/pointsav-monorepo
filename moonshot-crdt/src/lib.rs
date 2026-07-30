//! moonshot-crdt — edit history: undo/redo + version lineage.
//!
//! Replaces the history/undo facilities of Loro / Yjs / Automerge. v0 is the local
//! history core: a reversible [`Op`] log with linear undo/redo and a monotonic
//! version id per state. The conflict-free concurrent-merge layer (a sequence CRDT —
//! RGA or Logoot-style — that lets two replicas edit offline and converge) is the
//! documented next layer; it slots in alongside this history without changing it.
//!
//! An [`Op`] mirrors `moonshot-docengine::Edit` but is *reversible* — it records both
//! the text it `inserted` and the text it `removed`, so `op.inverse()` exactly undoes
//! it. That invertibility is what makes undo/redo correct by construction. Zero
//! dependencies, WASM-ready.

/// A reversible text operation: at byte offset `at`, the bytes `removed` were
/// replaced by `inserted`. Insertion has empty `removed`; deletion empty `inserted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Op {
    pub at: usize,
    pub removed: String,
    pub inserted: String,
}

impl Op {
    pub fn insert(at: usize, text: impl Into<String>) -> Self {
        Op {
            at,
            removed: String::new(),
            inserted: text.into(),
        }
    }

    pub fn delete(at: usize, removed: impl Into<String>) -> Self {
        Op {
            at,
            removed: removed.into(),
            inserted: String::new(),
        }
    }

    pub fn replace(at: usize, removed: impl Into<String>, inserted: impl Into<String>) -> Self {
        Op {
            at,
            removed: removed.into(),
            inserted: inserted.into(),
        }
    }

    /// The operation that exactly undoes this one when applied to the post-`self`
    /// text: swap `removed` and `inserted` at the same offset.
    pub fn inverse(&self) -> Op {
        Op {
            at: self.at,
            removed: self.inserted.clone(),
            inserted: self.removed.clone(),
        }
    }

    /// Apply to `text`, splicing `inserted` in place of the `removed`-length range
    /// at `at`. (`commit`/`undo`/`redo` uphold the precondition that the spliced
    /// range actually holds `removed`.)
    pub fn apply(&self, text: &str) -> String {
        let end = self.at + self.removed.len();
        let mut out = String::with_capacity(text.len() + self.inserted.len());
        out.push_str(&text[..self.at]);
        out.push_str(&self.inserted);
        out.push_str(&text[end..]);
        out
    }
}

/// A monotonic version identifier — increments on every committed state change
/// (commit, undo, redo). The basis for ledger-anchorable lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VersionId(pub u64);

/// Owns a text document plus its undo/redo history and version lineage.
#[derive(Debug, Clone)]
pub struct History {
    text: String,
    done: Vec<Op>,
    undone: Vec<Op>,
    version: u64,
    /// The sequence of version ids visited, oldest first — the lineage trail.
    lineage: Vec<u64>,
}

impl History {
    pub fn new(text: impl Into<String>) -> Self {
        History {
            text: text.into(),
            done: Vec::new(),
            undone: Vec::new(),
            version: 0,
            lineage: vec![0],
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn version(&self) -> VersionId {
        VersionId(self.version)
    }

    /// The version ids visited so far, oldest first.
    pub fn lineage(&self) -> Vec<VersionId> {
        self.lineage.iter().copied().map(VersionId).collect()
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    fn bump(&mut self) {
        self.version += 1;
        self.lineage.push(self.version);
    }

    /// Apply `op`, push it to the undo stack, and clear the redo stack (a fresh edit
    /// branches away from any undone future). Advances the version.
    pub fn commit(&mut self, op: Op) {
        debug_assert_eq!(
            &self.text[op.at..op.at + op.removed.len()],
            op.removed,
            "Op.removed must match the text being replaced"
        );
        self.text = op.apply(&self.text);
        self.done.push(op);
        self.undone.clear();
        self.bump();
    }

    /// Undo the most recent committed op. Returns `false` if there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(op) = self.done.pop() else {
            return false;
        };
        self.text = op.inverse().apply(&self.text);
        self.undone.push(op);
        self.bump();
        true
    }

    /// Redo the most recently undone op. Returns `false` if there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        let Some(op) = self.undone.pop() else {
            return false;
        };
        self.text = op.apply(&self.text);
        self.done.push(op);
        self.bump();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_inverse_round_trips() {
        let pre = "hello world";
        let op = Op::replace(6, "world", "rust");
        let post = op.apply(pre);
        assert_eq!(post, "hello rust");
        assert_eq!(op.inverse().apply(&post), pre);
    }

    #[test]
    fn commit_then_undo_restores_original() {
        let mut h = History::new("hello world");
        h.commit(Op::replace(6, "world", "rust"));
        assert_eq!(h.text(), "hello rust");
        assert!(h.undo());
        assert_eq!(h.text(), "hello world");
        assert!(!h.can_undo());
    }

    #[test]
    fn undo_then_redo_restores_edit() {
        let mut h = History::new("a");
        h.commit(Op::insert(1, "bc")); // "abc"
        h.undo();
        assert_eq!(h.text(), "a");
        assert!(h.redo());
        assert_eq!(h.text(), "abc");
        assert!(!h.can_redo());
    }

    #[test]
    fn multiple_edits_undo_in_reverse_order() {
        let mut h = History::new("");
        h.commit(Op::insert(0, "one"));
        h.commit(Op::insert(3, " two"));
        h.commit(Op::insert(7, " three"));
        assert_eq!(h.text(), "one two three");
        h.undo();
        assert_eq!(h.text(), "one two");
        h.undo();
        assert_eq!(h.text(), "one");
        h.redo();
        assert_eq!(h.text(), "one two");
    }

    #[test]
    fn commit_after_undo_clears_redo_branch() {
        let mut h = History::new("x");
        h.commit(Op::insert(1, "A")); // "xA"
        h.undo(); // "x", redo available
        assert!(h.can_redo());
        h.commit(Op::insert(1, "B")); // "xB" — new branch
        assert_eq!(h.text(), "xB");
        assert!(
            !h.can_redo(),
            "a fresh commit must discard the undone future"
        );
    }

    #[test]
    fn version_is_monotonic_across_all_state_changes() {
        let mut h = History::new("");
        assert_eq!(h.version(), VersionId(0));
        h.commit(Op::insert(0, "a"));
        h.commit(Op::insert(1, "b"));
        h.undo();
        h.redo();
        // 0 -> commit 1 -> commit 2 -> undo 3 -> redo 4
        assert_eq!(h.version(), VersionId(4));
        let lineage = h.lineage();
        assert_eq!(lineage.first(), Some(&VersionId(0)));
        assert_eq!(lineage.last(), Some(&VersionId(4)));
        assert!(
            lineage.windows(2).all(|w| w[0] < w[1]),
            "lineage must be increasing"
        );
    }

    #[test]
    fn delete_and_undo() {
        let mut h = History::new("keep DROP me");
        h.commit(Op::delete(5, "DROP ")); // "keep me"
        assert_eq!(h.text(), "keep me");
        h.undo();
        assert_eq!(h.text(), "keep DROP me");
    }
}
