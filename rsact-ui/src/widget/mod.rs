pub mod bar;
pub mod button;
pub mod canvas;
pub mod checkbox;
pub mod combinators;
pub mod container;
pub mod ctx;
pub mod edge;
pub mod flex;
pub mod icon;
pub mod image;
pub mod knob;
pub mod scrollable;
pub mod select;
pub mod show;
pub mod slider;
pub mod space;
pub mod text;

use crate::font::{Font, FontProps};
use bitflags::bitflags;
use prelude::*;
use rsact_reactive::maybe::IntoMaybeReactive;

pub type RenderResult = Result<(), ()>;

bitflags! {
    #[derive(Clone, Copy, PartialEq)]
    pub struct Behavior: u8 {
        const NONE = 0;
        const FOCUSABLE = 1 << 0;
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
}

// TODO: Custom MemoTree with SmallVec<T, 1>
pub type MetaTree = MemoTree<Meta>;

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
    fn meta(&self) -> MetaTree;

    // These functions MUST be called only ones per widget //
    fn on_mount(&mut self, ctx: MountCtx<W>);
    fn layout(&self) -> Signal<Layout>;

    // Hot-loop called functions //
    fn render(&self, ctx: &mut RenderCtx<'_, W>) -> RenderResult;
    // TODO: Reactive event context? Is it possible?
    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse;
}

/// Not implementing [`SizedWidget`] and [`BlockModelWidget`] does not mean that
/// Widget has layout without size or box model, it can be intentional to
/// disallow user to set size or box model properties.
pub trait SizedWidget<W: WidgetCtx>: Widget<W> {
    // TODO: MaybeReactive!
    fn size(self, size: Size<Length>) -> Self
    where
        Self: Sized + 'static,
    {
        self.width(size.width).height(size.height)
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
            layout.size.width = width.into();
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
            layout.size.height = height.into();
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

pub trait IntoWidget<W: WidgetCtx> {
    type Widget: Widget<W>;

    fn into_widget(self) -> Self::Widget;
}

pub mod prelude {
    pub use crate::{
        el::{El, ElId},
        event::{
            Capture, Event, EventResponse, FocusEvent, Propagate,
            message::UiMessage,
        },
        font::{FontSize, FontStyle},
        layout::{
            self, Align, ContainerLayout, FlexLayout, Layout, LayoutKind,
            LayoutModelNode, Limits,
            axis::{
                Anchor, Axial as _, Axis, AxisAnchorPoint, ColDir, Direction,
                RowDir,
            },
            block_model::BlockModel,
            padding::Padding,
            size::{Length, Size},
        },
        render::{Block, Border, Renderer, color::Color},
        style::{ColorStyle, WidgetStylist, block::*, declare_widget_style},
        widget::{
            BlockModelWidget, FontSettingWidget, Meta, MetaTree, RenderResult,
            SizedWidget, Widget, ctx::*,
        },
    };
    pub use alloc::{boxed::Box, string::String, vec::Vec};
    pub use rsact_reactive::prelude::*;
}
