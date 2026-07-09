use crate::el::{WidgetCtx, arena::ElArena, *};
use crate::{layout::node::Layout, widget::Widget};
use log::{error, warn};

/// A transient **builder**: holds construction-only state and is consumed at
/// build into its retained [`Widget`]. `build` is the type transform
/// (WS13 spec §2.1). `self: Box<Self>` keeps it object-safe for `dyn Build`.
///
/// Unconverted widgets get an *identity* `Build` (build in place, return self)
/// emitted per-type by `#[derive(View)]` — never a blanket (coherence, see
/// `el/view.rs`).
pub trait Build<W: WidgetCtx>: core::any::Any {
    fn build(
        self: alloc::boxed::Box<Self>,
        ctx: BuildCtx<W>,
    ) -> alloc::boxed::Box<dyn Widget<W>>;

    /// Layout snapshot the parent stores in its `El::Stored { id, layout }`
    /// husk (read by `arena.add` before build).
    fn layout(&self) -> Layout;

    fn flags(&self) -> WidgetFlags {
        WidgetFlags::default()
    }

    fn debug_name(&self) -> &'static str {
        core::any::type_name::<Self>()
    }
}

/// Context passed to elements on build pass, runs once per element.
/// `id` is the id of the element being build. But as elements don't call to
/// build themselves but their children it is by-design made so there's no case
/// when parent id is None. This is done by preallocating the root element.
/// WARN: It is strictly advised to run build pass only inside reactive batch
/// section to avoid running effects dependent on widget inside arena as last
/// could be taken for build. Though [`Widget::build`] implementations should
/// not trigger any effects but only create them (effect first run is not
/// dangerous because we sure that children elements are not taken from the
/// arena, because they don't even exist before run).
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
    // This requires children memos to not return new children, reusing old, but
    // it seems to require user to do this. Or we can compare previous
    // widget with new, but comparison can be very expensive, so skip this
    // variant. We better make something like a SignalVec datatype that will
    // support diffing and preserving old values. So we would compare:
    // remove(El::Stored) -> remove, remove(El::New) -> do nothing, add
    // (El::Stored) -> keep, add(El::New) -> build. Or maybe ChildrenQueue
    // command queue like "PushChild", "SetChild", "RemoveChild".
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
        let Some(data) = self.arena.update_untracked(|arena| arena.take_el(id))
        else {
            warn!(
                "Trying to build non-existent or taken element with id {:?}",
                id
            );
            return;
        };

        let ElData { stage, mut state } = data;
        let stage = if state.built {
            error!("Attempt to rebuild element {id:?}");
            stage
        } else {
            state.built = true;
            match stage {
                ElStage::Unbuilt(builder) => {
                    ElStage::Built(builder.build(self.for_el(id)))
                },
                built @ ElStage::Built(_) => {
                    error!("Element {id:?} was already built");
                    built
                },
            }
        };

        self.arena.update_untracked(|arena| {
            arena.restore_el(id, ElData { stage, state })
        });
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
        self.arena
            .update_untracked(|arena| arena.add(Some(self.id), el))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        el::{El, arena::ElArena, build::BuildCtx},
        test_support::NullWtf,
        widget::{Widget, combinators::Unit},
    };
    use rsact_reactive::prelude::*;

    /// The build pass transforms an `Unbuilt(Box<dyn Build>)` arena node into a
    /// `Built(Box<dyn Widget>)` node exactly once, and the built widget's layout is
    /// reachable through the new `ElData::widget()` accessor.
    #[test]
    fn build_transforms_unbuilt_to_built() {
        with_new_runtime(|_| {
            let mut root: El<NullWtf> = Unit.el();
            let arena = create_signal(ElArena::new());
            let root_id = BuildCtx::run(&mut root, arena);

            arena.with(|arena| {
                let data = arena.expect(root_id).expect("root must exist");
                assert!(data.state.built, "root must be marked built");
                // Reachable only through the two-stage accessor (compile-driver):
                assert!(
                    data.widget().is_some(),
                    "a built node must expose its retained widget"
                );
            });
        });
    }
}
