use crate::{
    event::EventResponse,
    layout::{Layout, LayoutKind},
    render::Renderer,
    widget::{Meta, MetaTree, Widget, WidgetCtx},
};
use embedded_graphics::{
    image::ImageRaw, iterator::raw::RawDataSlice, pixelcolor::raw::ByteOrder,
    prelude::*,
};
use rsact_reactive::{
    memo::{IntoMemo, MemoTree},
    signal::{IntoSignal, Signal},
};

/// Static Image
pub struct Image<'a, W: WidgetCtx, BO: ByteOrder> {
    // TODO: Reactive?
    data: ImageRaw<'a, W::Color, BO>,
    layout: Signal<Layout>,
}

impl<'a, W: WidgetCtx, BO: ByteOrder> Image<'a, W, BO> {
    pub fn new(data: ImageRaw<'a, W::Color, BO>) -> Self {
        let size = data.size().into();

        Self {
            data,
            layout: Layout { kind: LayoutKind::Edge, size }.into_signal(),
        }
    }
}

impl<'a, W: WidgetCtx, BO: ByteOrder> Widget<W> for Image<'a, W, BO>
where
    RawDataSlice<'a, <W::Color as PixelColor>::Raw, BO>:
        IntoIterator<Item = <W::Color as PixelColor>::Raw>,
{
    fn meta(&self) -> MetaTree {
        MetaTree::childless(Meta::none())
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        MemoTree::childless(self.layout.into_memo())
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, W>,
    ) -> crate::widget::DrawResult {
        ctx.renderer.image(embedded_graphics::image::Image::new(
            &self.data,
            ctx.layout.inner.top_left,
        ))
    }

    fn on_event(
        &mut self,
        _ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse<W> {
        W::ignore()
    }
}
