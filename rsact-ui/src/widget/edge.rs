use crate::{
    render::Renderable,
    widget::{Meta, MetaTree, prelude::*},
};
use rsact_reactive::memo_chain::IntoMemoChain;

pub struct Edge<W: WidgetCtx> {
    pub layout: Layout,
    style: MemoChain<BlockStyle<W::Color>>,
}

impl<W: WidgetCtx + 'static> Edge<W> {
    pub fn new() -> Self {
        Self {
            layout: Layout::shrink(LayoutKind::Edge).size(Size::fill()),
            style: BlockStyle::base().memo_chain(),
        }
    }

    pub fn style(
        self,
        styler: impl Fn(BlockStyle<W::Color>) -> BlockStyle<W::Color> + 'static,
    ) -> Self {
        self.style.last(move |prev_style| styler(*prev_style)).unwrap();
        self
    }
}

impl<W: WidgetCtx + 'static> SizedWidget<W> for Edge<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Edge<W> {
    fn meta(&self) -> crate::widget::MetaTree {
        MetaTree::childless(Meta::none)
    }

    fn layout(&self) -> &Layout {
        &self.layout
    }

    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }

    fn on_mount(&mut self, _ctx: crate::widget::MountCtx<W>) {}

    fn render(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        let style = self.style.get();

        Block::from_layout_style(
            ctx.layout.outer,
            self.layout.block_model(),
            style,
        )
        .render(ctx.renderer)
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse {
        ctx.ignore()
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
