use crate::{
    event::Propagate,
    layout::{
        box_model::BoxModel,
        size::{Length, Size},
        EdgeLayout, Layout, LayoutKind, Limits,
    },
    render::{Block, Renderer},
    style::BoxStyle,
    widget::{DrawCtx, DrawResult, Widget, WidgetCtx},
};
use rsact_core::{
    prelude::*,
    signal::{EcoSignal, ReadSignal, SignalTree},
};

pub struct Edge<C: WidgetCtx> {
    pub layout: Signal<Layout>,
    style: Signal<BoxStyle<C::Color>>,
}

impl<C: WidgetCtx + 'static> Edge<C> {
    pub fn new() -> Self {
        Self {
            layout: use_signal(Layout {
                kind: LayoutKind::Edge(EdgeLayout {}),
                size: Size::shrink(),
                box_model: BoxModel::zero(),
                content_size: use_signal(Limits::unknown()).read_only(),
            }),
            style: use_signal(BoxStyle::base()),
        }
    }

    pub fn with_style(self, new: BoxStyle<C::Color>) -> Self {
        self.style.update_untracked(|style| *style = new);
        self
    }
}

impl<C: WidgetCtx + 'static> Widget<C> for Edge<C> {
    // fn size(&self) -> Size<Length> {
    //     self.layout.size.get()
    // }

    // fn content_size(&self) -> Limits {
    //     Limits::unknown()
    // }

    // fn layout(&self, _ctx: &LayoutCtx<'_, C>) -> LayoutKind {
    //     LayoutKind::Edge(self.layout.kind.get())
    // }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> rsact_core::signal::SignalTree<Layout> {
        SignalTree { data: self.layout, children: vec![] }
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> DrawResult {
        let style = self.style.get();

        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.area,
            self.layout.get().box_model,
            style,
        ))
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, C>,
    ) -> crate::event::EventResponse<<C as WidgetCtx>::Event> {
        Propagate::Ignored.into()
    }
}
