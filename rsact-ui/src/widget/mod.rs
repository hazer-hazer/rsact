pub mod bar;
pub mod button;
pub mod canvas;
pub mod checkbox;
pub mod combinators;
pub mod container;
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
    el::{build::BuildCtx, update::UpdateCtx},
    font::{Font, FontProps, FontSize, FontStyle},
    layout::length::LengthSize,
};
use core::any::Any;
use prelude::*;
use rsact_reactive::prelude::*;

pub trait Widget<W>: Any
where
    W: WidgetCtx,
{
    fn flags(&self) -> WidgetFlags {
        WidgetFlags::default()
    }

    fn debug_name(&self) -> &'static str {
        core::any::type_name::<Self>()
    }

    fn el(self) -> El<W>
    where
        Self: Sized + crate::el::build::Build<W> + 'static,
    {
        El::new(self)
    }

    // TRANSITIONAL (WS13 spec §2.2): `build` moves to `Build`. Kept as a
    // defaulted no-op so unconverted widgets keep their `fn build(&mut self)`
    // override (called by the derived identity `Build`). Deleted outright once
    // the fleet is split (13.4) — the final 7.6 shape.
    fn build(&mut self, _ctx: BuildCtx<W>) {}

    fn update(&mut self, mut ctx: UpdateCtx<'_, W>) -> UpdateResult {
        ctx.handle()
    }

    // TODO: Meta can be collected in build pass
    // TODO: Use MaybeReactive tree
    // TODO: Can rewrite so that meta is called once?
    // fn meta(&self, id: ElId) -> MetaTree;

    fn layout(&self) -> Layout;

    // Hot-loop called functions //
    // TODO: Reactive event context? Is it possible?
    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse;
    fn render(&self, ctx: RenderCtx<'_, W>) -> RenderResult;
}

/// Mutable access to a widget's own [`Layout`], used by the reactive-on-write
/// setters (`SizedWidget`/`BlockModelWidget`/`FontSettingWidget`) so a reactive
/// upgrade persists in the widget's stored field rather than a discarded copy.
/// Kept separate from [`Widget`] because some widgets (e.g. `Unit`, `Show`) do
/// not own a `Layout` and must not expose `layout_mut`.
pub trait LayoutWidget<W: WidgetCtx>: Widget<W> {
    fn layout_mut(&mut self) -> &mut Layout;
}

/// Not implementing [`SizedWidget`] and [`BlockModelWidget`] does not mean that
/// Widget has layout without size or box model, it can be intentional to
/// disallow user to set size or box model properties.
pub trait SizedWidget<W: WidgetCtx>: LayoutWidget<W> {
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

    fn width_fill(self) -> Self
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

    fn width<L: Into<Length> + PartialEq + Clone + 'static>(
        mut self,
        width: impl IntoMaybeReactive<L>,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout_mut()
            .setter(width.maybe_reactive(), |layout, width| {
                layout.size.set_width(width.clone().into());
            });
        self
    }

    fn width_shrink(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.width(Length::Shrink)
    }

    fn height<L: Into<Length> + PartialEq + Clone + 'static>(
        mut self,
        height: impl IntoMaybeReactive<L> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout_mut()
            .setter(height.maybe_reactive(), |layout, height| {
                layout.size.set_height(height.clone().into());
            });
        self
    }

    fn height_fill(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.height(Length::fill())
    }

    fn height_shrink(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.height(Length::Shrink)
    }
}

pub trait BlockModelWidget<W: WidgetCtx>: LayoutWidget<W> {
    fn border_width(
        mut self,
        border_width: impl IntoMaybeReactive<u32> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout_mut().setter(
            border_width.maybe_reactive(),
            |layout, &border_width| {
                layout.set_border_width(border_width);
            },
        );
        self
    }

    fn padding<P: Into<Padding> + PartialEq + Copy + 'static>(
        mut self,
        padding: impl IntoMaybeReactive<P> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout_mut().setter(
            padding.maybe_reactive(),
            |layout, &padding| {
                layout.set_padding(padding.into());
            },
        );
        self
    }
}

pub trait FontSettingWidget<W: WidgetCtx>:
    LayoutWidget<W> + Sized + 'static
{
    fn font_props(&self) -> FontProps {
        self.layout().with(|layout| layout.font_props().unwrap())
    }

    // Constructors //
    fn font_size<S: Into<FontSize> + Clone + PartialEq + 'static>(
        mut self,
        font_size: impl IntoMaybeReactive<S>,
    ) -> Self {
        self.layout_mut().setter(
            font_size.maybe_reactive(),
            |layout, font_size| {
                if let Some(font_props) = layout.font_props_mut() {
                    font_props.font_size = Some(font_size.clone().into());
                }
            },
        );
        self
    }

    fn font_style(
        mut self,
        font_style: impl IntoMaybeReactive<FontStyle>,
    ) -> Self {
        self.layout_mut().setter(
            font_style.maybe_reactive(),
            |layout, &font_style| {
                if let Some(font_props) = layout.font_props_mut() {
                    font_props.font_style = Some(font_style);
                }
            },
        );
        self
    }

    fn font<F: Into<Font> + PartialEq + Clone + 'static>(
        mut self,
        font: impl IntoMaybeReactive<F>,
    ) -> Self {
        self.layout_mut()
            .setter(font.maybe_reactive(), |layout, font| {
                if let Some(font_props) = layout.font_props_mut() {
                    font_props.font = Some(font.clone().into());
                }
            });
        self
    }
}

pub mod prelude {
    pub use crate::{
        el::*,
        event::{
            Capture, Event, EventResponse, FocusEvent, message::UiMessage,
        },
        font::{
            Font, FontCtx, FontFamily, FontHandler, FontId, FontImport,
            FontProps, FontSize, FontStyle,
        },
        layout::{
            self, Align, ContainerLayout, FlexLayout, LayoutKind, Limits,
            length::{Length, LengthSize},
            node::Layout,
        },
        render::prelude::*,
        style::{
            StyleFn, StylePseudoClass, WidgetStyleFn, declare_widget_style,
        },
        widget::{
            BlockModelWidget, FontSettingWidget, LayoutWidget, SizedWidget,
            Widget,
        },
    };
    pub use alloc::{boxed::Box, string::String, vec::Vec};
    pub use rsact_macros::View;
    pub use rsact_reactive::prelude::*;
}
