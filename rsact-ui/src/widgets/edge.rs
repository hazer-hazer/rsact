use crate::widget::prelude::*;

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
                content_size: use_signal(Limits::unknown()),
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
    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> rsact_core::signal::SignalTree<Layout> {
        SignalTree { data: self.layout, children: use_computed(Vec::new) }
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
