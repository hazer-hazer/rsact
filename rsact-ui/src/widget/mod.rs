pub mod bar;
pub mod button;
pub mod canvas;
#[cfg(feature = "tiny-icons")]
pub mod checkbox;
pub mod combinators;
pub mod container;
pub mod ctx;
pub mod dynamic;
pub mod edge;
pub mod flex;
#[cfg(feature = "tiny-icons")]
pub mod icon;
// #[cfg(feature = "embedded-graphics")]
// pub mod image;
pub mod knob;
pub mod label;
pub mod scrollable;
pub mod select;
pub mod show;
pub mod slider;
pub mod space;

use crate::{
    font::{Font, FontProps, FontSize, FontStyle},
    layout::length::LengthSize,
};
use bitflags::bitflags;
use prelude::*;
use rsact_reactive::prelude::*;

bitflags! {
    #[derive(Clone, Copy, PartialEq)]
    pub struct Behavior: u8 {
        const NONE = 0;
        const FOCUSABLE = 1 << 0;
        const HOVERABLE = 1 << 1;
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Meta {
    pub behavior: Behavior,
    pub id: Option<ElId>,
}

impl Default for Meta {
    fn default() -> Self {
        Self::none()
    }
}

impl Meta {
    pub fn none() -> Self {
        Self { behavior: Behavior::NONE, id: None }
    }

    pub fn focusable(id: ElId) -> Self {
        Self { behavior: Behavior::FOCUSABLE, id: Some(id) }
    }

    pub fn hoverable(id: ElId) -> Self {
        Self { behavior: Behavior::HOVERABLE, id: Some(id) }
    }

    pub fn focusable_hoverable(id: ElId) -> Self {
        Self {
            behavior: Behavior::FOCUSABLE | Behavior::HOVERABLE,
            id: Some(id),
        }
    }

    // pub fn with_id(mut self, id: ElId) -> Self {
    //     self.id = Some(id);
    //     self
    // }
}

// TODO: MaybeReactive MetaTree
// TODO: Custom MemoTree with SmallVec<T, 1>
// pub type MetaTree = MemoTree<Meta>;

#[derive(PartialEq, Clone, Copy)]
pub struct MetaTree {
    // TODO: I don't see a place where meta needs to be reactive (or MaybeReactive).
    meta: Meta,
    // TODO: Optional vec to avoid useless allocations?
    children: MaybeReactive<Vec<MetaTree>>,
}

impl MetaTree {
    pub fn none() -> Self {
        Self::childless(Meta::none())
    }

    pub fn new(
        meta: Meta,
        children: impl IntoMaybeReactive<Vec<MetaTree>>,
    ) -> Self {
        Self { meta, children: children.maybe_reactive() }
    }

    pub fn childless(meta: Meta) -> Self {
        Self::new(meta, Vec::new().maybe_reactive())
    }

    pub fn flat_collect(&self) -> Vec<Meta> {
        self.children.with(|children| {
            core::iter::once(self.meta)
                .chain(children.iter().map(MetaTree::flat_collect).flatten())
                .collect()
        })
    }
}

// #[derive(PartialEq)]
// pub struct MetaTree {
//     data: MaybeReactive<Meta>,
//     children: MaybeReactive<Vec<MetaTree>>,
// }

// impl MetaTree {
//     pub fn flat_collect(&self) -> Vec<MaybeReactive<Meta>> {
//         self.children.with(|children| {
//             core::iter::once(self.data)
//                 .chain(children.iter().map(MetaTree::flat_collect).flatten())
//                 .collect()
//         })
//     }
// }

pub trait Widget<W>
where
    W: WidgetCtx,
{
    fn el(self) -> El<W>
    where
        Self: Sized + 'static,
    {
        El::new(self)
    }

    // TODO: Use MaybeReactive tree
    // TODO: Can rewrite so that meta is called once?
    fn meta(&self, id: ElId) -> MetaTree;

    fn layout(&self) -> Layout;

    // Hot-loop called functions //
    fn render(&self, ctx: RenderCtx<'_, W>) -> RenderResult;
    // TODO: Reactive event context? Is it possible?
    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse;
}

/// Not implementing [`SizedWidget`] and [`BlockModelWidget`] does not mean that
/// Widget has layout without size or box model, it can be intentional to
/// disallow user to set size or box model properties.
pub trait SizedWidget<W: WidgetCtx>: Widget<W> {
    // TODO: MaybeReactive!
    fn size(self, size: impl Into<LengthSize>) -> Self
    where
        Self: Sized + 'static,
    {
        let size = size.into();
        self.width(size.width()).height(size.height())
    }

    fn fill(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.width(Length::fill()).height(Length::fill())
    }

    fn fill_width(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.width(Length::fill())
    }

    fn shrink(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.width(Length::Shrink).height(Length::Shrink)
    }

    fn width<L: Into<Length> + PartialEq + Copy + 'static>(
        self,
        width: impl IntoMaybeReactive<L>,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(width.maybe_reactive(), |layout, &width| {
            layout.size.set_width(width.into());
        });
        self
    }

    fn height<L: Into<Length> + PartialEq + Copy + 'static>(
        self,
        height: impl IntoMaybeReactive<L> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(height.maybe_reactive(), |layout, &height| {
            layout.size.set_height(height.into());
        });
        self
    }

    fn fill_height(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.height(Length::fill())
    }
}

pub trait BlockModelWidget<W: WidgetCtx>: Widget<W> {
    fn border_width(
        self,
        border_width: impl IntoMaybeReactive<u32> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(
            border_width.maybe_reactive(),
            |layout, &border_width| {
                layout.set_border_width(border_width);
            },
        );
        self
    }

    fn padding<P: Into<Padding> + PartialEq + Copy + 'static>(
        self,
        padding: impl IntoMaybeReactive<P> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(padding.maybe_reactive(), |layout, &padding| {
            layout.set_padding(padding.into());
        });
        self
    }
}

