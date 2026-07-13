use crate::{
    style::{StyleFn, WidgetStyleFn},
    widget::prelude::*,
};

declare_widget_style! {
    ContainerStyle () {
        container: container,
    }
}

// WS13.4 (Task 5.7): single child, like `Button` — `content: El<W>` is
// build-only (consumed by `ctx.set_single_child` in `Build::build`, never
// read again after `build`), so it becomes `#[child(single)]`; `layout`/
// `style` are both read by `render`/`layout`, so they stay retained
// `#[widget]` fields.
#[derive(Builder)]
#[builds(Container<W>)]
pub struct ContainerBuilder<W: WidgetCtx> {
    #[widget]
    layout: Layout,
    #[child(single)]
    content: El<W>,
    #[widget]
    style: WidgetStyleFn<ContainerStyle<W::Color>>,
}

pub struct Container<W: WidgetCtx> {
    layout: Layout,
    style: WidgetStyleFn<ContainerStyle<W::Color>>,
}

impl<W: WidgetCtx + 'static> Container<W> {
    pub fn new(content: impl View<W>) -> ContainerBuilder<W> {
        let content = content.into_el();

        let layout = Layout::shrink(LayoutKind::Container(
            ContainerLayout::base(content.layout()),
        ));

        ContainerBuilder { layout, content, style: None }
    }
}

impl<W: WidgetCtx + 'static> ContainerBuilder<W> {
    pub fn style(
        mut self,
        style: impl StyleFn<ContainerStyle<W::Color>>,
    ) -> Self {
        self.style = Some(Box::new(style));
        self
    }

    // TODO: Use MaybeReactive
    pub fn vertical_align<A: Into<Align> + PartialEq + Clone + 'static>(
        mut self,
        vertical_align: impl IntoMaybeReactive<A>,
    ) -> Self {
        self.layout_mut().setter(
            vertical_align.maybe_reactive(),
            |layout, vertical_align| {
                layout.expect_container_mut().vertical_align =
                    vertical_align.clone().into();
            },
        );
        self
    }

    pub fn horizontal_align<A: Into<Align> + PartialEq + Clone + 'static>(
        mut self,
        horizontal_align: impl IntoMaybeReactive<A>,
    ) -> Self {
        self.layout_mut().setter(
            horizontal_align.maybe_reactive(),
            |layout, horizontal_align| {
                layout.expect_container_mut().horizontal_align =
                    horizontal_align.clone().into();
            },
        );
        self
    }

    pub fn center(self) -> Self {
        self.vertical_align(Align::Center)
            .horizontal_align(Align::Center)
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

impl<W: WidgetCtx + 'static> LayoutWidget<W> for ContainerBuilder<W> {
    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }
}
impl<W: WidgetCtx + 'static> SizedWidget<W> for ContainerBuilder<W> {}
impl<W: WidgetCtx + 'static> BlockModelWidget<W> for ContainerBuilder<W> {}

impl<W: WidgetCtx> FontSettingWidget<W> for ContainerBuilder<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Container<W> {
    // NOTE: no `flags`/`debug_name` override on the retained widget — both
    // are read exactly once, pre-build, from `Build` (seeding `ElState` at
    // `state.rs:72`); post-build all consumption is via `ElState`, so an
    // override here would be dead duplication of `ContainerBuilder`'s
    // derived `Build::debug_name` ("Container" from
    // `#[builds(Container<W>)]`). `Container` never overrode `flags` either,
    // so no `#[flags(...)]` attr is needed on `ContainerBuilder`.
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

pub trait ContainerExt<W: WidgetCtx> {
    #[allow(non_snake_case)]
    fn Container(self) -> ContainerBuilder<W>;
}

impl<W: WidgetCtx, T: View<W>> ContainerExt<W> for T {
    fn Container(self) -> ContainerBuilder<W> {
        Container::new(self)
    }
}
