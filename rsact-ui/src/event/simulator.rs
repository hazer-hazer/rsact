use super::{DevToolsEvent, Event, MouseEvent, MouseWheelDir, PressEvent};
use embedded_graphics_simulator::{
    SimulatorEvent as SE,
    sdl2::{Keycode, Mod, MouseWheelDirection},
};

pub fn simulator_single_encoder(event: SE) -> Option<Event> {
    match event {
        SE::KeyUp { keycode, keymod, .. } => match keycode {
            Keycode::Return | Keycode::Space => {
                Some(Event::Press(PressEvent::Release))
            },
            Keycode::Right => Some(Event::move_1(super::MoveDir::Right)),
            Keycode::Left => Some(Event::move_1(super::MoveDir::Left)),
            Keycode::Up => Some(Event::move_1(super::MoveDir::Up)),
            Keycode::Down => Some(Event::move_1(super::MoveDir::Down)),
            Keycode::D
                if [
                    Mod::LCTRLMOD,
                    Mod::RCTRLMOD,
                    Mod::LGUIMOD,
                    Mod::RGUIMOD,
                ]
                .contains(&keymod) =>
            {
                Some(Event::DevTools(DevToolsEvent::Toggle))
            },
            _ => None,
        },
        SE::MouseButtonDown { mouse_btn, point, .. } => match mouse_btn {
            embedded_graphics_simulator::sdl2::MouseButton::Left => {
                Some(Event::Mouse(MouseEvent::ButtonDown(
                    super::MouseButton::Left,
                    Some(point.into()),
                )))
            },
            embedded_graphics_simulator::sdl2::MouseButton::Middle => {
                Some(Event::Mouse(MouseEvent::ButtonDown(
                    super::MouseButton::Middle,
                    Some(point.into()),
                )))
            },
            embedded_graphics_simulator::sdl2::MouseButton::Right => {
                Some(Event::Mouse(MouseEvent::ButtonDown(
                    super::MouseButton::Right,
                    Some(point.into()),
                )))
            },
            _ => None,
        },
        SE::MouseButtonUp { mouse_btn, point, .. } => match mouse_btn {
            embedded_graphics_simulator::sdl2::MouseButton::Left => {
                Some(Event::Mouse(MouseEvent::ButtonUp(
                    super::MouseButton::Left,
                    Some(point.into()),
                )))
            },
            embedded_graphics_simulator::sdl2::MouseButton::Middle => {
                Some(Event::Mouse(MouseEvent::ButtonUp(
                    super::MouseButton::Middle,
                    Some(point.into()),
                )))
            },
            embedded_graphics_simulator::sdl2::MouseButton::Right => {
                Some(Event::Mouse(MouseEvent::ButtonUp(
                    super::MouseButton::Right,
                    Some(point.into()),
                )))
            },
            _ => None,
        },
        SE::KeyDown { keycode, .. } => match keycode {
            Keycode::Return | Keycode::Space => {
                Some(Event::Press(PressEvent::Press))
            },
            _ => None,
        },
        SE::MouseWheel { scroll_delta, direction } => match direction {
            MouseWheelDirection::Normal => Some(Event::Mouse(
                MouseEvent::Wheel(scroll_delta.into(), MouseWheelDir::Normal),
            )),
            MouseWheelDirection::Flipped => Some(Event::Mouse(
                MouseEvent::Wheel(scroll_delta.into(), MouseWheelDir::Flipped),
            )),
            MouseWheelDirection::Unknown(_) => None,
        },
        SE::Quit => Some(Event::Exit),
        SE::MouseMove { point } => {
            Some(Event::Mouse(MouseEvent::MouseMove(point.into())))
        },
    }
}
