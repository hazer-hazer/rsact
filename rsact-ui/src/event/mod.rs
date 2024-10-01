use dev::{DevElHover, DevToolsToggle};
use embedded_graphics::prelude::Point;

use crate::el::ElId;
use crate::layout::Axis;
use crate::widgets::button::ButtonEvent;
use crate::widgets::scrollable::ScrollEvent;
use crate::widgets::slider::SliderEvent;
use core::fmt::Debug;
use core::ops::ControlFlow;

pub mod dev;

#[cfg(feature = "simulator")]
pub mod simulator;

#[derive(Clone, Debug)]
pub enum BubbledData<Custom = ()> {
    /// Focused element bubbles its absolute position so parent can react to
    /// that event, for example, by scrolling to it
    Focused(ElId, Point),
    Custom(Custom),
}

#[derive(Clone, Debug)]
pub enum Capture<E: Event> {
    /// Event is captured by element and should not be handled by its parents
    Captured,

    // TODO: Maybe here should not be event but some mapped type to allow user
    // to change the logic?
    /// BubbleUp captured by parent
    Bubble(BubbledData<E::BubbledData>),
}

impl<E: Event> Into<EventResponse<E>> for Capture<E> {
    #[inline]
    fn into(self) -> EventResponse<E> {
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

impl<E: Event> Into<EventResponse<E>> for Propagate {
    #[inline]
    fn into(self) -> EventResponse<E> {
        EventResponse::Continue(self)
    }
}

pub type EventResponse<E> = ControlFlow<Capture<E>, Propagate>;

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

pub trait Event:
    FocusEvent + ExitEvent + DevToolsToggle + DevElHover + Clone
{
    type BubbledData;
}

#[derive(Clone, Debug)]
pub struct NullEvent;

impl Event for NullEvent {
    type BubbledData = ();
}
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

impl SliderEvent for NullEvent {
    fn as_slider_move(&self, _axis: Axis) -> Option<i32> {
        None
    }
}

impl DevElHover for NullEvent {
    fn as_dev_el_hover(&self) -> Option<embedded_graphics::prelude::Point> {
        None
    }
}

impl DevToolsToggle for NullEvent {
    fn as_dev_tools_toggle(&self) -> bool {
        false
    }
}
