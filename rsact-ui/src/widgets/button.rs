use super::container::Container;
use crate::widget::{prelude::*, Behavior, IdTree};

pub trait ButtonEvent {
    fn is_button_press(&self) -> bool;
    fn is_button_release(&self) -> bool;
}

#[derive(Clone, Copy)]
pub struct ButtonState {
    pressed: bool,
}

impl ButtonState {
    pub fn none() -> Self {
        Self { pressed: false }
    }
}

pub struct ButtonStyle {
    // todo
}

pub struct Button<C: WidgetCtx> {
    pub container: Container<C>,
    state: Signal<ButtonState>,
    on_click: Option<Box<dyn Fn()>>,
}

impl<C: WidgetCtx + 'static> Button<C> {
    pub fn new(content: impl IntoSignal<El<C>>) -> Self {
        let container = Container::new(content);

        container.layout.update_untracked(move |layout| {
            let container = layout.expect_container_mut();
            container.vertical_align = Align::Center;
            container.horizontal_align = Align::Center;
        });

        Self {
            container,
            state: use_signal(ButtonState::none()),
            on_click: None,
        }
    }

    pub fn on_click<F: 'static>(mut self, on_click: F) -> Self
    where
        F: Fn(),
    {
        self.on_click = Some(Box::new(on_click));
        self
    }
}

impl<C: WidgetCtx + 'static> Widget<C> for Button<C>
where
    C::Event: ButtonEvent,
{
    fn layout(&self) -> Signal<Layout> {
        self.container.layout
    }

    fn build_layout_tree(&self) -> rsact_core::signal::SignalTree<Layout> {
        self.container.build_layout_tree()
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> DrawResult {
        self.container.draw(ctx)
    }

    fn behavior(&self) -> Behavior {
        Behavior { focusable: true }
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, C>,
    ) -> EventResponse<<C as WidgetCtx>::Event> {
        let current_state = self.state.get();

        let button_event = if ctx.event.is_button_press() {
            Some(true)
        } else if ctx.event.is_button_release() {
            Some(false)
        } else {
            None
        };

        if let Some(press) = button_event {
            if let Some(on_click) = self.on_click.as_ref() {
                if !current_state.pressed && press {
                    on_click()
                }
            }

            if press != current_state.pressed {
                self.state.update(|state| state.pressed = true);
                Capture::Captured.into()
            } else {
                Propagate::Ignored.into()
            }
        } else {
            Propagate::Ignored.into()
        }
    }
}
