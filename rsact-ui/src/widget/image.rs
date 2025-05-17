use crate::{
    el::ElId,
    event::EventResponse,
    layout::Layout,
    render::Renderable,
    widget::{Meta, MetaTree, Widget, WidgetCtx},
};
use embedded_graphics::{
    image::ImageRaw, iterator::raw::RawDataSlice, pixelcolor::raw::ByteOrder,
    prelude::*,
};
use rsact_reactive::signal::{IntoSignal, Signal};

use super::ctx::EventCtx;

/// Static Image
pub struct Image<'a, W: WidgetCtx, BO: ByteOrder> {
    // TODO: Reactive?
    data: ImageRaw<'a, W::Color, BO>,
    layout: Signal<Layout>,
}

impl<'a, W: WidgetCtx, BO: ByteOrder> Image<'a, W, BO> {
    pub fn new(data: ImageRaw<'a, W::Color, BO>) -> Self {
        let size = data.size().into();

        Self { data, layout: Layout::edge(size).signal() }
    }
}

impl<'a, W: WidgetCtx, BO: ByteOrder> Widget<W> for Image<'a, W, BO>
where
    RawDataSlice<'a, <W::Color as PixelColor>::Raw, BO>:
        IntoIterator<Item = <W::Color as PixelColor>::Raw>,
{
    fn meta(&self, _: ElId) -> MetaTree {
        MetaTree::childless(Meta::none)
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn render(
        &self,
        ctx: &mut crate::widget::RenderCtx<'_, W>,
    ) -> crate::widget::RenderResult {
        ctx.render_self(|ctx| {
            embedded_graphics::image::Image::new(
                &self.data,
                ctx.layout.inner.top_left,
            )
            .render(ctx.renderer())
        })
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.ignore()
    }
}
