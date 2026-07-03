use crate::widget::prelude::*;

declare_widget_style! {
    EdgeStyle () {
        container: container,
    }
}

// TODO: Edge is wrong, LayoutKind::Edge is used while BlockStyle can be set in EdgeStyle::container, we need to decide what edge should be, maybe it even should be any renderable Primitive.

#[derive(View)]
pub struct Edge<W: WidgetCtx> {
    pub layout: Layout,
    style: WidgetStyleFn<EdgeStyle<W::Color>>,
}

impl<W: WidgetCtx + 'static> Edge<W> {
    pub fn new() -> Self {
        Self { layout: Layout::shrink(LayoutKind::Edge), style: None }
    }

    pub fn style(mut self, class: impl StyleFn<EdgeStyle<W::Color>>) -> Self {
        self.style = Some(Box::new(class));
        self
    }
}

impl<W: WidgetCtx + 'static> LayoutWidget<W> for Edge<W> {
    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }
}

impl<W: WidgetCtx + 'static> SizedWidget<W> for Edge<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Edge<W> {
    fn debug_name(&self) -> &'static str {
        "Edge"
    }

    fn build(&mut self, ctx: BuildCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self(|ctx| {
            let style = ctx.get_style(self.style.as_deref());

            log::info!("Edge style: {:?}", style);

            Block::from_layout_style(
                ctx.layout.outer,
                self.layout.with(|layout| layout.block_model()),
                style.container,
            )
            .render(ctx.renderer)
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
