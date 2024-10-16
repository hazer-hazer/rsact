use crate::prelude::*;

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
    content: Signal<El<W>>,
    state: Signal<ButtonState>,
    style: MemoChain<ButtonStyle<W::Color>>,
    on_click: Option<Box<dyn Fn() -> Option<Message<W>>>>,
}

impl<W: WidgetCtx + 'static> Button<W> {
    pub fn new(content: impl IntoSignal<El<W>>) -> Self {
        let content = content.into_signal();
        let state = use_signal(ButtonState::none());

        let layout = Layout::shrink(LayoutKind::Container(ContainerLayout {
            block_model: BlockModel::zero().border_width(1).padding(5),
            horizontal_align: Align::Center,
            vertical_align: Align::Center,
            content_size: content.mapped(|content| {
                content.layout().with(|layout| layout.content_size())
            }),
        }))
        .into_signal();

        Self {
            id: ElId::unique(),
            layout,
            content,
            state,
            style: use_memo_chain(|_| ButtonStyle::base()),
            on_click: None,
        }
    }

    pub fn on_click<F: 'static>(mut self, on_click: F) -> Self
    where
        F: Fn() -> Option<Message<W>>,
    {
        self.on_click = Some(Box::new(on_click));
        self
    }

    pub fn use_state(
        self,
        state: impl WriteSignal<ButtonState> + 'static,
    ) -> Self {
        state.set_from(self.state);
        self
    }

    pub fn style(
        self,
        styler: impl Fn(ButtonStyle<W::Color>, ButtonState) -> ButtonStyle<W::Color>
            + 'static,
    ) -> Self {
        let state = self.state;
        self.style.last(move |base| styler(*base, state.get()));
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
        MetaTree::childless(Meta::focusable(self.id))
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);

        ctx.pass_to_child(self.content);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        MemoTree {
            data: self.layout.into_memo(),
            children: self
                .content
                .mapped(|content| vec![content.build_layout_tree()]),
        }
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        ctx.draw_focus_outline(self.id)?;

        let style = self.style.get();

        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.outer,
            self.layout.get().block_model(),
            style.container,
        ))?;

        self.content.with(|content| ctx.draw_child(content))
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse<W> {
        ctx.handle_focusable(self.id, |pressed| {
            let current_state = self.state.get();

            if current_state.pressed != pressed {
                self.state.update(|state| state.pressed = pressed);

                if let Some(on_click) = self.on_click.as_ref() {
                    if !current_state.pressed && pressed {
                        let message = on_click();
                        if let Some(message) = message {
                            return W::bubble(BubbledData::Message(message));
                        }
                    }
                }

                W::capture()
            } else {
                W::ignore()
            }
        })
    }
}
