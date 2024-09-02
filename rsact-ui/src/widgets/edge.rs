use crate::widget::prelude::*;

pub struct Edge<C: WidgetCtx> {
    pub layout: Signal<Layout>,
    style: Signal<BoxStyle<C::Color>>,
}

impl<C: WidgetCtx + 'static> Edge<C> {
    pub fn new() -> Self {
        Self {
            layout: Layout::new(LayoutKind::Edge, Limits::zero().into_memo())
                .size(Size::fill())
                .into_signal(),
            style: BoxStyle::base().into_signal(),
        }
    }

    pub fn style(self, style: impl IntoMemo<BoxStyle<C::Color>>) -> Self {
        self.style.set_from(style.into_memo());
        self
    }
}

impl<C: WidgetCtx + 'static> Widget<C> for Edge<C> {
    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        MemoTree::childless(self.layout.into_memo())
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

impl<C> From<Edge<C>> for El<C>
where
    C: WidgetCtx + 'static,
{
    fn from(value: Edge<C>) -> Self {
        El::new(value)
    }
}
