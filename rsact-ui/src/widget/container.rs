use crate::{
    render::Renderable,
    widget::{BlockModelWidget, Meta, MetaTree, SizedWidget, prelude::*},
};

pub struct Container<W: WidgetCtx> {
    pub layout: Signal<Layout>,
    pub content: El<W>,
    pub style: MemoChain<BlockStyle<W::Color>>,
}

impl<W: WidgetCtx + 'static> Container<W> {
    pub fn new(content: impl Widget<W> + 'static) -> Self {
        let content = content.el();

        Self {
            layout: Layout::shrink(LayoutKind::Container(
                ContainerLayout::base(content.layout()),
            ))
            .signal(),
            content,
            style: BlockStyle::base().memo_chain(),
        }
    }

    pub fn style(
        self,
        style: impl (Fn(BlockStyle<W::Color>) -> BlockStyle<W::Color>) + 'static,
    ) -> Self {
        self.style.last(move |prev_style| style(*prev_style)).unwrap();
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
    fn meta(&self, id: ElId) -> MetaTree {
        let content_tree = self.content.meta(id);

        MetaTree {
            data: Meta::none.memo(),
            children: vec![content_tree].inert().memo(),
        }
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        // ctx.accept_styles(self.style, ());
        ctx.pass_to_child(self.layout, &mut self.content);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn render(
        &self,
        ctx: &mut RenderCtx<'_, W>,
    ) -> crate::widget::RenderResult {
        ctx.render_self(|ctx| {
            let style = self.style.get();

            Block::from_layout_style(
                ctx.layout.outer,
                self.layout.with(|layout| layout.block_model()),
                style,
            )
            .render(ctx.renderer())
        })?;

        ctx.render_child(&self.content)
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        // self.content.control_flow(|content| ctx.pass_to_child(content))
        ctx.pass_to_child(&mut self.content)
    }
}
