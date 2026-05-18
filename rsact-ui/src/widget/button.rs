use crate::{prelude::*, render::Renderable};

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
        container: container,
    }
}

impl<C: Color> ButtonStyle<C> {
    pub fn base() -> Self {
        Self {
            container: BlockStyle::base().border(
                BorderStyle::base().color(C::default_foreground()).radius(5),
            ),
        }
    }
}

pub struct Button<W: WidgetCtx> {
    layout: Layout,
    content: El<W>,
    state: Signal<ButtonState>,
    style: Option<Box<dyn Fn(ButtonStyle<W::Color>) -> ButtonStyle<W::Color>>>,
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

    pub fn style(
        mut self,
        styler: impl Fn(ButtonStyle<W::Color>) -> ButtonStyle<W::Color> + 'static,
    ) -> Self {
        self.style = Some(Box::new(styler));
        self
    }
}

impl<W: WidgetCtx + 'static> SizedWidget<W> for Button<W> {}
impl<W: WidgetCtx + 'static> BlockModelWidget<W> for Button<W> {}
impl<W: WidgetCtx> FontSettingWidget<W> for Button<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Button<W> {
    fn meta(&self, id: ElId) -> MetaTree {
        MetaTree::childless(Meta::focusable(id))
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, ctx: &mut RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self("Button", |ctx| {
            let style = ctx.get_style(|t| t.button, self.style.as_deref());

            Block::from_layout_style(
                ctx.layout.outer,
                self.layout.with(|layout| layout.block_model()),
                style.container,
            )
            .render(ctx.renderer())?;

            ctx.render_focus_outline(ctx.id)
        })?;

        ctx.render_child(&self.content)
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.handle_focusable(|ctx, pressed| {
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
