//! The `LayoutTree` accessor (WS5.1 — Option 2, one-tree layout).
//!
//! WS5.1 walks the **arena** to lay out, instead of the parallel
//! `FlexLayout.children` / `ContainerLayout.content` layout-handle sub-tree.
//! The layout kernel (`model_layout`/`model_flex`) reads the tree through this
//! trait so `layout/` stays decoupled from `el/arena` internals (spec §7-E):
//! it asks only for a node's live layout handle, its child `ElId`s, and whether
//! a node is transparent.
//!
//! **Transparent nodes** (`Dynamic`, `WidgetFlags::transparent_layout`) own no
//! layout level: their single child inherits their slot — exactly how the
//! render/event passes treat them (`el/render.rs:536`, `el/event.rs:74`).
//! [`effective_children`] flattens them out so the produced `LayoutModel` has
//! one child per *effective* child, keeping the arena↔model child counts equal
//! (the positional zip the passes still perform stays valid).

use crate::{
    el::{ElId, WidgetCtx, arena::ElArena},
    layout::LayoutData,
};
use alloc::vec::Vec;

/// Read access to the element tree for the layout pass. Implemented by
/// `ElArena`; a mock implements it in tests.
pub trait LayoutTree {
    /// The node's arena-owned `LayoutData` (WS5.1: off the graph, keyed by
    /// `ElId`), or `None` if the node is missing (caller degrades, never
    /// panics).
    fn layout(&self, id: ElId) -> Option<&LayoutData>;

    /// The node's arena children in order (empty if none).
    fn children(&self, id: ElId) -> &[ElId];

    /// Whether the node is a transparent-layout node (owns no layout level).
    fn is_transparent(&self, id: ElId) -> bool;
}

impl<W: WidgetCtx> LayoutTree for ElArena<W> {
    fn layout(&self, id: ElId) -> Option<&LayoutData> {
        // The arena OWNS the LayoutData (WS5.1). Inherent `ElArena::layout`
        // wins over this trait method — no recursion.
        ElArena::layout(self, id)
    }

    fn children(&self, id: ElId) -> &[ElId] {
        // Inherent `ElArena::children` (returns `Option<&[ElId]>`) wins over the
        // trait method here — no recursion.
        ElArena::children(self, id).unwrap_or(&[])
    }

    fn is_transparent(&self, id: ElId) -> bool {
        self.expect(id)
            .map(|d| d.state.flags.is_transparent_layout())
            .unwrap_or(false)
    }
}

/// The **effective** layout children of `id`: its arena children with every
/// transparent child replaced by that child's own (single) effective child,
/// recursively. The produced list has one entry per real layout node, so the
/// `LayoutModel` it seeds skips transparent nodes just like the render/event
/// passes, and arena↔model child counts stay equal for the positional zip.
///
/// A transparent node that does not have exactly one child is a bug (it must
/// wrap exactly one); it is logged and contributes nothing (degrade, matching
/// the render pass), rather than panicking.
pub fn effective_children<T: LayoutTree + ?Sized>(
    tree: &T,
    id: ElId,
) -> Vec<ElId> {
    let mut out = Vec::new();
    for_each_effective_child(tree, id, |c| out.push(c));
    out
}

/// [`effective_children`] without the allocation — visits each effective child
/// in order. The `min_size` fold uses this so a bottom-up size pass does not
/// allocate a `Vec` per node.
pub fn for_each_effective_child<T: LayoutTree + ?Sized>(
    tree: &T,
    id: ElId,
    mut f: impl FnMut(ElId),
) {
    for &child in tree.children(id) {
        push_effective(tree, child, &mut f);
    }
}

/// The single effective child of a one-child node (`Container`/`Scrollable`
/// content). `None` (logged by the caller's degrade) if the node does not have
/// exactly one effective child.
pub fn effective_single_child<T: LayoutTree + ?Sized>(
    tree: &T,
    id: ElId,
) -> Option<ElId> {
    let mut found = None;
    let mut count = 0usize;
    for_each_effective_child(tree, id, |c| {
        if count == 0 {
            found = Some(c);
        }
        count += 1;
    });
    if count == 1 { found } else { None }
}

