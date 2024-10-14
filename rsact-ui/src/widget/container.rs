use crate::widget::{
    prelude::*, BlockModelWidget, Meta, MetaTree, SizedWidget,
};
use rsact_reactive::memo_chain::IntoMemoChain;

pub struct Container<W: WidgetCtx> {
    pub layout: Signal<Layout>,
    pub content: Signal<El<W>>,
    pub style: MemoChain<BlockStyle<W::Color>>,
}

impl<W: WidgetCtx + 'static> Container<W> {
    pub fn new(content: impl IntoSignal<El<W>>) -> Self {
        let content = content.into_signal();

        Self {
            layout: Layout::shrink(LayoutKind::Container(
                ContainerLayout::base(content.mapped(|content| {
                    content.layout().with(|layout| layout.content_size())
                })),
            ))
            .into_signal(),
            content,
            style: BlockStyle::base().into_memo_chain(),
        }
    }

    pub fn style(
        self,
        style: impl Fn(BlockStyle<W::Color>) -> BlockStyle<W::Color> + 'static,
    ) -> Self {
        self.style.last(move |prev_style| style(*prev_style));
        self
    }

    pub fn vertical_align(
        self,
        vertical_align: impl MaybeSignal<Align> + 'static,
    ) -> Self {
        self.layout.setter(
            vertical_align.maybe_signal(),
            |&vertical_align, layout| {
                layout.expect_container_mut().vertical_align = vertical_align
            },
        );
        self
    }

    pub fn horizontal_align(
        self,
        horizontal_align: impl MaybeSignal<Align> + 'static,
    ) -> Self {
        self.layout.setter(
            horizontal_align.maybe_signal(),
            |&horizontal_align, layout| {
                layout.expect_container_mut().horizontal_align =
                    horizontal_align
            },
        );
        self
    }
}

impl<W: WidgetCtx + 'static> SizedWidget<W> for Container<W> {}
impl<W: WidgetCtx + 'static> BlockModelWidget<W> for Container<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Container<W> {
    fn meta(&self) -> MetaTree {
        MetaTree {
            data: Meta::none().into_memo(),
            children: self.content.mapped(|content| vec![content.meta()]),
        }
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        // ctx.accept_styles(self.style, ());
        ctx.pass_to_child(self.content);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        let content = self.content;
        MemoTree {
            data: self.layout.into_memo(),
            children: use_memo(move |_| {
                content.with(|content| vec![content.build_layout_tree()])
            }),
        }
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> crate::widget::DrawResult {
        let style = self.style.get();

        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.outer,
            self.layout.get().block_model(),
            style,
        ))?;

        self.content.with(|content| ctx.draw_child(content))
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse<W> {
        self.content.control_flow(|content| ctx.pass_to_child(content))
    }
}

// FIXME: Remove?
impl<W> From<Container<W>> for El<W>
where
    W: WidgetCtx + 'static,
{
    fn from(value: Container<W>) -> Self {
        El::new(value)
    }
}
