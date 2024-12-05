use crate::{el::ElId, widget::WidgetCtx};
use core::{fmt::Debug, ops::ControlFlow};
use embedded_graphics::prelude::Point;

pub mod message;
#[cfg(feature = "simulator")]
pub mod simulator;

#[derive(Debug, Clone, Copy)]
pub enum DevToolsEvent {
    Activate,
    Deactivate,
    /// Toggling Activate/Deactivate
    Toggle,
}

#[derive(Debug, Clone, Copy)]
pub enum MoveDir {
    Left,
    Right,
    Up,
    Down,
}

impl MoveDir {
    /// Sign of the direction on screen. Coordinates start from top left
    pub fn sign(&self) -> i32 {
        match self {
            MoveDir::Left => -1,
            MoveDir::Right => 1,
            MoveDir::Up => -1,
            MoveDir::Down => 1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MoveEvent {
    dir: MoveDir,
    delta: u16,
}

/// Event sent internally on focus move.
#[derive(Debug, Clone, Copy)]
pub enum FocusEvent {
    Focus(ElId),
    // Blur(ElId),
}

#[derive(Debug, Clone, Copy)]
pub enum PressEvent {
    Press,
    Release,
}

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    // TODO: Additional buttons
}

#[derive(Debug, Clone, Copy)]
pub enum MouseWheelDir {
    Normal,
    Flipped,
}

impl Into<i32> for MouseWheelDir {
    fn into(self) -> i32 {
        match self {
            MouseWheelDir::Normal => 1,
            MouseWheelDir::Flipped => -1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MouseEvent {
    ButtonDown(MouseButton),
    ButtonUp(MouseButton),
    MouseMove(Point),
    Wheel(Point, MouseWheelDir),
}

// TODO: Accept WidgetCtx
#[derive(Debug)]
pub enum Event<Custom = ()> {
    Move(MoveEvent),
    Focus(FocusEvent),
    Press(PressEvent),
    Exit,
    DevTools(DevToolsEvent),
    Mouse(MouseEvent),
    Custom(Custom),
}

impl<Custom> Event<Custom> {
    // Constructors //
    pub fn move_1(dir: MoveDir) -> Self {
        Self::Move(MoveEvent { dir, delta: 1 })
    }

    pub fn movement(dir: MoveDir, delta: u16) -> Self {
        Self::Move(MoveEvent { dir, delta })
    }

    // Interpretations //
    pub fn interpret_as_focus_move(&self) -> Option<i32> {
        match self {
            &Event::Move(move_event) => {
                Some(move_event.delta as i32 * move_event.dir.sign())
            },
            Event::Focus(_)
            | Event::Press(_)
            | Event::Exit
            | Event::DevTools(_) => None,
            Event::Mouse(mouse_event) => match mouse_event {
                MouseEvent::ButtonDown(_)
                | MouseEvent::ButtonUp(_)
                | MouseEvent::MouseMove(_) => None,
                MouseEvent::Wheel(point, dir) => match dir {
                    MouseWheelDir::Normal => Some(point.y as i32),
                    MouseWheelDir::Flipped => Some(-(point.y as i32)),
                },
            },
            Event::Custom(_) => None,
        }
    }

    pub fn interpret_as_rotation(&self) -> Option<i32> {
        match self {
            &Event::Move(move_event) => {
                Some(move_event.delta as i32 * move_event.dir.sign())
            },
            Event::Focus(_)
            | Event::Press(_)
            | Event::Exit
            | Event::DevTools(_) => None,
            Event::Mouse(mouse_event) => match mouse_event {
                MouseEvent::ButtonDown(_)
                | MouseEvent::ButtonUp(_)
                | MouseEvent::MouseMove(_) => None,
                // TODO: What about X movement?
                &MouseEvent::Wheel(point, dir) => {
                    Some(Into::<i32>::into(dir) * point.y)
                },
            },
            Event::Custom(_) => None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct FocusedWidget {
    pub id: ElId,
    pub absolute_position: Point,
}

pub enum UnhandledEvent<W: WidgetCtx> {
    Event(Event<W::CustomEvent>),
}

impl<W: WidgetCtx> Debug for UnhandledEvent<W> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Event(arg0) => f.debug_tuple("Event").field(arg0).finish(),
        }
    }
}

// impl<W: WidgetCtx> Debug for UnhandledEvent<W> {
//     fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//         match self {
//             Self::Event(arg0) => f.debug_tuple("Event").field(arg0).finish(),
//         }
//     }
// }

/// Info about element that captured event. Useful in such elements where child event handling affects parent behavior.
#[derive(Debug)]
pub struct CaptureData {
    /// Absolute position of the layout model of element captured the event
    pub absolute_position: Point,
}

#[derive(Debug)]
pub enum Capture {
    /// Event is captured by element and should not be handled by its parents
    Captured(CaptureData),
}

impl Into<EventResponse> for Capture {
    #[inline]
    fn into(self) -> EventResponse {
        EventResponse::Break(self)
    }
}

#[derive(Clone, Debug)]
pub enum Propagate {
    /// Event is ignored by element and can be accepted by parents
    Ignored,
    // /// Event is accepted by element and does not belongs to it logic but
    // its /// parent. For example FocusMove on focused button is captured
    // by /// button but bubbles up to its container which already moves
    // the focus to /// next children. Check source of Linear container as
    // an example of how to /// handle bubble up and why it doesn't need
    // to store any state or /// identifier of element started the bubble
    // up. BubbleUp(ElId, E),
}

impl Into<EventResponse> for Propagate {
    #[inline]
    fn into(self) -> EventResponse {
        EventResponse::Continue(self)
    }
}

pub type EventResponse = ControlFlow<Capture, Propagate>;

// TODO: Rename to InputEdge or something
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
