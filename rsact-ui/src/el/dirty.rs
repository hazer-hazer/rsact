//! The per-page layout **dirty set** (WS5.1).
//!
//! Replaces the whole-page `Memo<LayoutModel>`'s implicit "any reactive layout
//! signal changed ⇒ recompute the world" with an explicit set of the `ElId`s
//! whose layout inputs changed since the last relayout. A reactive layout-prop
//! binding effect (created at build time, WS5.1 §2.2) writes the arena-owned
//! `LayoutData` and then `mark`s its own `ElId` here; relayout is pulled at the
//! usual points (event hit-test / render) and runs iff the set is non-empty.
//!
//! It lives on [`super::arena::ElArena`] rather than in a standalone reactive
//! node on purpose: the binding effects already capture the arena `Signal`, so
//! marking rides the arena's legitimate shared-mutable storage — no extra
//! handle, no `Rc`, and (unlike the retired `Layout::Static` fake-inert) no
//! reactive node used as plain storage.
//!
//! **WS5.1 uses only the "is anything dirty?" bit** — relayout stays a *full*
//! recompute (the compiled default). The *contents* (`nodes`, `is_full`) are
//! the interface WS5.2 (incremental, skip-clean-subtrees) and WS5.3
//! (changed-set) consume; they are tracked now so those stages inherit them.

use crate::el::ElId;
use alloc::vec::Vec;

/// The set of elements whose layout must be recomputed, plus a `full` flag for
/// tree-wide invalidation (fonts/viewport change, page enter) that would
/// otherwise enumerate every node.
///
/// `nodes` is a deduplicated `Vec` (not a `BTreeSet` — the workspace prefers
/// sorted/flat vecs for flash, WS9a.2); dirty sets are small (the handful of
/// widgets a frame actually touches), so linear `contains` on `mark` is cheap.
/// A `full` mark clears and supersedes the node list.
#[derive(Debug, Default)]
pub struct DirtySet {
    full: bool,
    nodes: Vec<ElId>,
}

impl DirtySet {
    pub fn new() -> Self {
        Self { full: false, nodes: Vec::new() }
    }

    /// Mark a single element dirty. No-op once `full` is set (a full relayout
    /// already covers it) or if already present.
    pub fn mark(&mut self, id: ElId) {
        if !self.full && !self.nodes.contains(&id) {
            self.nodes.push(id);
        }
    }

    /// Request a whole-tree relayout (fonts/viewport change, page enter). Drops
    /// the per-node list — it is subsumed.
    pub fn mark_full(&mut self) {
        self.full = true;
        self.nodes.clear();
    }

    /// Nothing to relayout.
    pub fn is_empty(&self) -> bool {
        !self.full && self.nodes.is_empty()
    }

    /// Whether a tree-wide relayout was requested.
    pub fn is_full(&self) -> bool {
        self.full
    }

    /// The specific dirty elements (empty when `is_full`). The WS5.2/5.3
    /// consumption point; WS5.1 only checks [`is_empty`](Self::is_empty).
    pub fn nodes(&self) -> &[ElId] {
        &self.nodes
    }

    /// Reset to empty. Called by relayout once it has consumed the set.
    pub fn clear(&mut self) {
        self.full = false;
        self.nodes.clear();
    }

    /// Take the current state, leaving the set empty — the relayout drains it so
    /// marks arriving *during* a relayout are attributed to the next pass.
    pub fn take(&mut self) -> DirtySet {
        core::mem::take(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::el::ElId;
    use slotmap::KeyData;

    // Fabricate distinct ElIds without an arena (ElId is a slotmap key).
    fn el_id(n: u64) -> ElId {
        ElId::from(KeyData::from_ffi(n))
    }

    #[test]
    fn empty_by_default() {
        let d = DirtySet::new();
        assert!(d.is_empty());
        assert!(!d.is_full());
        assert!(d.nodes().is_empty());
    }

    #[test]
    fn mark_dedups_and_reports_non_empty() {
        let mut d = DirtySet::new();
        let (a, b) = (el_id(1), el_id(2));
        d.mark(a);
        d.mark(a); // dedup
        d.mark(b);
        assert!(!d.is_empty());
        assert_eq!(d.nodes(), &[a, b]);
    }

    #[test]
    fn mark_full_supersedes_and_clears_nodes() {
        let mut d = DirtySet::new();
        d.mark(el_id(1));
        d.mark_full();
        assert!(d.is_full());
        assert!(!d.is_empty());
        assert!(d.nodes().is_empty(), "full subsumes the per-node list");
        // Further single marks are no-ops while full.
        d.mark(el_id(2));
        assert!(d.nodes().is_empty());
    }

    #[test]
    fn take_leaves_empty_and_returns_prior() {
        let mut d = DirtySet::new();
        let a = el_id(1);
        d.mark(a);
        let taken = d.take();
        assert!(d.is_empty(), "take drains the set");
        assert_eq!(taken.nodes(), &[a]);
    }

    #[test]
    fn clear_resets_full_and_nodes() {
        let mut d = DirtySet::new();
        d.mark_full();
        d.clear();
        assert!(d.is_empty());
        assert!(!d.is_full());
    }
}
