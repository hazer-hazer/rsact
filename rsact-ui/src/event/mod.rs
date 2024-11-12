use crate::{
    el::ElId,
    layout::Axis,
    widget::{
        button::ButtonEvent, scrollable::ScrollEvent, slider::SliderEvent,
        WidgetCtx,
    },
};
use core::{fmt::Debug, ops::ControlFlow};
use dev::{DevElHover, DevToolsToggle};
use embedded_graphics::prelude::Point;

pub mod dev;
pub mod message;
#[cfg(feature = "simulator")]
pub mod simulator;

#[derive(Clone, Copy)]
pub struct FocusedWidget {
    pub id: ElId,
    pub absolute_position: Point,
}

pub struct EventPass {
    // /// Count of focusable elements in the tree
    // pub focusable: usize,
    /// Absolute element index to focus
    pub focus_search: Option<usize>,

    focused: Option<FocusedWidget>,
}

impl EventPass {
    pub fn new(focus_target: Option<usize>) -> Self {
        Self { focus_search: focus_target, focused: None }
    }

    pub fn set_focused(&mut self, focused: FocusedWidget) {
        self.focused = Some(focused);
        self.focus_search = None;
    }

    pub fn focused(&self) -> Option<FocusedWidget> {
        self.focused
    }
}

pub enum UnhandledEvent<W: WidgetCtx> {
    Event(W::Event),
    Bubbled(BubbledData<W>),
}

impl<W: WidgetCtx> Debug for UnhandledEvent<W> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Event(arg0) => f.debug_tuple("Event").field(arg0).finish(),
            Self::Bubbled(arg0) => {
                f.debug_tuple("Bubbled").field(arg0).finish()
            },
        }
    }
}

#[derive(Clone)]
pub enum BubbledData<W: WidgetCtx> {
    // // /// Focused element bubbles its absolute position so parent can react
    // // to /// that event, for example, by scrolling to it
    // // Focused(ElId, Point),
    // Message(Message<W>),
    Custom(<W::Event as Event>::BubbledData),
}

impl<W: WidgetCtx> Debug for BubbledData<W> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Custom(custom) => {
                f.debug_tuple("Custom").field(custom).finish()
            },
        }
    }
}

#[derive(Debug)]
pub enum Capture<W: WidgetCtx> {
    /// Event is captured by element and should not be handled by its parents
    Captured,
    /// BubbleUp captured by parent
    Bubble(BubbledData<W>),
}

impl<W: WidgetCtx> Into<EventResponse<W>> for Capture<W> {
    #[inline]
    fn into(self) -> EventResponse<W> {
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

impl<W: WidgetCtx> Into<EventResponse<W>> for Propagate {
    #[inline]
    fn into(self) -> EventResponse<W> {
        EventResponse::Continue(self)
    }
}

pub type EventResponse<W> = ControlFlow<Capture<W>, Propagate>;

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
    fn zero() -> Self;
    fn as_focus_move(&self) -> Option<i32>;
    fn as_focus_press(&self) -> bool;
    fn as_focus_release(&self) -> bool;
}

pub trait ExitEvent {
    fn as_exit(&self) -> bool;
}

pub trait Event:
    FocusEvent + ExitEvent + DevToolsToggle + DevElHover + Debug + Clone
{
    /// User-defined bubbled data found in event responses
    type BubbledData: Clone + Debug;
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
    fn zero() -> Self {
        Self
    }

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