pub trait FontSettingWidget<W: WidgetCtx>: Widget<W> + Sized + 'static {
    fn font_props(&self) -> FontProps {
        self.layout().with(|layout| layout.font_props().unwrap())
    }

    fn update_font_props(&mut self, update: impl FnOnce(&mut FontProps)) {
        self.layout()
            .update_untracked(|layout| update(layout.font_props_mut().unwrap()))
    }

    // Constructors //
    fn font_size<S: Into<FontSize> + Clone + PartialEq + 'static>(
        mut self,
        font_size: impl IntoMaybeReactive<S>,
    ) -> Self {
        // TODO: Wrong, we accept MaybeReactive but update only once. Use setter!
        // TODO: Warn on overwrite
        self.update_font_props(|font_props| {
            font_props.font_size = Some(
                font_size
                    .maybe_reactive()
                    .map(|font_size| font_size.clone().into()),
            )
        });

        self
    }

    fn font_style(
        mut self,
        font_style: impl IntoMaybeReactive<FontStyle>,
    ) -> Self {
        self.update_font_props(|font_props| {
            font_props.font_style = Some(font_style.maybe_reactive())
        });
        self
    }

    fn font<F: Into<Font> + PartialEq + Clone + 'static>(
        mut self,
        font: impl IntoMaybeReactive<F>,
    ) -> Self {
        self.update_font_props(|font_props| {
            font_props.font =
                Some(font.maybe_reactive().map(|font| font.clone().into()))
        });
        self
    }
}

pub mod prelude {
    pub use crate::render::prelude::*;
    pub use crate::{
        el::{El, ElId},
        event::{
            Capture, Event, EventResponse, FocusEvent, Propagate,
            message::UiMessage,
        },
        font::{
            Font, FontCtx, FontFamily, FontHandler, FontId, FontImport,
            FontProps, FontSize, FontStyle,
        },
        layout::{
            self, Align, ContainerLayout, FlexLayout, LayoutKind, Limits,
            length::Length, node::Layout,
        },
        style::declare_widget_style,
        widget::{
            BlockModelWidget, FontSettingWidget, Meta, MetaTree, SizedWidget,
            Widget, ctx::*,
        },
    };
    pub use alloc::{boxed::Box, string::String, vec::Vec};
    pub use rsact_reactive::prelude::*;
}
