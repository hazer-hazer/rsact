use crate::{
    el::El,
    layout::{
        box_model::BoxModel,
        size::{Length, Size},
        Align, ContainerLayout, Layout, LayoutKind, Limits,
    },
    render::{Block, Renderer},
    style::BoxStyle,
    widget::{DrawCtx, LayoutCtx, Widget, WidgetCtx},
};
use rsact_core::{
    prelude::*,
    signal::{marker::ReadOnly, EcoSignal, IntoSignal, ReadSignal, SignalTree},
};

pub struct Container<C: WidgetCtx> {
    pub layout: Signal<Layout>,
    pub content: Signal<El<C>>,
    pub style: Signal<BoxStyle<C::Color>>,
}

impl<C: WidgetCtx + 'static> Container<C> {
    pub fn new(content: impl IntoSignal<El<C>>) -> Self {
        let content = content.signal();

        Self {
            layout: use_signal(Layout {
                // TODO: Container layout settings
                kind: LayoutKind::Container(ContainerLayout::base()),
                size: Size::shrink(),
                box_model: BoxModel::zero(),
                content_size: use_computed(move || {
                    content.with(move |content| {
                        content
                            .layout()
                            .with(move |content| content.content_size.get())
                    })
                }),
            }),
            content,
            style: use_signal(BoxStyle::base()),
        }
    }

    pub fn vertical_align(
        self,
        vertical_align: impl EcoSignal<Align> + 'static,
    ) -> Self {
        let vertical_align = vertical_align.eco_signal();
        use_memo(move || {
            let vertical_align = vertical_align.get();
            self.layout.update(move |layout| {
                layout.expect_container_mut().vertical_align = vertical_align
            });
            vertical_align
        });
        self
    }

    pub fn horizontal_align(
        self,
        horizontal_align: impl EcoSignal<Align> + 'static,
    ) -> Self {
        let horizontal_align = horizontal_align.eco_signal();
        use_memo(move || {
            let horizontal_align = horizontal_align.get();
            self.layout.update(move |layout| {
                layout.expect_container_mut().horizontal_align =
                    horizontal_align
            });
            horizontal_align
        });
        self
    }
}

impl<C: WidgetCtx + 'static> Widget<C> for Container<C> {
    // fn children(&self) -> &[El<C>] {
    //     core::slice::from_ref(&self.content)
    // }

    // fn children_mut(&mut self) -> &mut [El<C>] {
    //     core::slice::from_mut(&mut self.content)
    // }

    // fn children(&self) -> Signal<Vec<El<C>>> {}

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

    fn build_layout_tree(&self) -> SignalTree<Layout> {
        SignalTree {
            data: self.layout,
            children: vec![self
                .content
                .with(|content| content.build_layout_tree())],
        }
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> crate::widget::DrawResult {
        let style = self.style.get();

        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.area,
            self.layout.get().box_model,
            style,
        ))?;

        self.content.with(|content| ctx.draw_child(content))
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, C>,
    ) -> crate::event::EventResponse<<C as WidgetCtx>::Event> {
        self.content.maybe_update(|content| content.on_event(ctx))
    }
}
