use crate::el::ElId;
use crate::layout::Axis;
use crate::widgets::button::ButtonEvent;
use crate::widgets::scrollable::ScrollEvent;
use core::fmt::Debug;
use core::ops::ControlFlow;

#[derive(Clone, Debug)]
pub enum Capture<E: Event> {
    /// Event is captured by element and should not be handled by its parents
    Captured,

    // TODO: Maybe here should not be event but some mapped type to allow user
    // to change the logic?
    /// BubbleUp captured by parent
    Bubbled(ElId, E),
}

impl<E: Event> Into<EventResponse<E>> for Capture<E> {
    #[inline]
    fn into(self) -> EventResponse<E> {
        EventResponse::Break(self)
    }
}

#[derive(Clone, Debug)]
pub enum Propagate<E: Event> {
    /// Event is ignored by element and can be accepted by parents
    Ignored,
    /// Event is accepted by element and does not belongs to it logic but its
    /// parent. For example FocusMove on focused button is captured by
    /// button but bubbles up to its container which already moves the focus to
    /// next children. Check source of Linear container as an example of how to
    /// handle bubble up and why it doesn't need to store any state or
    /// identifier of element started the bubble up.
    BubbleUp(ElId, E),
}

impl<E: Event> Into<EventResponse<E>> for Propagate<E> {
    #[inline]
    fn into(self) -> EventResponse<E> {
        EventResponse::Continue(self)
    }
}

pub type EventResponse<E> = ControlFlow<Capture<E>, Propagate<E>>;

#[derive(Clone, Copy)]
pub enum ButtonEdge {
    None,
    Rising,
    Falling,
}

impl ButtonEdge {
    pub fn new(from: bool, to: bool) -> Self {
        match (from, to) {
            (true, false) => Self::Falling,
            (false, true) => Self::Rising,
            (true, true) | (false, false) => Self::None,
        }
    }
}

pub trait FocusEvent {
    fn as_focus_move(&self) -> Option<i32>;
    fn as_focus_press(&self) -> bool;
    fn as_focus_release(&self) -> bool;
}

pub trait ExitEvent {
    fn as_exit(&self) -> bool;
}

// FIXME: Do we really need From<CommonEvent>???
pub trait Event: FocusEvent + ExitEvent + Clone {}

#[derive(Clone, Debug)]
pub struct NullEvent;

impl Event for NullEvent {}
impl ButtonEvent for NullEvent {
    fn as_button_press(&self) -> bool {
        false
    }

    fn as_button_release(&self) -> bool {
        false
    }
}

impl ExitEvent for NullEvent {
    fn as_exit(&self) -> bool {
        false
    }
}

impl FocusEvent for NullEvent {
    fn as_focus_move(&self) -> Option<i32> {
        None
    }

    fn as_focus_press(&self) -> bool {
        false
    }

    fn as_focus_release(&self) -> bool {
        false
    }
}

impl ScrollEvent for NullEvent {
    fn as_scroll(&self, _axis: Axis) -> Option<i32> {
        None
    }
}

#[cfg(feature = "simulator")]
pub mod simulator {
    use crate::widgets::{button::ButtonEvent, scrollable::ScrollEvent};

    use super::{Event, ExitEvent, FocusEvent};

    #[derive(Clone, Debug)]
    pub enum SimulatorEvent {
        FocusMove(i32),
        FocusedPress,
        FocusedRelease,
        Exit,
    }

    impl ButtonEvent for SimulatorEvent {
        fn as_button_press(&self) -> bool {
            self.as_focus_press()
        }

        fn as_button_release(&self) -> bool {
            self.as_focus_release()
        }
    }

    impl FocusEvent for SimulatorEvent {
        fn as_focus_move(&self) -> Option<i32> {
            match self {
                &SimulatorEvent::FocusMove(offset) => Some(offset),
                _ => None,
            }
        }

        fn as_focus_press(&self) -> bool {
            match self {
                SimulatorEvent::FocusedPress => true,
                _ => false,
            }
        }

        fn as_focus_release(&self) -> bool {
            match self {
                SimulatorEvent::FocusedRelease => true,
                _ => false,
            }
        }
    }

    impl ExitEvent for SimulatorEvent {
        fn as_exit(&self) -> bool {
            match self {
                SimulatorEvent::Exit => true,
                _ => false,
            }
        }
    }

    impl ScrollEvent for SimulatorEvent {
        // Encoder
        fn as_scroll(&self, _axis: crate::layout::Axis) -> Option<i32> {
            self.as_focus_move().map(|offset| offset * 5)
        }
    }

    impl Event for SimulatorEvent {}

    pub fn simulator_single_encoder(
        event: embedded_graphics_simulator::SimulatorEvent,
    ) -> Option<SimulatorEvent> {
        match event {
            embedded_graphics_simulator::SimulatorEvent::KeyUp {
                keycode,
                ..
            } => match keycode {
                embedded_graphics_simulator::sdl2::Keycode::Return
                | embedded_graphics_simulator::sdl2::Keycode::Space => {
                    Some(SimulatorEvent::FocusedRelease)
                },
                embedded_graphics_simulator::sdl2::Keycode::Right
                | embedded_graphics_simulator::sdl2::Keycode::Down => {
                    Some(SimulatorEvent::FocusMove(1))
                },
                embedded_graphics_simulator::sdl2::Keycode::Left
                | embedded_graphics_simulator::sdl2::Keycode::Up => {
                    Some(SimulatorEvent::FocusMove(-1))
                },
                _ => None,
            },
            embedded_graphics_simulator::SimulatorEvent::KeyDown {
                keycode,
                ..
            } => match keycode {
                embedded_graphics_simulator::sdl2::Keycode::Return
                | embedded_graphics_simulator::sdl2::Keycode::Space => {
                    Some(SimulatorEvent::FocusedPress)
                },
                _ => None,
            },
            embedded_graphics_simulator::SimulatorEvent::MouseButtonUp {
                mouse_btn,
                ..
            } => match mouse_btn {
                embedded_graphics_simulator::sdl2::MouseButton::Left => Some(SimulatorEvent::FocusedRelease),
                _ => None
            },
            embedded_graphics_simulator::SimulatorEvent::MouseButtonDown {
                mouse_btn,
                ..
            } => match mouse_btn {
                embedded_graphics_simulator::sdl2::MouseButton::Left => Some(SimulatorEvent::FocusedPress),
                _ => None
            },
            embedded_graphics_simulator::SimulatorEvent::MouseWheel {
                scroll_delta,
                direction,
            } => Some(SimulatorEvent::FocusMove(match direction {
                embedded_graphics_simulator::sdl2::MouseWheelDirection::Normal => 1,
                embedded_graphics_simulator::sdl2::MouseWheelDirection::Flipped => -1,
                embedded_graphics_simulator::sdl2::MouseWheelDirection::Unknown(_) => 0,
            } * scroll_delta.y)),
            embedded_graphics_simulator::SimulatorEvent::MouseMove {
                ..
            } => None,
            embedded_graphics_simulator::SimulatorEvent::Quit => {
                Some(SimulatorEvent::Exit)
            },
        }
    }
}
