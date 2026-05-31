use crate::{
    layout::length::LengthSize,
    style::WidgetStyleFn,
    widget::{MetaTree, prelude::*},
};

pub struct Edge<W: WidgetCtx> {
    pub layout: Layout,
    style: WidgetStyleFn<BlockStyle<W::Color>>,
}

impl<W: WidgetCtx + 'static> Edge<W> {
    pub fn new() -> Self {
        Self {
            layout: Layout::shrink(LayoutKind::Edge).size(LengthSize::fill()),
            style: None,
        }
    }

    pub fn style(
        mut self,
        styler: impl (Fn(BlockStyle<W::Color>) -> BlockStyle<W::Color>) + 'static,
    ) -> Self {
        self.style = Some(Box::new(styler));
        self
    }
}

impl<W: WidgetCtx + 'static> SizedWidget<W> for Edge<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Edge<W> {
    fn meta(&self, _: ElId) -> crate::widget::MetaTree {
        MetaTree::none()
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, ctx: &mut RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self("Edge", |ctx| {
            let base = BlockStyle::base();
            let style = self.style.as_ref().map(|f| f(base)).unwrap_or(base);

            Block::from_layout_style(
                ctx.layout.outer,
                self.layout.with(|layout| layout.block_model()),
                style,
            )
            .render(ctx.renderer())
        })
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
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
