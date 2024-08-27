use crate::{
    el::El,
    layout::{
        box_model::BoxModel,
        size::{Length, Size},
        ContainerLayout, Layout, LayoutKind, Limits,
    },
    render::{Block, Renderer},
    style::BoxStyle,
    widget::{DrawCtx, LayoutCtx, Widget, WidgetCtx},
};
use rsact_core::{prelude::*, signal::ReadSignal};

pub struct Container<C: WidgetCtx> {
    pub layout: Signal<Layout>,
    pub content: El<C>,
    pub style: Signal<BoxStyle<C::Color>>,
}

impl<C: WidgetCtx + 'static> Container<C> {
    pub fn new(content: El<C>) -> Self {
        let content_layout = content.layout();

        Self {
            // layout: Layout::new(Size::shrink(), ContainerLayout::base()),
            layout: use_signal(Layout {
                // TODO: Container layout settings
                kind: LayoutKind::Container(ContainerLayout::base()),
                size: Size::shrink(),
                box_model: BoxModel::zero(),
                content_size: content_layout
                    .with(|content_layout| content_layout.content_size),
            }),
            content,
            style: use_signal(BoxStyle::base()),
        }
    }
}

impl<C: WidgetCtx + 'static> Widget<C> for Container<C> {
    fn children(&self) -> &[El<C>] {
        core::slice::from_ref(&self.content)
    }

    fn children_mut(&mut self) -> &mut [El<C>] {
        core::slice::from_mut(&mut self.content)
    }

    // fn size(&self) -> Size<Length> {
    //     self.layout.size.get()
    // }

    // fn content_size(&self) -> Limits {
    //     self.content.content_size()
    // }

    // fn layout(&self, _ctx: &LayoutCtx<'_, C>) -> LayoutKind {
    //     LayoutKind::Container(self.layout.kind.get())
    // }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> crate::widget::DrawResult {
        let style = self.style.get();

        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.area,
            self.layout.get().box_model,
            style,
        ))?;

        ctx.draw_children(self.children())
    }
}
