use crate::{
    el::{El, ElData, ElId, WidgetCtx, dirty::DirtySet},
    layout::LayoutData,
};
use alloc::vec::Vec;
use log::error;

pub struct ElNode<W: WidgetCtx> {
    // TODO: Can eliminate Option by using UnsafeCell, but then we need to
    // prove parent is never access while child is used. Take/restore logic is
    // only used in build phase, and as it is kept to be strictly top-down, we
    // can guarantee soundness.
    pub(crate) data: Option<ElData<W>>,
}

impl<W: WidgetCtx> ElNode<W> {
    pub fn new(data: ElData<W>) -> Self {
        Self { data: Some(data) }
    }
}

pub struct ArenaEls<W: WidgetCtx> {
    els: slotmap::SlotMap<ElId, ElNode<W>>,
}

impl<W: WidgetCtx> ArenaEls<W> {
    pub fn new() -> Self {
        Self { els: slotmap::SlotMap::with_key() }
    }

    pub fn get_mut(&mut self, id: ElId) -> Option<&mut ElNode<W>> {
        self.els.get_mut(id)
    }

    /// Whether `id` still names a node in the arena. Unlike [`expect`], this is
    /// true even while the node's `data` is temporarily taken (build/event
    /// take/restore) — it is a pure identity check (WS3.3).
    pub fn contains(&self, id: ElId) -> bool {
        self.els.contains_key(id)
    }

    pub fn expect(&self, id: ElId) -> Option<&ElData<W>> {
        self.els
            .get(id)
            .and_then(|el| el.data.as_ref())
            .or_else(|| {
                error!("Element must exist at this place");
                None
            })
    }

    pub fn expect_unreachable(&self, id: ElId) -> &ElData<W> {
        self.els
            .get(id)
            .and_then(|el| el.data.as_ref())
            .expect("Element must exist at this place")
    }

    pub fn expect_mut(&mut self, id: ElId) -> Option<&mut ElData<W>> {
        self.get_mut(id)
            .and_then(|el| el.data.as_mut())
            .or_else(|| {
                error!("Element must exist at this place");
                None
            })
    }

    /// Number of live element nodes. Used to assert rebuilds don't leak.
    pub(crate) fn len(&self) -> usize {
        self.els.len()
    }
}

type ArenaChildrenVec = tinyvec::TinyVec<[ElId; 1]>;

pub struct ArenaChildren {
    children: slotmap::SecondaryMap<ElId, ArenaChildrenVec>,
}

impl ArenaChildren {
    pub fn new() -> Self {
        Self { children: slotmap::SecondaryMap::new() }
    }

    pub fn set(
        &mut self,
        parent: ElId,
        children: Vec<ElId>,
    ) -> Option<ArenaChildrenVec> {
        self.children
            .insert(parent, tinyvec::TinyVec::Heap(children))
    }

    pub fn set_single(
        &mut self,
        parent: ElId,
        child: ElId,
    ) -> Option<ArenaChildrenVec> {
        self.children
            .insert(parent, tinyvec::TinyVec::from_array_len([child; 1], 1))
    }

    pub fn get(&self, parent: ElId) -> Option<&[ElId]> {
        self.children.get(parent).map(|v| v.as_slice())
    }

    pub fn remove(&mut self, parent: ElId) -> Option<ArenaChildrenVec> {
        self.children.remove(parent)
    }
}

pub struct ElArena<W: WidgetCtx> {
    pub(crate) els: ArenaEls<W>,
    pub(crate) children: ArenaChildren,
    // TODO: Do we really need parent relation separately?
    pub(crate) parents: slotmap::SecondaryMap<ElId, ElId>,
    /// WS5.1: the arena OWNS each element's `LayoutData`, keyed by `ElId` — the
    /// off-graph layout store that replaces the `Layout` runtime-node handle.
    /// (Commit A0: populated at `add`; the kernel read-path + reactive-prop
    /// writers flip onto it in A1.)
    pub(crate) layouts: slotmap::SecondaryMap<ElId, LayoutData>,
    /// WS5.1 layout dirty set — the `ElId`s whose layout inputs changed since
    /// the last relayout. Reactive layout-prop binding effects mark here (they
    /// capture the arena `Signal`); relayout is pulled iff non-empty. Off the
    /// reactive graph on purpose (no fake-inert node).
    pub(crate) dirty: DirtySet,
}

impl<W: WidgetCtx> ElArena<W> {
    pub fn new() -> Self {
        Self {
            els: ArenaEls::new(),
            children: ArenaChildren::new(),
            parents: slotmap::SecondaryMap::new(),
            layouts: slotmap::SecondaryMap::new(),
            dirty: DirtySet::new(),
        }
    }

