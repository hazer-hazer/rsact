use crate::{
    el::{El, ElData, ElId, WidgetCtx},
    widget::Widget,
};
use alloc::boxed::Box;
use alloc::vec::Vec;
use rsact_reactive::prelude::*;

pub struct ArenaEl<W: WidgetCtx> {
    data: ElData<W>,
    parent: Option<ElId>,
    // TODO: Set? Will allow intersections which is good for reconciliation
    children: Option<Vec<ElId>>,
}

impl<W: WidgetCtx> ArenaEl<W> {
    pub fn new(parent: Option<ElId>, data: ElData<W>) -> Self {
        Self { data, parent, children: None }
    }
}

pub struct ElArena<W: WidgetCtx> {
    els: slotmap::SlotMap<ElId, ArenaEl<W>>,
    // TODO: Do we really need parent relation?
    parents: slotmap::SecondaryMap<ElId, ElId>,
}

impl<W: WidgetCtx> ElArena<W> {
    pub fn new() -> Self {
        Self {
            els: slotmap::SlotMap::with_key(),
            parents: slotmap::SecondaryMap::new(),
        }
    }

    pub fn add(
        &mut self,
        parent: Option<ElId>,
        data: impl FnOnce(ElId) -> ElData<W>,
    ) -> ElId {
        let id = self.els.insert_with_key(|id| ArenaEl::new(parent, data(id)));

        if let Some(parent) = parent {
            self.parents.insert(id, parent);
            self.els
                .get_mut(parent)
                .unwrap()
                .children
                .get_or_insert(Vec::new())
                .push(id);
        }

        id
    }

    pub fn get(&self, id: ElId) -> Option<&dyn Widget<W>> {
        self.els.get(id).map(|el| el.data.widget.as_ref())
    }

    pub fn get_mut(
        &mut self,
        id: ElId,
    ) -> Option<&mut (dyn Widget<W> + 'static)> {
        self.els.get_mut(id).map(|el| el.data.widget.as_mut())
    }

    pub fn remove(&mut self, id: ElId) -> Option<Box<dyn Widget<W>>> {
        if let Some(parent) = self.parents.remove(id) {
            if let Some(children) =
                self.els.get_mut(parent).unwrap().children.as_mut()
            {
                children.retain(|&child_id| child_id != id);
            }
        }
        self.els.remove(id).map(|el| el.data.widget)
    }

    pub fn expect_stored(&self, el: &El<W>) -> (ElId, &dyn Widget<W>) {
        match el {
            El::New(_) => panic!("Expected stored element, got new element"),
            El::Stored(id) => {
                (*id, self.get(*id).expect("Element not found in arena"))
            },
        }
    }

    pub fn expect_stored_mut(
        &mut self,
        el: &El<W>,
    ) -> (ElId, &mut dyn Widget<W>) {
        match el {
            El::New(_) => panic!("Expected stored element, got new element"),
            El::Stored(id) => {
                (*id, self.get_mut(*id).expect("Element not found in arena"))
            },
        }
    }
}
