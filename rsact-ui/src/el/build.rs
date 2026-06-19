use crate::el::{WidgetCtx, arena::ElArena, *};
use log::{error, warn};

/// Context passed to elements on build pass, runs once per element.
/// `id` is the id of the element being build. But as elements don't call to build themselves but their children it is by-design made so there's no case when parent id is None. This is done by preallocating the root element.
/// WARN: It is strictly advised to run build pass only inside reactive batch section to avoid running effects dependent on widget inside arena as last could be taken for build. Though [`Widget::build`] implementations should not trigger any effects but only create them (effect first run is not dangerous because we sure that children elements are not taken from the arena, because they don't even exist before run).
pub struct BuildCtx<W: WidgetCtx> {
    arena: Signal<ElArena<W>>,
    id: ElId,
}

impl<W: WidgetCtx> Clone for BuildCtx<W> {
    fn clone(&self) -> Self {
        Self { arena: self.arena.clone(), id: self.id.clone() }
    }
}
impl<W: WidgetCtx> Copy for BuildCtx<W> {}

impl<W: WidgetCtx> BuildCtx<W> {
    pub fn run(root: &mut El<W>, mut arena: Signal<ElArena<W>>) -> ElId {
        let root = arena.update_untracked(|arena| arena.add(None, root));

        let mut ctx = Self { id: root, arena };

        ctx.build_el(root);

        root
    }

    // TODO: Is it possible to reconcile children preserving unchanged?
    // This requires children memos to not return new children, reusing old, but it seems to require user to do this.
    // Or we can compare previous widget with new, but comparison can be very expensive, so skip this variant.
    // We better make something like a SignalVec datatype that will support diffing and preserving old values. So we would compare: remove(El::Stored) -> remove, remove(El::New) -> do nothing, add (El::Stored) -> keep, add(El::New) -> build.
    // Or maybe ChildrenQueue command queue like "PushChild", "SetChild", "RemoveChild".
    pub fn set_children(&mut self, children: &mut [El<W>]) -> &mut Self {
        let children_ids = children
            .iter_mut()
            .map(|child| self.add_inner(child))
            .collect::<Vec<_>>();

        children_ids.iter().for_each(|child_id| {
            self.build_el(*child_id);
        });

        self.arena.update_untracked(|arena| {
            arena.set_children(self.id, children_ids);
        });

        self
    }

    pub fn set_single_child(&mut self, child: &mut El<W>) -> &mut Self {
        let child_id = self.add_inner(child);
        self.build_el(child_id);

        self.arena.update_untracked(|arena| {
            arena.set_single_child(self.id, child_id);
        });

        self
    }

    fn build_el(&mut self, id: ElId) {
        let Some(mut el) =
            self.arena.update_untracked(|arena| arena.take_el(id))
        else {
            warn!(
                "Trying to build non-existent or taken element with id {:?}",
                id
            );
            return;
        };

        if el.state.built {
            error!("Attempt to rebuild element {id:?}");
        } else {
            el.widget.build(self.for_el(id));
            el.state.built = true;
        }

        self.arena.update_untracked(|arena| arena.restore_el(id, el));
    }

    // pub fn add_children(mut self, children: &mut [El<W>]) {
    //     let children_ids = children
    //         .iter_mut()
    //         .map(|child| self.add_inner(child))
    //         .collect::<Vec<_>>();

    //     children_ids.iter().for_each(|child_id| {
    //         self.arena.update(move |arena| {
    //             let child = arena.get_mut(*child_id).unwrap();
    //             child.build(self.with_parent(*child_id));
    //         });
    //     });
    // }

    // pub fn add_child(mut self, child: &mut El<W>) -> ElId {
    //     let child_id = self.add_inner(child);
    //     if let El::New(el_data) = child {
    //         el_data.widget.build(self.with_parent(child_id));
    //     }
    //     child_id
    // }

    fn for_el(self, parent_id: ElId) -> Self {
        Self { arena: self.arena, id: parent_id }
    }

    fn add_inner(&mut self, el: &mut El<W>) -> ElId {
        self.arena.update_untracked(|arena| arena.add(Some(self.id), el))
    }
}

#[cfg(test)]
mod tests {
    use crate::el::arena::ElArena;
    use rsact_reactive::prelude::*;

    // TODO
}