    /// The arena-owned `LayoutData` for `id` (WS5.1). `None` if absent (caller
    /// degrades, never panics).
    pub fn layout(&self, id: ElId) -> Option<&LayoutData> {
        self.layouts.get(id)
    }

    /// Mutable access to `id`'s owned `LayoutData` (reactive-prop binding
    /// effects write through this at build time).
    pub fn layout_mut(&mut self, id: ElId) -> Option<&mut LayoutData> {
        self.layouts.get_mut(id)
    }

    /// Mark `id`'s layout dirty (WS5.1). Called from reactive layout-prop
    /// binding effects after they write `layout_mut(id)`.
    pub fn mark_dirty(&mut self, id: ElId) {
        self.dirty.mark(id);
    }

    /// Request a whole-tree relayout (fonts/viewport change, page enter).
    pub fn mark_full_relayout(&mut self) {
        self.dirty.mark_full();
    }

    /// Whether any layout is dirty — the WS5.1 relayout gate.
    pub fn is_layout_dirty(&self) -> bool {
        !self.dirty.is_empty()
    }

    /// Drain the dirty set (relayout consumes it; marks arriving mid-pass are
    /// attributed to the next relayout).
    pub fn take_dirty(&mut self) -> DirtySet {
        self.dirty.take()
    }

    // pub fn traverse_result<E>(
    //     &mut self,
    //     id: ElId,
    //     mut f: impl FnMut(ElId, &mut ElNode<W>) -> Result<(), E>,
    // ) -> Result<(), E> {
    //     self.traverse_cf(id, |id, node| match f(id, node) {
    //         Ok(()) => ControlFlow::Continue(()),
    //         Err(e) => ControlFlow::Break(e),
    //     })
    //     .continue_ok()
    // }

    // pub fn traverse_cf<B>(
    //     &mut self,
    //     id: ElId,
    //     f: impl FnMut(ElId, &mut ElNode<W>) -> ControlFlow<B, ()>,
    // ) -> ControlFlow<B, ()> {
    //     Self::traverse_(id, &mut self.els, &self.children, f)
    // }

    // fn traverse_<B>(
    //     id: ElId,
    //     arena: &mut slotmap::SlotMap<ElId, ElNode<W>>,
    //     children: &slotmap::SecondaryMap<ElId, Vec<ElId>>,
    //     mut f: impl FnMut(ElId, &mut ElNode<W>) -> ControlFlow<B, ()>,
    // ) -> ControlFlow<B, ()> {
    //     if let Some(children_ids) = children.get(id) {
    //         for child in children_ids {
    //             Self::traverse_(*child, arena, children, &mut f)?;
    //         }
    //     }

    //     if let Some(el) = arena.get_mut(id) {
    //         f(id, el)
    //     } else {
    //         warn!("Trying to traverse non-existent element with id {:?}",
    // id);         return ControlFlow::Continue(());
    //     }
    // }

    pub fn take_el(&mut self, id: ElId) -> Option<ElData<W>> {
        self.els.els.get_mut(id).and_then(|el| el.data.take())
    }

    pub fn restore_el(&mut self, id: ElId, data: ElData<W>) {
        if let Some(el) = self.els.els.get_mut(id) {
            el.data = Some(data);
        }
    }

    /// Recursively remove `root` and its entire subtree from `els`, `children`
    /// and `parents`. Iterative (heap worklist) so a deep tree can't overflow
    /// the stack. Without this, replacing a subtree (every reactive rebuild —
    /// `dynamic(...)`, tab switch, list update) would leak all descendants in
    /// `els` and leave stale `children`/`parents` edges (unbounded growth).
    fn remove_subtree(&mut self, root: ElId) {
        let mut stack = alloc::vec![root];
        while let Some(id) = stack.pop() {
            if let Some(children) = self.children.remove(id) {
                stack.extend(children.iter().copied());
            }
            self.parents.remove(id);
            // Dispose the element's render probes as it leaves the tree, so
            // they do not outlive it (WS2.3) — otherwise every reactive rebuild
            // (`dynamic(...)`, tab switch, list update) leaks probe nodes.
            if let Some(mut node) = self.els.els.remove(id) {
                if let Some(mut data) = node.data.take() {
                    data.state.dispose_probes();
                }
            }
        }
    }

    /// Dispose every element's render probes (WS2.3). Used on page drop, before
    /// the arena signal itself is disposed, so no probe node outlives the page.
    pub(crate) fn dispose_all_probes(&mut self) {
        for node in self.els.els.values_mut() {
            if let Some(data) = node.data.as_mut() {
                data.state.dispose_probes();
            }
        }
    }

