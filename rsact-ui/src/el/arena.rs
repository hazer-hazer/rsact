use crate::{
    el::{El, ElData, ElId, WidgetCtx},
    widget::Widget,
};
use alloc::vec::Vec;
use log::{error, warn};

pub struct ElNode<W: WidgetCtx> {
    parent: Option<ElId>,
    // TODO: Can eliminate Option by using UnsafeCell, but then we need to prove parent is never access while child is used. Take/restore logic is only used in build phase, and as it is kept to be strictly top-down, we can guarantee soundness.
    pub(crate) data: Option<ElData<W>>,
    // TODO: Set? Will allow intersections which is good for reconciliation
    // children: Option<Vec<ElId>>,
}

impl<W: WidgetCtx> ElNode<W> {
    pub fn new(parent: Option<ElId>, data: ElData<W>) -> Self {
        Self { data: Some(data), parent }
    }
}

pub type ArenaEls<W> = slotmap::SlotMap<ElId, ElNode<W>>;

pub type ArenaChildrenVec = tinyvec::TinyVec<[ElId; 1]>;

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
        self.children.insert(parent, tinyvec::TinyVec::Heap(children))
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
}

pub struct ElArena<W: WidgetCtx> {
    pub(crate) els: ArenaEls<W>,
    pub(crate) children: ArenaChildren,
    // TODO: Do we really need parent relation?
    pub(crate) parents: slotmap::SecondaryMap<ElId, ElId>,
}

impl<W: WidgetCtx> ElArena<W> {
    pub fn new() -> Self {
        Self {
            els: slotmap::SlotMap::with_key(),
            children: ArenaChildren::new(),
            parents: slotmap::SecondaryMap::new(),
        }
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
    //         warn!("Trying to traverse non-existent element with id {:?}", id);
    //         return ControlFlow::Continue(());
    //     }
    // }

    pub fn take_el(&mut self, id: ElId) -> Option<ElData<W>> {
        self.els.get_mut(id).and_then(|el| el.data.take())
    }

    pub fn restore_el(&mut self, id: ElId, data: ElData<W>) {
        if let Some(el) = self.els.get_mut(id) {
            el.data = Some(data);
        }
    }

    pub fn set_children(&mut self, id: ElId, children: Vec<ElId>) {
        if self.els.contains_key(id) {
            let old_children = self.children.set(id, children);

            if let Some(old_children) = old_children {
                old_children.iter().for_each(|child_id| {
                    self.els.remove(*child_id);
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
        if self.els.contains_key(id) {
            let old_children = self.children.set_single(id, child);

            // Soundness: It is unsound to have two or more nodes having same children, but it is expensive to check this.
            if let Some(old_children) = old_children {
                old_children.iter().for_each(|child_id| {
                    self.els.remove(*child_id);
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
        let id = self.els.insert_with_key(|id| {
            let layout = el.layout();
            let el = core::mem::replace(el, El::Stored {id, layout});

            match el {
                El::New(el_data) => {
                    ElNode::new(parent, el_data)
                },
                El::Stored {id, ..} => {
                    panic!("Expected new element, got stored element with id {id:?}")
                },
            }
        });

        if let Some(parent) = parent {
            self.parents.insert(id, parent);
        }

        id
    }

    pub fn get_widget(&self, id: ElId) -> Option<&dyn Widget<W>> {
        self.els
            .get(id)
            .and_then(|el| el.data.as_ref().map(|data| data.widget.as_ref()))
    }

    pub fn get_mut(&mut self, id: ElId) -> Option<&mut ElNode<W>> {
        self.els.get_mut(id)
    }

    pub fn expect(&self, id: ElId) -> Option<&ElData<W>> {
        self.els.get(id).and_then(|el| el.data.as_ref()).or_else(|| {
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
        self.get_mut(id).and_then(|el| el.data.as_mut()).or_else(|| {
            error!("Element must exist at this place");
            None
        })
    }

    pub fn children(&self, id: ElId) -> Option<&[ElId]> {
        self.children.get(id).map(|children| children)
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
