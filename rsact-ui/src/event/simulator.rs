use super::{
    dev::{DevElHover, DevToolsToggle},
    Event, ExitEvent, FocusEvent,
};
use crate::widget::{
    button::ButtonEvent, knob::KnobEvent, scrollable::ScrollEvent,
    select::SelectEvent, slider::SliderEvent,
};
use embedded_graphics::prelude::Point;
use embedded_graphics_simulator::{
    sdl2::{Keycode, Mod, MouseWheelDirection},
    SimulatorEvent as SE,
};

#[derive(Clone, Debug)]
pub enum SimulatorEvent {
    FocusMove(i32),
    FocusedPress,
    FocusedRelease,
    MouseMove(Point),
    ToggleDevTools,
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
    fn zero() -> Self {
        Self::FocusMove(0)
    }

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

impl SliderEvent for SimulatorEvent {
    fn as_slider_move(&self, _axis: crate::layout::Axis) -> Option<i32> {
        self.as_focus_move()
    }
}

impl SelectEvent for SimulatorEvent {
    fn as_select(&self, _axis: crate::layout::Axis) -> Option<i32> {
        self.as_focus_move()
    }
}

impl KnobEvent for SimulatorEvent {
    fn as_knob_rotate(&self) -> Option<i32> {
        self.as_focus_move()
        // .map(core::ops::Neg::neg)
    }
}

impl Event for SimulatorEvent {
    type BubbledData = ();
}

pub fn simulator_single_encoder(event: SE) -> Option<SimulatorEvent> {
    match event {
        SE::KeyUp { keycode, keymod, .. } => match keycode {
            Keycode::Return | Keycode::Space => {
                Some(SimulatorEvent::FocusedRelease)
            },
            Keycode::Right | Keycode::Down => {
                Some(SimulatorEvent::FocusMove(1))
            },
            Keycode::Left | Keycode::Up => Some(SimulatorEvent::FocusMove(-1)),
            Keycode::D
                if keymod == Mod::LSHIFTMOD || keymod == Mod::RSHIFTMOD =>
            {
                Some(SimulatorEvent::ToggleDevTools)
            },
            _ => None,
        },
        SE::MouseButtonDown { mouse_btn, .. } => match mouse_btn {
            embedded_graphics_simulator::sdl2::MouseButton::Left => {
                Some(SimulatorEvent::FocusedPress)
            },
            _ => None,
        },
        SE::MouseButtonUp { mouse_btn, .. } => match mouse_btn {
            embedded_graphics_simulator::sdl2::MouseButton::Left => {
                Some(SimulatorEvent::FocusedRelease)
            },
            _ => None,
        },
        SE::KeyDown { keycode, .. } => match keycode {
            Keycode::Return | Keycode::Space => {
                Some(SimulatorEvent::FocusedPress)
            },
            _ => None,
        },
        SE::MouseWheel { scroll_delta, direction } => {
            Some(SimulatorEvent::FocusMove(
                match direction {
                    MouseWheelDirection::Normal => 1,
                    MouseWheelDirection::Flipped => -1,
                    MouseWheelDirection::Unknown(_) => 0,
                } * scroll_delta.y,
            ))
        },
        SE::MouseMove { point } => Some(SimulatorEvent::MouseMove(point)),
        SE::Quit => Some(SimulatorEvent::Exit),
    }
}

impl DevElHover for SimulatorEvent {
    fn as_dev_el_hover(&self) -> Option<embedded_graphics::prelude::Point> {
        match self {
            &SimulatorEvent::MouseMove(point) => Some(point),
            _ => None,
        }
    }
}

impl DevToolsToggle for SimulatorEvent {
    fn as_dev_tools_toggle(&self) -> bool {
        match self {
            SimulatorEvent::ToggleDevTools => true,
            _ => false,
        }
    }
}
