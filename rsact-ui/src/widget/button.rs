use crate::{prelude::*, render::Renderable};

pub trait ButtonEvent {
    fn as_button_press(&self) -> bool;
    fn as_button_release(&self) -> bool;
}

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
            content_size: content
                .layout()
                .map(|layout| layout.content_size())
                .into(),
        }))
        .signal();

        Self {
            id: ElId::unique(),
            layout,
            content,
            state,
            style: create_memo_chain(|_| ButtonStyle::base()),
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

impl<W: WidgetCtx + 'static> SizedWidget<W> for Button<W>
where
    W::Event: ButtonEvent,
    W::Styler: Styler<ButtonStyle<W::Color>, Class = ()>,
{
}

impl<W: WidgetCtx + 'static> BlockModelWidget<W> for Button<W>
where
    W::Event: ButtonEvent,
    W::Styler: Styler<ButtonStyle<W::Color>, Class = ()>,
{
}

impl<W: WidgetCtx + 'static> Widget<W> for Button<W>
where
    W::Event: ButtonEvent,
    W::Styler: Styler<ButtonStyle<W::Color>, Class = ()>,
{
    fn meta(&self) -> MetaTree {
        let id = self.id;
        MetaTree::childless(move || Meta::focusable(id))
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);

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

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        let style = self.style.get();

        Block::from_layout_style(
            ctx.layout.outer,
            self.layout.with(|layout| layout.block_model()),
            style.container,
        )
        .render(ctx.renderer)?;

        ctx.draw_child(&self.content)?;

        ctx.draw_focus_outline(self.id)
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse<W> {
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
