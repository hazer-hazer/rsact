use crate::{
    prelude::*,
    style::{StyleFn, WidgetStyleFn},
};

// TODO: Add text style for dynamic styling on hover, press, etc.
declare_widget_style! {
    ButtonStyle () {
        container: container,
    }
}

#[derive(Builder)]
#[builds(Button<W>)]
#[flags(hoverable, hoverable_from_children, clickable, focusable)]
pub struct ButtonBuilder<W: WidgetCtx> {
    #[widget]
    layout: Layout,
    #[child(single)]
    content: El<W>,
    #[widget]
    style: WidgetStyleFn<ButtonStyle<W::Color>>,
    #[widget]
    on_click: Option<Box<dyn FnMut()>>,
}

pub struct Button<W: WidgetCtx> {
    layout: Layout,
    style: WidgetStyleFn<ButtonStyle<W::Color>>,
    on_click: Option<Box<dyn FnMut()>>,
}

impl<W: WidgetCtx + 'static> Button<W> {
    pub fn new(content: impl View<W>) -> ButtonBuilder<W> {
        let content = content.into_el();

        let layout = Layout::shrink(LayoutKind::Container(ContainerLayout {
            block_model: BlockModel::zero().padding(5).border_width(1),
            horizontal_align: Align::Center,
            vertical_align: Align::Center,
            font_props: Default::default(),
        }));

        ButtonBuilder { layout, content, style: None, on_click: None }
    }
}

impl<W: WidgetCtx + 'static> ButtonBuilder<W> {
    // TODO: Allow function to return some value to be sent to the UI, so user
    // can easily call ui events like goto page, etc without getting access to the [`MessageQueue`].
    pub fn on_click<F: 'static>(mut self, on_click: F) -> Self
    where
        F: FnMut(),
    {
        self.on_click = Some(Box::new(on_click));
        self
    }

    // TODO: Do we need to support such logic?
    // This would allow us to have similar to what JS provides where we can acquire some element and dispatch events on it. Without this, users won't be able to trigger events on a button programmatically. But I am not sure if it is a first-tier requirement for a UI framework -\_(*_*)_/-
    // // It's okay to replace state in builder, as it isn't used before startup
    // pub fn use_state(mut self, state: Signal<ButtonState>) -> Self {
    //     self.state = state;
    //     self
    // }

    pub fn style(mut self, class: impl StyleFn<ButtonStyle<W::Color>>) -> Self {
        self.style = Some(Box::new(class));
        self
    }
}

impl<W: WidgetCtx + 'static> LayoutWidget<W> for ButtonBuilder<W> {
    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }
}
impl<W: WidgetCtx + 'static> SizedWidget<W> for ButtonBuilder<W> {}
impl<W: WidgetCtx + 'static> BlockModelWidget<W> for ButtonBuilder<W> {}
impl<W: WidgetCtx + 'static> FontSettingWidget<W> for ButtonBuilder<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Button<W> {
    // NOTE: no `flags`/`debug_name` override on the retained widget — both are
    // read exactly once, pre-build, from `Build` (seeding `ElState` at
    // `state.rs:72`); post-build all consumption is via `ElState`, so an
    // override here would be dead duplication of the builder's `Build::flags`.
    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self(|mut ctx| {
            let style = ctx.get_style(self.style.as_deref());

            Block::from_layout_style(
                ctx.layout.outer,
                self.layout.with(|layout| layout.block_model()),
                style.container,
            )
            .render(ctx.renderer)?;

            ctx.render_focus_outline(ctx.id)
        })
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.handle()?; // hover + press claim + pointer capture (automatic)
        ctx.handle_click(|ctx| {
            if let Some(on_click) = self.on_click.as_mut() {
                on_click();
            }
            ctx.capture()
        })
    }
}
