use crate::el::{WidgetCtx, arena::ElArena, *};
use crate::{
    layout::{LayoutData, LayoutKind, length::LengthSize},
    render::prelude::Axis,
    widget::Widget,
};
use alloc::{boxed::Box, vec::Vec};
use log::{error, warn};
use rsact_reactive::prelude::Trigger;

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

    /// WS5.1: the builder's owned initial [`LayoutData`], moved into the
    /// arena-owned layout store by [`ElArena::add`] before build. Off the
    /// reactive graph — no `Layout` handle, no `create_inert`. For a split
    /// builder the derive returns `self.layout.data().clone()`; a delegate
    /// builder returns a child's `layout_data()`.
    fn layout_data(&self) -> LayoutData;

    /// WS5.1: set the `show` visibility memo on this builder's layout, used by
    /// `Show` to hide the wrapped child. Default no-op — a builder that owns no
    /// settable layout (delegate/identity builders) can't carry `show`.
    fn set_show(&mut self, show: Memo<bool>) {
        let _ = show;
    }

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
    /// WS5.1: the page's relayout trigger. Reactive layout-prop bindings
    /// ([`bind_layout`](Self::bind_layout)) and structure changes
    /// (`set_children`/`set_single_child`) fire it after mutating the arena so
    /// the page's layout `Memo` (which tracks it) re-runs a relayout. This is
    /// the off-graph replacement for the memo tracking a reactive `Layout`
    /// handle.
    relayout: Trigger,
}

impl<W: WidgetCtx> Clone for BuildCtx<W> {
    fn clone(&self) -> Self {
        Self {
            arena: self.arena.clone(),
            id: self.id.clone(),
            relayout: self.relayout,
        }
    }
}
impl<W: WidgetCtx> Copy for BuildCtx<W> {}

impl<W: WidgetCtx> BuildCtx<W> {
    pub fn run(
        root: &mut El<W>,
        mut arena: Signal<ElArena<W>>,
        relayout: Trigger,
    ) -> ElId {
        let root = arena.update_untracked(|arena| arena.add(None, root));

        let mut ctx = Self { id: root, arena, relayout };

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

        // WS5.1: a structure change relayouts the page (the layout `Memo`
        // tracks this trigger). No-op before the memo exists (initial build).
        self.relayout.notify();

        self
    }

    pub fn set_single_child(&mut self, child: &mut El<W>) -> &mut Self {
        let child_id = self.add_inner(child);
        self.build_el(child_id);

        self.arena.update_untracked(|arena| {
            arena.set_single_child(self.id, child_id);
        });

        // WS5.1: a structure change relayouts the page (see `set_children`).
        self.relayout.notify();

        self
    }

    /// WS5.1: wire a reactive layout-prop binding at build time (here `id` is
    /// known). The effect reads `source`, writes this element's arena-owned
    /// `LayoutData`, and marks it dirty — so a source change triggers relayout.
    /// This is the off-graph replacement for `Layout::now_reactive()` binding
    /// through a runtime node; the effect is owned by the page scope and
    /// survives the builder's consumption.
    pub fn bind_layout<U: PartialEq + 'static>(
        &self,
        source: Memo<U>,
        mut set_map: impl FnMut(&mut LayoutData, &U) + 'static,
    ) {
        let mut arena = self.arena;
        let id = self.id;
        let relayout = self.relayout;
        create_effect(move |_| {
            source.with(|value| {
                arena.update_untracked(|arena| {
                    if let Some(data) = arena.layout_mut(id) {
                        set_map(data, value);
                    }
                    arena.mark_dirty(id);
                });
                // WS5.1: a reactive prop change relayouts the page (the layout
                // `Memo` tracks this trigger). The arena reads inside the memo
                // are untracked, so this trigger is the only dependency that
                // re-runs relayout on a prop change.
                relayout.notify();
            });
        });
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
        Self { arena: self.arena, id: parent_id, relayout: self.relayout }
    }

    fn add_inner(&mut self, el: &mut El<W>) -> ElId {
        self.arena
            .update_untracked(|arena| arena.add(Some(self.id), el))
    }
}