    pub fn set_children(&mut self, id: ElId, children: Vec<ElId>) {
        if self.els.els.contains_key(id) {
            let old_children = self.children.set(id, children);

            if let Some(old_children) = old_children {
                old_children.iter().for_each(|child_id| {
                    self.remove_subtree(*child_id);
                });
            }
        } else {
            error!(
                "Trying to set children of non-existent element with id {:?}",
                id
            );
        }
    }

    pub fn set_single_child(&mut self, id: ElId, child: ElId) {
        if self.els.els.contains_key(id) {
            let old_children = self.children.set_single(id, child);

            // Soundness: It is unsound to have two or more nodes having same
            // children, but it is expensive to check this.
            if let Some(old_children) = old_children {
                old_children.iter().for_each(|child_id| {
                    self.remove_subtree(*child_id);
                });
            }
        } else {
            error!(
                "Trying to set child of non-existent element with id {:?}",
                id
            );
        }
    }

    pub fn add(&mut self, parent: Option<ElId>, el: &mut El<W>) -> ElId {
        // WS5.1 (A1): move the builder's OWNED layout data into the arena-owned
        // store, keyed by the ElId minted below. No `Layout` handle, no
        // `with_untracked` — the arena owns `LayoutData` directly. Reactive
        // layout-prop bindings mutate it later via `BuildCtx::bind_layout`.
        let layout_data = el.layout_data();

        let id = self.els.els.insert_with_key(|id| {
            let el = core::mem::replace(el, El::Stored {id});

            match el {
                El::New(el_data) => {
                    ElNode::new(el_data)
                },
                El::Stored {id, ..} => {
                    panic!("Expected new element, got stored element with id {id:?}")
                },
            }
        });

        self.layouts.insert(id, layout_data);

        if let Some(parent) = parent {
            self.parents.insert(id, parent);
        }

        id
    }

    pub fn expect(&self, id: ElId) -> Option<&ElData<W>> {
        self.els.expect(id)
    }

    /// Whether `id` still names an element in this arena (WS3.3). Pure identity
    /// check — see [`ArenaEls::contains`].
    pub fn contains(&self, id: ElId) -> bool {
        self.els.contains(id)
    }

    pub fn expect_mut(&mut self, id: ElId) -> Option<&mut ElData<W>> {
        self.els.expect_mut(id)
    }

    pub fn expect_unreachable(&self, id: ElId) -> &ElData<W> {
        self.els.expect_unreachable(id)
    }

    pub fn children(&self, id: ElId) -> Option<&[ElId]> {
        self.children.get(id)
    }

    /// Number of live element nodes in the arena (test/diagnostics helper).
    pub(crate) fn el_count(&self) -> usize {
        self.els.len()
    }

    // pub fn set_children(&mut self, id: ElId, children: Vec<ElId>) {
    //     self.els.get_mut(id).unwrap().children = Some(children);
    // }

    // pub fn get(&self, id: ElId) -> Option<&dyn Widget<W>> {
    //     self.els.get(id).map(|el| el.data.widget.as_ref())
    // }

    // pub fn get_mut(
    //     &mut self,
    //     id: ElId,
    // ) -> Option<&mut (dyn Widget<W> + 'static)> {
    //     self.els.get_mut(id).map(|el| el.data.widget.as_mut())
    // }

    // pub fn remove(&mut self, id: ElId) -> Option<Box<dyn Widget<W>>> {
    //     if let Some(parent) = self.parents.remove(id) {
    //         if let Some(children) =
    //             self.els.get_mut(parent).unwrap().children.as_mut()
    //         {
    //             children.retain(|&child_id| child_id != id);
    //         }
    //     }
    //     self.els.remove(id).map(|el| el.data.widget)
    // }

    // pub fn expect_stored(&self, el: &El<W>) -> (ElId, &dyn Widget<W>) {
    //     match el {
    //         El::New(_) => panic!("Expected stored element, got new element"),
    //         El::Stored(id) => {
    //             (*id, self.get(*id).expect("Element not found in tree"))
    //         },
    //     }
    // }

    // pub fn expect_stored_mut(
    //     &mut self,
    //     el: &El<W>,
    // ) -> (ElId, &mut dyn Widget<W>) {
    //     match el {
    //         El::New(_) => panic!("Expected stored element, got new element"),
    //         El::Stored(id) => {
    //             (*id, self.get_mut(*id).expect("Element not found in tree"))
    //         },
    //     }
    // }
}
