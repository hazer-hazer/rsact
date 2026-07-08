use crate::widget::prelude::*;
use alloc::boxed::Box;
use core::fmt::Debug;

pub mod arena;
pub mod build;
pub mod ctx;
pub mod event;
pub mod flags;
pub mod render;
pub mod state;
pub mod update;
pub mod view;

pub use build::*;
pub use ctx::*;
pub use event::*;
pub use flags::WidgetFlags;
pub use render::*;
pub use state::*;
pub use update::*;
pub use view::*;

// The `View` derive macro lives in `rsact-macros`; re-export it alongside the
// `View` trait (different namespaces, so the names coexist) so widget modules
// get `#[derive(View)]` through the same prelude glob.
pub use rsact_macros::View;

slotmap::new_key_type! {
    pub struct ElId;
}

/// Log (never panic) when an element's arena child list and its layout subtree
/// diverge in length at `id` during the `pass` traversal.
///
/// The arena and the layout tree are built in structural parallel and
/// positionally zipped in both the event and render passes (the arena↔layout
/// parallelism invariant, load-bearing until WS5.1 makes identity explicit).
/// A divergence must **degrade** — the callers keep their `.zip()`, which
/// truncates to the common prefix — rather than abort (WS3.5). This centralizes
/// the diagnostic so every pass reports the mismatch identically instead of
/// truncating silently. Returns whether a divergence was detected (so it is
/// unit-testable without capturing logs; callers use it as a statement).
pub(crate) fn check_children_parallel(
    pass: &str,
    id: ElId,
    arena_len: usize,
    layout_len: usize,
) -> bool {
    let diverged = arena_len != layout_len;
    if diverged {
        log::error!(
            "{pass}: arena/layout child divergence at {id:?} ({arena_len} \
             arena children vs {layout_len} layout children) — iterating the \
             common prefix, not aborting (WS3.5)"
        );
    }
    diverged
}

pub struct ElData<W: WidgetCtx> {
    // TODO: If rsact-reactive would support ?Sized as a real smart-pointer we
    // could do MaybeReactive<dyn Widget<W>>, so reactive elements creation
    // would be possible in place. But the problem is that MaybeReactive is a
    // readonly value, while MaybeSignal is owned stack value/Signal, so we
    // either change the MaybeSignal to StoredValue/Signal or create a new
    // MaybeSignal-like value with heap storage. We can't, Rust does not
    // allow unsized fields in structs, only through internal Box, Rc, etc. So
    // we cannot make a custom arena-allocated smart pointer.
    pub widget: Box<dyn Widget<W>>,

    pub state: ElState<W>,
}

impl<W: WidgetCtx> ElData<W> {
    pub fn new(widget: Box<dyn Widget<W>>) -> Self {
        let state = ElState::for_widget(widget.as_ref());

        Self { widget, state }
    }
}

pub enum El<W>
where
    W: WidgetCtx,
{
    New(ElData<W>),
    Stored { id: ElId, layout: Layout },
}

impl<W> El<W>
where
    W: WidgetCtx,
{
    pub(crate) fn new(widget: impl Widget<W> + 'static) -> Self {
        Self::New(ElData::new(Box::new(widget)))
    }

    pub(crate) fn layout(&self) -> Layout {
        match self {
            Self::New(data) => data.widget.layout(),
            Self::Stored { layout, .. } => *layout,
        }
    }

    // pub(crate) fn meta(&self, id: ElId) -> MetaTree {
    //     match self {
    //         Self::New(data) => data.widget.meta(id),
    //         Self::Stored(_) => {
    //             panic!("Stored element cannot be metaed without arena")
    //         },
    //     }
    // }

    // pub(crate) fn as_new(self) -> Result<ElData<W>, ElId> {
    //     match self {
    //         El::New(el_data) => Ok(el_data),
    //         El::Stored(el_id) => Err(el_id),
    //     }
    // }
}

// impl<W> Widget<W> for El<W>
// where
//     W: WidgetCtx + 'static,
// {
//     fn el(self) -> El<W>
//     where
//         Self: Sized + 'static,
//     {
//         self
//     }

//     fn meta(&self, _parent_id: ElId) -> MetaTree {
//         self.widget.meta(self.id)
//     }

//     fn layout(&self) -> Layout {
//         self.widget.layout()
//     }

//     #[track_caller]
//     fn render(&self, ctx: RenderCtx<'_, W>) -> RenderResult {
//         self.widget.render(ctx)
//     }

//     fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
//         ctx.id = self.id;
//         self.widget.on_event(ctx)
//     }
// }

#[cfg(test)]
mod tests {
    use super::{ElId, check_children_parallel};

    /// WS3.5: the arena↔layout divergence check flags a length mismatch (so it
    /// can be logged and degraded) and stays silent when the counts match.
    #[test]
    fn check_children_parallel_flags_only_divergence() {
        let id = ElId::default();
        assert!(
            check_children_parallel("test", id, 3, 2),
            "more arena than layout children must be flagged"
        );
        assert!(
            check_children_parallel("test", id, 2, 3),
            "fewer arena than layout children must be flagged"
        );
        assert!(
            check_children_parallel("test", id, 0, 1),
            "empty arena vs one layout child must be flagged"
        );
        assert!(
            !check_children_parallel("test", id, 2, 2),
            "equal counts must not be flagged"
        );
        assert!(
            !check_children_parallel("test", id, 0, 0),
            "both empty must not be flagged"
        );
    }
}
