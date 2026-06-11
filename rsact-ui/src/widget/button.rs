use crate::{
    prelude::*,
    style::{StyleFn, WidgetStyleFn},
};

#[derive(Clone, Copy)]
pub struct ButtonState {
    pub pressed: bool,
}

impl ButtonState {
    pub fn none() -> Self {
        Self { pressed: false }
    }
}

declare_widget_style! {
    ButtonStyle (ButtonState) {
        // TODO: Better keep unset instead of some default values?
        container: container,
    }
}

pub struct Button<W: WidgetCtx> {
    layout: Layout,
    content: El<W>,
    state: Signal<ButtonState>,
    style: WidgetStyleFn<ButtonStyle<W::Color>>,
    on_click: Option<Box<dyn FnMut()>>,
}

impl<W: WidgetCtx + 'static> Button<W> {
    pub fn new(content: impl Into<El<W>>) -> Self {
        let content = content.into();
        let state = create_signal(ButtonState::none());

        let layout = Layout::shrink(LayoutKind::Container(ContainerLayout {
            block_model: BlockModel::zero().padding(2).border_width(1),
            horizontal_align: Align::Center,
            vertical_align: Align::Center,
            content: content.layout(),
            font_props: Default::default(),
        }));

        Self { layout, content, state, style: None, on_click: None }
    }

    // TODO: Allow function to return some value to be sent to the UI, so user can easily call ui events like goto page, etc.
    pub fn on_click<F: 'static>(mut self, on_click: F) -> Self
    where
        F: FnMut(),
    {
        self.on_click = Some(Box::new(on_click));
        self
    }

    // It's okay to replace state in builder, as it isn't used before startup
    pub fn use_state(mut self, state: Signal<ButtonState>) -> Self {
        self.state = state;
        self
    }

    pub fn style(mut self, class: impl StyleFn<ButtonStyle<W::Color>>) -> Self {
        self.style = Some(Box::new(class));
        self
    }
}

impl<W: WidgetCtx + 'static> SizedWidget<W> for Button<W> {}
impl<W: WidgetCtx + 'static> BlockModelWidget<W> for Button<W> {}
impl<W: WidgetCtx> FontSettingWidget<W> for Button<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Button<W> {
    fn debug_name(&self) -> &'static str {
        "Button"
    }

    fn build(&mut self, mut ctx: BuildCtx<W>) {
        ctx.set_single_child(&mut self.content);
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self("Button", |mut ctx| {
            let style = ctx.get_style(self.style.as_deref());

            Block::from_layout_style(
                ctx.layout.outer,
                self.layout.with(|layout| layout.block_model()),
                style.container,
            )
            .render(ctx.renderer())?;

            ctx.render_focus_outline(ctx.id)
        })
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        let _ = ctx.handle_hover_move();
        ctx.handle_focusable_or_clickable(|ctx, pressed| {
            let current_state = self.state.get();

            if current_state.pressed != pressed {
                self.state.update(|state| state.pressed = pressed);

                if let Some(on_click) = self.on_click.as_mut() {
                    if !current_state.pressed && pressed {
                        on_click();
                    }
                }

                ctx.capture()
            } else {
                ctx.ignore()
            }
        })
    }
}
