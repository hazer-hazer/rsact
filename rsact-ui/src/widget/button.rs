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
    id: ElId,
    layout: Signal<Layout>,
    content: El<W>,
    state: Signal<ButtonState>,
    style: MemoChain<ButtonStyle<W::Color>>,
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
            content: content.layout().memo(),
            font_props: Default::default(),
        }))
        .signal();

        Self {
            id: ElId::unique(),
            layout,
            content,
            state,
            style: ButtonStyle::base().memo_chain(),
            on_click: None,
        }
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
        self,
        styler: impl Fn(ButtonStyle<W::Color>, ButtonState) -> ButtonStyle<W::Color>
        + 'static,
    ) -> Self {
        let state = self.state;
        self.style.last(move |base| styler(*base, state.get())).unwrap();
        self
    }
}

impl<W: WidgetCtx + 'static> SizedWidget<W> for Button<W> where
    W::Styler: WidgetStylist<ButtonStyle<W::Color>>
{
}

impl<W: WidgetCtx + 'static> BlockModelWidget<W> for Button<W> where
    W::Styler: WidgetStylist<ButtonStyle<W::Color>>
{
}

impl<W: WidgetCtx> FontSettingWidget<W> for Button<W> where
    W::Styler: WidgetStylist<ButtonStyle<W::Color>>
{
}

impl<W: WidgetCtx + 'static> Widget<W> for Button<W>
where
    W::Styler: WidgetStylist<ButtonStyle<W::Color>>,
{
    fn meta(&self) -> MetaTree {
        let id = self.id;
        MetaTree::childless(move || Meta::focusable(id))
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);
        ctx.pass_to_child(self.layout, &mut self.content);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn render(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        let style = self.style.get();

        Block::from_layout_style(
            ctx.layout.outer,
            self.layout.with(|layout| layout.block_model()),
            style.container,
        )
        .render(ctx.renderer)?;

        ctx.render_child(&self.content)?;

        ctx.render_focus_outline(self.id)
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse {
        ctx.handle_focusable(self.id, |ctx, pressed| {
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
