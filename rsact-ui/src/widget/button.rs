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

#[derive(View)]
pub struct Button<W: WidgetCtx> {
    layout: Layout,
    content: El<W>,
    style: WidgetStyleFn<ButtonStyle<W::Color>>,
    on_click: Option<Box<dyn FnMut()>>,
}

impl<W: WidgetCtx + 'static> Button<W> {
    pub fn new(content: impl View<W>) -> Self {
        let content = content.into_el();

        let layout = Layout::shrink(LayoutKind::Container(ContainerLayout {
            block_model: BlockModel::zero().padding(5).border_width(1),
            horizontal_align: Align::Center,
            vertical_align: Align::Center,
            content: content.layout(),
            font_props: Default::default(),
        }));

        Self { layout, content, style: None, on_click: None }
    }

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

impl<W: WidgetCtx + 'static> LayoutWidget<W> for Button<W> {
    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }
}
impl<W: WidgetCtx + 'static> SizedWidget<W> for Button<W> {}
impl<W: WidgetCtx + 'static> BlockModelWidget<W> for Button<W> {}
impl<W: WidgetCtx> FontSettingWidget<W> for Button<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Button<W> {
    fn debug_name(&self) -> &'static str {
        "Button"
    }

    fn flags(&self) -> WidgetFlags {
        WidgetFlags::default()
            .hoverable()
            .hoverable_from_children()
            .clickable()
            .focusable()
    }

    fn build(&mut self, mut ctx: BuildCtx<W>) {
        ctx.set_single_child(&mut self.content);
    }

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