fn push_effective<T: LayoutTree + ?Sized>(
    tree: &T,
    id: ElId,
    f: &mut impl FnMut(ElId),
) {
    if tree.is_transparent(id) {
        let children = tree.children(id);
        if children.len() == 1 {
            push_effective(tree, children[0], f);
        } else {
            log::error!(
                "Transparent-layout node {id:?} must wrap exactly one child \
                 (has {}) — skipping it in layout (WS5.1)",
                children.len()
            );
        }
    } else {
        f(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::el::ElId;
    use alloc::{vec, vec::Vec};
    use slotmap::{KeyData, SecondaryMap};

    fn el_id(n: u64) -> ElId {
        ElId::from(KeyData::from_ffi(n))
    }

    /// A minimal `LayoutTree` for exercising [`effective_children`] without an
    /// arena: children edges + a transparent-node set. `layout` is unused here.
    struct MockTree {
        children: SecondaryMap<ElId, Vec<ElId>>,
        transparent: Vec<ElId>,
    }

    impl MockTree {
        fn new() -> Self {
            Self { children: SecondaryMap::new(), transparent: Vec::new() }
        }
        fn set_children(&mut self, id: ElId, kids: Vec<ElId>) {
            self.children.insert(id, kids);
        }
        fn mark_transparent(&mut self, id: ElId) {
            self.transparent.push(id);
        }
    }

    impl LayoutTree for MockTree {
        fn layout(&self, _id: ElId) -> Option<&crate::layout::LayoutData> {
            None
        }
        fn children(&self, id: ElId) -> &[ElId] {
            self.children.get(id).map(|v| v.as_slice()).unwrap_or(&[])
        }
        fn is_transparent(&self, id: ElId) -> bool {
            self.transparent.contains(&id)
        }
    }

    #[test]
    fn plain_children_pass_through() {
        let mut t = MockTree::new();
        let (p, a, b) = (el_id(1), el_id(2), el_id(3));
        t.set_children(p, vec![a, b]);
        assert_eq!(effective_children(&t, p), vec![a, b]);
    }

    #[test]
    fn transparent_child_is_replaced_by_its_single_child() {
        // p -> [dyn]; dyn(transparent) -> [inner]
        let mut t = MockTree::new();
        let (p, dynamic, inner) = (el_id(1), el_id(2), el_id(3));
        t.set_children(p, vec![dynamic]);
        t.set_children(dynamic, vec![inner]);
        t.mark_transparent(dynamic);
        // p's effective child is `inner`, not `dynamic` — one node per slot.
        assert_eq!(effective_children(&t, p), vec![inner]);
    }

    #[test]
    fn nested_transparent_descends_to_first_real_node() {
        // p -> [d1]; d1(t) -> [d2]; d2(t) -> [real]
        let mut t = MockTree::new();
        let (p, d1, d2, real) = (el_id(1), el_id(2), el_id(3), el_id(4));
        t.set_children(p, vec![d1]);
        t.set_children(d1, vec![d2]);
        t.set_children(d2, vec![real]);
        t.mark_transparent(d1);
        t.mark_transparent(d2);
        assert_eq!(effective_children(&t, p), vec![real]);
    }

    #[test]
    fn transparent_among_plain_preserves_order_and_count() {
        // p -> [a, dyn, b]; dyn(t) -> [inner]  => [a, inner, b]
        let mut t = MockTree::new();
        let (p, a, dynamic, inner, b) =
            (el_id(1), el_id(2), el_id(3), el_id(4), el_id(5));
        t.set_children(p, vec![a, dynamic, b]);
        t.set_children(dynamic, vec![inner]);
        t.mark_transparent(dynamic);
        let eff = effective_children(&t, p);
        assert_eq!(eff, vec![a, inner, b]);
        // One effective child per arena child — the zip-count invariant.
        assert_eq!(eff.len(), tree_child_count(&t, p));
    }

    fn tree_child_count(t: &MockTree, id: ElId) -> usize {
        t.children(id).len()
    }

    #[test]
    fn malformed_transparent_degrades_to_nothing() {
        // A transparent node with two children is a bug: contributes nothing.
        let mut t = MockTree::new();
        let (p, bad, x, y) = (el_id(1), el_id(2), el_id(3), el_id(4));
        t.set_children(p, vec![bad]);
        t.set_children(bad, vec![x, y]);
        t.mark_transparent(bad);
        assert!(effective_children(&t, p).is_empty());
    }

    #[test]
    fn leaf_has_no_children() {
        let t = MockTree::new();
        assert!(effective_children(&t, el_id(1)).is_empty());
    }
}
