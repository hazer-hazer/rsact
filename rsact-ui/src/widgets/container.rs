use rsact_core::memo_chain::IntoMemoChain;

use crate::widget::{prelude::*, BoxModelWidget, SizedWidget};

pub struct Container<C: WidgetCtx> {
    pub layout: Signal<Layout>,
    pub content: Signal<El<C>>,
    pub style: MemoChain<BoxStyle<C::Color>>,
}

impl<C: WidgetCtx + 'static> Container<C> {
    pub fn new(content: impl IntoSignal<El<C>>) -> Self {
        let content = content.into_signal();

        Self {
            layout: Layout::new(
                LayoutKind::Container(ContainerLayout::base()),
                content.mapped(|content| {
                    content.layout().with(|layout| layout.content_size.get())
                }),
            )
            .into_signal(),
            content,
            style: BoxStyle::base().into_memo_chain(),
        }
    }

    pub fn style(
        self,
        style: impl Fn(BoxStyle<C::Color>) -> BoxStyle<C::Color> + 'static,
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

impl<C: WidgetCtx + 'static> SizedWidget<C> for Container<C> {}
impl<C: WidgetCtx + 'static> BoxModelWidget<C> for Container<C> {}

impl<C: WidgetCtx + 'static> Widget<C> for Container<C> {
    fn children_ids(&self) -> Memo<Vec<ElId>> {
        let content = self.content;
        content.with(Widget::children_ids)
    }

    fn on_mount(&mut self, _ctx: crate::widget::MountCtx<C>) {
        // ctx.accept_styles(self.style, ());
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

    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> crate::widget::DrawResult {
        let style = self.style.get();

        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.area,
            self.layout.get().box_model,
            style,
        ))?;

        self.content.with(|content| ctx.draw_child(content))
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, C>,
    ) -> crate::event::EventResponse<<C as WidgetCtx>::Event> {
        self.content.control_flow(|content| content.on_event(ctx))
    }
}

impl<C> From<Container<C>> for El<C>
where
    C: WidgetCtx + 'static,
{
    fn from(value: Container<C>) -> Self {
        El::new(value)
    }
}
