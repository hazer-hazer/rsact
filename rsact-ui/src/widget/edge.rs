use crate::widget::prelude::*;

declare_widget_style! {
    EdgeStyle () {
        container: container,
    }
}

// TODO: Edge is wrong, LayoutKind::Edge is used while BlockStyle can be set in EdgeStyle::container, we need to decide what edge should be, maybe it even should be any renderable Primitive.

#[derive(Builder)]
#[builds(Edge<W>)]
pub struct EdgeBuilder<W: WidgetCtx> {
    #[widget]
    layout: Layout,
    #[widget]
    style: WidgetStyleFn<EdgeStyle<W::Color>>,
}

pub struct Edge<W: WidgetCtx> {
    layout: Layout,
    style: WidgetStyleFn<EdgeStyle<W::Color>>,
}

impl<W: WidgetCtx + 'static> Edge<W> {
    pub fn new() -> EdgeBuilder<W> {
        EdgeBuilder { layout: Layout::shrink(LayoutKind::Edge), style: None }
    }
}

impl<W: WidgetCtx + 'static> EdgeBuilder<W> {
    pub fn style(mut self, class: impl StyleFn<EdgeStyle<W::Color>>) -> Self {
        self.style = Some(Box::new(class));
        self
    }
}

impl<W: WidgetCtx + 'static> LayoutWidget<W> for EdgeBuilder<W> {
    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }
}

impl<W: WidgetCtx + 'static> SizedWidget<W> for EdgeBuilder<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Edge<W> {
    // NOTE: no `flags`/`debug_name` override on the retained widget — both
    // are read exactly once, pre-build, from `Build` (seeding `ElState` at
    // `state.rs:72`); post-build all consumption is via `ElState`, so an
    // override here would be dead duplication of `EdgeBuilder`'s derived
    // `Build::debug_name` ("Edge" from `#[builds(Edge<W>)]`). `Edge` never
    // overrode `flags` either, so no `#[flags(...)]` attr is needed on
    // `EdgeBuilder`.
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
