use crate::widget::prelude::*;
use rsact_core::memo_chain::IntoMemoChain;

pub struct Edge<W: WidgetCtx> {
    pub layout: Signal<Layout>,
    style: MemoChain<BoxStyle<W::Color>>,
}

impl<W: WidgetCtx + 'static> Edge<W> {
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
        styler: impl Fn(BoxStyle<W::Color>) -> BoxStyle<W::Color> + 'static,
    ) -> Self {
        self.style.last(move |prev_style| styler(*prev_style));
        self
    }
}

impl<W: WidgetCtx + 'static> SizedWidget<W> for Edge<W> {}
impl<W: WidgetCtx + 'static> BoxModelWidget<W> for Edge<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Edge<W> {
    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn on_mount(&mut self, _ctx: crate::widget::MountCtx<W>) {}

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        MemoTree::childless(self.layout.into_memo())
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        let style = self.style.get();

        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.area,
            self.layout.get().box_model,
            style,
        ))
    }

    fn on_event(
        &mut self,
        _ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> crate::event::EventResponse<<W as WidgetCtx>::Event> {
        Propagate::Ignored.into()
    }
}

impl<W> From<Edge<W>> for El<W>
where
    W: WidgetCtx + 'static,
{
    fn from(value: Edge<W>) -> Self {
        El::new(value)
    }
}