/// WS5.1: the **off-graph, builder-side** layout. Replaces the `Layout`
/// runtime-node handle on widget builders: it holds the owned `LayoutData`
/// being assembled by the builder methods (`.width`/`.padding`/…) plus the
/// *reactive* bindings recorded for later wiring. `build` moves the data into
/// the arena (`ElArena.layouts[id]`) and drains the bindings through
/// [`BuildCtx::bind_layout`] (which has `ctx.id`). No reactive node is minted
/// for a static layout — the fake-inert `Layout::Static` is gone.
pub struct LayoutBuilder<W: WidgetCtx> {
    data: LayoutData,
    // Boxed so heterogeneous `(Memo<U>, set_map)` pairs share one Vec; drained
    // at build. Empty for a fully-static layout (no allocation beyond the Vec
    // header, which is empty).
    bindings: Vec<Box<dyn FnOnce(&mut BuildCtx<W>)>>,
}

impl<W: WidgetCtx> LayoutBuilder<W> {
    pub fn new(data: LayoutData) -> Self {
        Self { data, bindings: Vec::new() }
    }

    pub fn zero() -> Self {
        Self::new(LayoutData::zero())
    }

    pub fn shrink(kind: LayoutKind) -> Self {
        Self::new(LayoutData::shrink(kind))
    }

    pub fn fill(kind: LayoutKind) -> Self {
        Self::new(LayoutData::fill(kind))
    }

    pub fn edge(size: LengthSize) -> Self {
        Self::new(LayoutData::edge(size))
    }

    /// Base scrollable layout (main axis shrinks, cross fills). WS5.1: the
    /// content child comes from the arena, so this carries no content handle.
    pub fn scrollable(axis: Axis) -> Self {
        Self::new(LayoutData::scrollable(axis))
    }

    /// The assembled static layout data (read by `arena.add` / `Build`).
    pub fn data(&self) -> &LayoutData {
        &self.data
    }

    /// Consume into the owned `LayoutData` moved into the arena at build.
    pub fn into_data(self) -> LayoutData {
        self.data
    }

    /// Untracked mutation of the owned data (static setters, `show`, `size`).
    pub fn update_untracked(&mut self, f: impl FnOnce(&mut LayoutData)) {
        f(&mut self.data);
    }

    pub fn show(&mut self, show: Memo<bool>) {
        self.data.set_show(show);
    }

    pub fn size(mut self, size: LengthSize) -> Self {
        self.data.size = size;
        self
    }

    /// Drain the recorded reactive bindings (called by `Build::build`, which
    /// then wires each through `BuildCtx::bind_layout`).
    pub fn take_bindings(&mut self) -> Vec<Box<dyn FnOnce(&mut BuildCtx<W>)>> {
        core::mem::take(&mut self.bindings)
    }
}

// Same `SignalSetter` surface as the old `Layout` handle, so the builder-trait
// setters (`SizedWidget::width`, …) call it unchanged. The Inert arm writes the
// owned data now; the reactive arm *records* a binding wired at build (§2.2) —
// no `now_reactive`, no node.
impl<W: WidgetCtx, U: PartialEq + 'static>
    SignalSetter<LayoutData, MaybeReactive<U>> for LayoutBuilder<W>
{
    fn setter(
        &mut self,
        source: MaybeReactive<U>,
        set_map: impl FnMut(
            &mut LayoutData,
            &<MaybeReactive<U> as ReactiveValue>::Value,
        ) + 'static,
    ) {
        match source {
            MaybeReactive::Inert(inert) => {
                let mut set_map = set_map;
                inert.with(|inert| set_map(&mut self.data, inert));
            },
            MaybeReactive::Memo(memo) => {
                self.bindings.push(Box::new(move |ctx| {
                    ctx.bind_layout(memo, set_map);
                }));
            },
        }
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
            let root_id = BuildCtx::run(&mut root, arena, create_trigger());

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
