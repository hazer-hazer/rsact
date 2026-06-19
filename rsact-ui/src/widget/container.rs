use crate::{
    style::{StyleFn, WidgetStyleFn},
    widget::prelude::*,
};

declare_widget_style! {
    ContainerStyle () {
        container: container,
    }
}

pub struct Container<W: WidgetCtx> {
    pub layout: Layout,
    pub content: El<W>,
    pub style: WidgetStyleFn<ContainerStyle<W::Color>>,
}

impl<W: WidgetCtx + 'static> Container<W> {
    pub fn new(content: impl Into<El<W>>) -> Self {
        let content = content.into();

        Self {
            layout: Layout::shrink(LayoutKind::Container(
                ContainerLayout::base(content.layout()),
            )),
            content,
            style: None,
        }
    }

    pub fn style(
        mut self,
        style: impl StyleFn<ContainerStyle<W::Color>>,
    ) -> Self {
        self.style = Some(Box::new(style));
        self
    }

    // TODO: Use MaybeReactive
    pub fn vertical_align(mut self, vertical_align: impl Into<Align>) -> Self {
        self.layout.update_untracked(|layout| {
            layout.expect_container_mut().vertical_align =
                vertical_align.into();
        });
        self
    }

    pub fn horizontal_align(
        mut self,
        horizontal_align: impl Into<Align>,
    ) -> Self {
        self.layout.update_untracked(|layout| {
            layout.expect_container_mut().horizontal_align =
                horizontal_align.into();
        });
        self
    }

    pub fn center(self) -> Self {
        self.vertical_align(Align::Center).horizontal_align(Align::Center)
    }

    // pub fn vertical_align(
    //     self,
    //     vertical_align: impl MaybeSignal<Align> + 'static,
    // ) -> Self {
    //     self.layout.setter(
    //         vertical_align.maybe_signal(),
    //         |&vertical_align, layout| {
    //             layout.expect_container_mut().vertical_align = vertical_align
    //         },
    //     );
    //     self
    // }

    // pub fn horizontal_align(
    //     self,
    //     horizontal_align: impl MaybeSignal<Align> + 'static,
    // ) -> Self {
    //     self.layout.setter(
    //         horizontal_align.maybe_signal(),
    //         |&horizontal_align, layout| {
    //             layout.expect_container_mut().horizontal_align =
    //                 horizontal_align
    //         },
    //     );
    //     self
    // }
}

impl<W: WidgetCtx + 'static> SizedWidget<W> for Container<W> {}
impl<W: WidgetCtx + 'static> BlockModelWidget<W> for Container<W> {}

impl<W: WidgetCtx> FontSettingWidget<W> for Container<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Container<W> {
    fn debug_name(&self) -> &'static str {
        "Container"
    }

    fn build(&mut self, mut ctx: BuildCtx<W>) {
        ctx.set_single_child(&mut self.content);
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    fn render(&self, mut ctx: RenderCtx<'_, W>) -> crate::widget::RenderResult {
        ctx.render_self(|ctx| {
            let style = ctx.get_style(self.style.as_deref());

            Block::from_layout_style(
                ctx.layout.outer,
                self.layout.with(|layout| layout.block_model()),
                style.container,
            )
            .render(ctx.renderer)
        })
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        // self.content.control_flow(|content| ctx.pass_to_child(content))
        ctx.ignore()
    }
}
