use crate::el::{WidgetCtx, arena::ElArena, *};

/// Context passed to elements on build pass, runs once per element.
pub struct BuildCtx<W: WidgetCtx> {
    arena: Signal<ElArena<W>>,
    parent_id: Option<ElId>,
}

impl<W: WidgetCtx> Clone for BuildCtx<W> {
    fn clone(&self) -> Self {
        Self { arena: self.arena.clone(), parent_id: self.parent_id.clone() }
    }
}

impl<W: WidgetCtx> Copy for BuildCtx<W> {}

impl<W: WidgetCtx> BuildCtx<W> {
    pub fn root(mut root: &mut El<W>, arena: Signal<ElArena<W>>) -> Self {
        let mut this = Self { arena, parent_id: None };
        this.add_inner(&mut root);

        Self { arena, parent_id: None }
    }

    pub fn add_children(mut self, children: &mut [El<W>]) {
        for child in children {
            let child_id = self.add_inner(child);
            if let El::New(el_data) = child {
                el_data.widget.build(self.with_parent(child_id));
            }
        }
    }

    pub fn add_child(mut self, child: &mut El<W>) -> ElId {
        let child_id = self.add_inner(child);
        if let El::New(el_data) = child {
            el_data.widget.build(self.with_parent(child_id));
        }
        child_id
    }

    fn with_parent(self, parent_id: ElId) -> Self {
        Self { arena: self.arena, parent_id: Some(parent_id) }
    }

    fn add_inner(&mut self, child: &mut El<W>) -> ElId {
        self
                .arena
                .update_untracked(|arena| {

                    arena.add(self.parent_id, |id| {
                        let el = core::mem::replace(child, El::Stored(id));

                        match el {
                            El::New(el_data) => {
                                el_data
                            },
                            El::Stored(el_id) => {
                                panic!("Expected new element, got stored element with id {el_id:?}")
                            },
                        }
                    })
                })
    }
}
