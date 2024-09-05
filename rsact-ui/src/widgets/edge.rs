use crate::widget::prelude::*;
use rsact_core::memo_chain::IntoMemoChain;

pub struct Edge<C: WidgetCtx> {
    pub layout: Signal<Layout>,
    style: MemoChain<BoxStyle<C::Color>>,
}

impl<C: WidgetCtx + 'static> Edge<C> {
    pub fn new() -> Self {
        Self {
            layout: Layout::new(LayoutKind::Edge, Limits::zero().into_memo())
                .size(Size::fill())
                .into_signal(),
            style: BoxStyle::base().into_memo_chain(),
        }
    }

    pub fn style(
        self,
        styler: impl Fn(BoxStyle<C::Color>) -> BoxStyle<C::Color> + 'static,
    ) -> Self {
        self.style.last(move |prev_style| styler(*prev_style));
        self
    }
}

impl<C: WidgetCtx + 'static> SizedWidget<C> for Edge<C> {}
impl<C: WidgetCtx + 'static> BoxModelWidget<C> for Edge<C> {}

impl<C: WidgetCtx + 'static> Widget<C> for Edge<C> {
    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn on_mount(&mut self, _ctx: crate::widget::MountCtx<C>) {}

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
        _ctx: &mut crate::widget::EventCtx<'_, C>,
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
