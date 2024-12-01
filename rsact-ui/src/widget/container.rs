use crate::{
    render::Renderable,
    widget::{prelude::*, BlockModelWidget, Meta, MetaTree, SizedWidget},
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
                ContainerLayout::base(
                    content.layout().map(|layout| layout.content_size()),
                ),
            ))
            .signal(),
            content,
            style: BlockStyle::base().memo_chain(),
        }
    }

    pub fn style(
        self,
        style: impl Fn(BlockStyle<W::Color>) -> BlockStyle<W::Color> + 'static,
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

impl<W: WidgetCtx + 'static> Widget<W> for Container<W> {
    fn meta(&self) -> MetaTree {
        let content_tree = self.content.meta();

        MetaTree {
            data: Meta::none.memo(),
            children: vec![content_tree].inert().memo(),
        }
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        // ctx.accept_styles(self.style, ());
        // ctx.pass_to_child(self.content);
        self.content.on_mount(ctx);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        let content_tree = self.content.build_layout_tree();
        MemoTree {
            data: self.layout.memo(),
            children: create_memo(move |_| vec![content_tree]),
        }
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> crate::widget::DrawResult {
        let style = self.style.get();

        Block::from_layout_style(
            ctx.layout.outer,
            self.layout.with(|layout| layout.block_model()),
            style,
        )
        .render(ctx.renderer)?;

        // self.content.with(|content| ctx.draw_child(content))
        ctx.draw_child(&self.content)
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse<W> {
        // self.content.control_flow(|content| ctx.pass_to_child(content))
        ctx.pass_to_child(&mut self.content)
    }
}
