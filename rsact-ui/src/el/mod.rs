use crate::widget::prelude::*;
use alloc::boxed::Box;
use core::marker::PhantomData;

pub mod arena;
pub mod build;
pub mod ctx;
pub mod event;
pub mod flags;
pub mod render;
pub mod update;

pub use build::*;
pub use ctx::*;
pub use event::*;
pub use flags::WidgetFlags;
pub use render::*;
pub use update::*;

slotmap::new_key_type! {
    pub struct ElId;
}

/// Value bound with [`ElId`], used for hashing purposes
#[derive(Debug, Clone, Copy, Hash)]
pub struct WithElId<T> {
    id: ElId,
    value: T,
}

impl<T> WithElId<T> {
    pub fn new(id: ElId, value: T) -> Self {
        Self { id, value }
    }
}

pub enum ClipPath {
    // Rect(Rect),
    InnerRect,
}

pub enum RedrawReason {
    PseudoclassChange,
}

pub struct ElState<W: WidgetCtx> {
    _marker: PhantomData<W>,

    pub built: bool,

    pub debug_name: &'static str,

    pub flags: WidgetFlags,

    // TODO:Move ElState to a child module to hide implementation for hovers, etc. Because we should never set hover for non-hoverable widgets and need to encapsulate this logic.
    // Action state //
    pub hovered: bool,

    // // Styling //
    // pub pseudoclass: StylePseudoClass,

    // Rendering //
    pub needs_redraw: Option<RedrawReason>,
    pub clip_path: Option<ClipPath>,
}

pub struct ElData<W: WidgetCtx> {
    // TODO: If rsact-reactive would support ?Sized as a real smart-pointer we could do MaybeReactive<dyn Widget<W>>, so reactive elements creation would be possible in place. But the problem is that MaybeReactive is a readonly value, while MaybeSignal is owned stack value/Signal, so we either change the MaybeSignal to StoredValue/Signal or create a new MaybeSignal-like value with heap storage.
    // We can't, Rust does not allow unsized fields in structs, only through internal Box, Rc, etc. So we cannot make a custom arena-allocated smart pointer.
    pub widget: Box<dyn Widget<W>>,

    pub state: ElState<W>,
}

impl<W: WidgetCtx> ElData<W> {
    pub fn new(widget: Box<dyn Widget<W>>) -> Self {
        let debug_name = Self::pretty_type_name(widget.as_ref().debug_name());
        let flags = widget.flags();

        Self {
            widget,
            state: ElState {
                _marker: PhantomData,
                debug_name,
                flags,
                built: false,

                hovered: false,

                needs_redraw: None,
                clip_path: None,
                // pseudoclass: StylePseudoClass::default(),
            },
        }
    }

    fn pretty_type_name(debug_name: &'static str) -> &'static str {
        // TODO
        debug_name
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
