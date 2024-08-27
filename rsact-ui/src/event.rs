use crate::el::ElId;
use core::fmt::Debug;
use core::ops::ControlFlow;

#[derive(Clone, Debug)]
pub enum Capture {
    /// Event is captured by element and should not be handled by its parents
    Captured,
}

impl<E: Event> Into<EventResponse<E>> for Capture {
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

pub type EventResponse<E> = ControlFlow<Capture, Propagate<E>>;

#[derive(Clone, Copy, Debug)]
pub enum CommonEvent {
    /// Moves focus by current ±offset
    FocusMove(i32),
    /// Moves focus starting from back (internal usage only)
    // FocusMoveRev(i32),
    /// Focus button (e.g. enter key) is down
    FocusButtonDown,
    /// Focus button is up
    FocusButtonUp,
    /// Quit the UI. Can be captured by for example some dialog like
    /// "Are you sure you wan't to quit?"
    Exit,
}

// FIXME: Do we really need From<CommonEvent>???
pub trait Event {
    // fn is_focus_move(&self) -> Option<i32>;

    // fn is_focus_click(&self) -> bool;

    fn as_common(&self) -> Option<CommonEvent>;

    // TODO: This might better be split and moved to separate traits such as
    // `AsSelectShift`, etc. so if user don't want to use Slider for example,
    // these methods don't need to be implemented.  Or the easier way is to
    // make these methods return `None` or use `FocusMove` by default.
    fn as_select_shift(&self) -> Option<i32>;
    fn as_slider_shift(&self) -> Option<i32>;
    fn as_knob_rotation(&self) -> Option<i32>;
    fn as_input_letter_scroll(&self) -> Option<i32>;
    fn as_scroll_offset(&self) -> Option<i32>;
}

#[derive(Clone, Debug)]
pub struct NullEvent;

impl Event for NullEvent {
    fn as_common(&self) -> Option<CommonEvent> {
        None
    }

    fn as_select_shift(&self) -> Option<i32> {
        None
    }

    fn as_slider_shift(&self) -> Option<i32> {
        None
    }

    fn as_knob_rotation(&self) -> Option<i32> {
        None
    }

    fn as_input_letter_scroll(&self) -> Option<i32> {
        None
    }

    fn as_scroll_offset(&self) -> Option<i32> {
        None
    }
}

impl From<CommonEvent> for NullEvent {
    fn from(_: CommonEvent) -> Self {
        Self
    }
}
