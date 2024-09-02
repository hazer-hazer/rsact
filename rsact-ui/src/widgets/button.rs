use super::container::Container;
use crate::{render::color::Color, widget::prelude::*};

pub trait ButtonEvent {
    fn as_button_press(&self) -> bool;
    fn as_button_release(&self) -> bool;
}

#[derive(Clone, Copy)]
pub struct ButtonState {
    pub pressed: bool,
    // pub focused: bool,
}

impl ButtonState {
    pub fn none() -> Self {
        Self { pressed: false }
    }
}

pub trait ButtonStyler<C: Color> {
    type Class;

    fn style(&self, class: Self::Class, state: ButtonState) -> ButtonStyle<C>;
}

#[derive(Clone, Copy, PartialEq)]
pub struct ButtonStyle<C: Color> {
    container: BoxStyle<C>,
}

impl<C: Color> ButtonStyle<C> {
    pub fn base() -> Self {
        Self {
            container: BoxStyle::base()
                .border(BorderStyle::base().color(C::default_foreground())),
        }
    }

    pub fn container(mut self, container: BoxStyle<C>) -> Self {
        self.container = container;
        self
    }
}

// fn default_styler<C: Color>(status: ButtonStatus) -> ButtonStyle<C> {
//     let base = ButtonStyle::base();
//     match status {
//         ButtonStatus { pressed: true } => base,
//         ButtonStatus { pressed: false } => base,
//     }
// }

pub struct Button<C: WidgetCtx> {
    id: ElId,
    container: Container<C>,
    state: Signal<ButtonState>,
    style: Signal<ButtonStyle<C::Color>>,
    on_click: Option<Box<dyn Fn()>>,
}

impl<C: WidgetCtx + 'static> Button<C> {
    pub fn new(content: impl IntoSignal<El<C>>) -> Self {
        let state = use_signal(ButtonState::none());
        let style = use_signal(ButtonStyle::base());

        let container = Container::new(content)
            .style(style.mapped(|style| style.container));

        container.layout.update_untracked(move |layout| {
            layout.box_model.border_width = 1;
            layout.box_model.padding = 5.into();
            let container = layout.expect_container_mut();
            container.vertical_align = Align::Center;
            container.horizontal_align = Align::Center;
        });

        Self { id: ElId::unique(), container, style, state, on_click: None }
    }

    pub fn on_click<F: 'static>(mut self, on_click: F) -> Self
    where
        F: Fn(),
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
        style: impl Fn(ButtonState) -> ButtonStyle<C::Color> + 'static,
    ) -> Self {
        self.style.set_from(self.state.mapped_clone(style));
        self
    }

    // pub fn style(
    //     self,
    //     style: impl ReadSignal<ButtonStyle<C::Color>> + 'static,
    // ) -> Self {
    //     self.style.set_from(style);
    //     self
    // }
}

impl<C: WidgetCtx + 'static> Widget<C> for Button<C>
where
    C::Event: ButtonEvent,
{
    fn children_ids(&self) -> Memo<Vec<ElId>> {
        let id = self.id;
        use_memo(move |_| vec![id])
    }

    fn layout(&self) -> Signal<Layout> {
        self.container.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        self.container.build_layout_tree()
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> DrawResult {
        ctx.draw_focus_outline(self.id)?;
        self.container.draw(ctx)
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, C>,
    ) -> EventResponse<<C as WidgetCtx>::Event> {
        ctx.handle_focusable(self.id, |pressed| {
            let current_state = self.state.get();

            // if current_state.focused != is_focused {
            //     self.state.update(|state| state.focused = is_focused);
            // }

            // if !is_focused {
            //     return Propagate::Ignored.into();
            // }

            if current_state.pressed != pressed {
                if let Some(on_click) = self.on_click.as_ref() {
                    if !current_state.pressed && pressed {
                        on_click()
                    }
                }

                self.state.update(|state| state.pressed = pressed);

                Capture::Captured.into()
            } else {
                Propagate::Ignored.into()
            }
        })
    }
}

// impl<C> From<Button<C>> for El<C>
// where
//     C::Event: ButtonEvent,
//     C: WidgetCtx + 'static,
// {
//     fn from(value: Button<C>) -> Self {
//         El::new(value)
//     }
// }
