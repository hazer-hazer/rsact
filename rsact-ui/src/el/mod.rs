use crate::layout::LayoutData;
use crate::widget::prelude::*;
use alloc::boxed::Box;
use core::fmt::Debug;

pub mod arena;
pub mod build;
pub mod ctx;
pub mod dirty;
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

/// Two-stage arena payload: a builder before the build pass, the retained
/// widget after (WS13 spec §2.3).
// TODO: If rsact-reactive would support ?Sized as a real smart-pointer we
// could do MaybeReactive<dyn Widget<W>>, so reactive elements creation
// would be possible in place. But the problem is that MaybeReactive is a
// readonly value, while MaybeSignal is owned stack value/Signal, so we
// either change the MaybeSignal to StoredValue/Signal or create a new
// MaybeSignal-like value with heap storage. We can't, Rust does not
// allow unsized fields in structs, only through internal Box, Rc, etc. So
// we cannot make a custom arena-allocated smart pointer.
pub enum ElStage<W: WidgetCtx> {
    Unbuilt(Box<dyn Build<W>>),
    Built(Box<dyn Widget<W>>),
}

impl<W: WidgetCtx> ElStage<W> {
    /// The retained widget, once built. `None` (logged) if still unbuilt — a
    /// bug on any hot-loop path; callers degrade rather than panic.
    pub(crate) fn built(&self) -> Option<&dyn Widget<W>> {
        match self {
            Self::Built(w) => Some(w.as_ref()),
            Self::Unbuilt(_) => {
                log::error!("Element accessed as widget before build");
                None
            },
        }
    }

    pub(crate) fn built_mut(
        &mut self,
    ) -> Option<&mut (dyn Widget<W> + 'static)> {
        match self {
            Self::Built(w) => Some(w.as_mut()),
            Self::Unbuilt(_) => {
                log::error!("Element accessed as widget before build");
                None
            },
        }
    }
}

pub struct ElData<W: WidgetCtx> {
    pub stage: ElStage<W>,
    pub state: ElState<W>,
}

impl<W: WidgetCtx> ElData<W> {
    pub fn new(builder: Box<dyn Build<W>>) -> Self {
        let state = ElState::for_builder(builder.as_ref());
        Self { stage: ElStage::Unbuilt(builder), state }
    }

    /// Read-only access to the retained widget (post-build). For sites that
    /// need `widget` + `state` disjointly, use `self.stage.built_mut()` and
    /// `&mut self.state` directly (field-split borrow).
    pub fn widget(&self) -> Option<&dyn Widget<W>> {
        self.stage.built()
    }
}

pub enum El<W>
where
    W: WidgetCtx,
{
    New(ElData<W>),
    // WS5.1: the arena OWNS the `LayoutData` (keyed by `ElId`) now, so the
    // stored husk no longer carries a `Layout` handle — it is pure identity.
    Stored { id: ElId },
}

impl<W> El<W>
where
    W: WidgetCtx,
{
    pub(crate) fn new(builder: impl Build<W> + 'static) -> Self {
        Self::New(ElData::new(Box::new(builder)))
    }

    /// WS5.1: the element's owned initial [`LayoutData`], read by
    /// [`ElArena::add`] before the node is stored (the arena then owns it,
    /// keyed by `ElId`). Only meaningful on a `New` (pre-build) element — a
    /// `Stored` husk no longer carries layout (the arena does), so it degrades
    /// to a zero layout (unreachable: `add` reads this before storing).
    pub(crate) fn layout_data(&self) -> LayoutData {
        match self {
            Self::New(data) => match &data.stage {
                ElStage::Unbuilt(b) => b.layout_data(),
                ElStage::Built(w) => w.layout_data(),
            },
            Self::Stored { .. } => {
                log::error!(
                    "layout_data() on a Stored element — arena owns it"
                );
                LayoutData::zero()
            },
        }
    }

    /// WS5.1: set the `show` visibility memo on this (pre-build) element's
    /// builder layout. Used by `Show` to hide its wrapped child off the graph.
    pub(crate) fn set_layout_show(&mut self, show: Memo<bool>) {
        match self {
            Self::New(data) => match &mut data.stage {
                ElStage::Unbuilt(b) => b.set_show(show),
                ElStage::Built(_) => {
                    log::error!("set_layout_show() on a built element");
                },
            },
            Self::Stored { .. } => {
                log::error!("set_layout_show() on a Stored element");
            },
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
