use crate::{el::*, page::id::PageId, style::stylist::InternalStylist};
use core::{fmt::Debug, marker::PhantomData};
use rsact_render::prelude::*;

pub trait WidgetCtx: Sized + PartialEq + Clone + 'static {
    type Renderer: Renderer<Color = Self::Color>;
    type Color: Color;
    type PageId: PageId;
    type Stylist: InternalStylist<Self::Color>;
    type CustomEvent: Debug;

    // Methods delegated from renderer //
    fn default_background() -> Self::Color {
        Self::Color::default_background()
    }

    fn default_foreground() -> Self::Color {
        Self::Color::default_foreground()
    }
}

/// WidgetTypeFamily
/// Type family of types used in Widgets
pub struct Wtf<R, I, S, E = ()>
where
    R: Renderer,
{
    _renderer: PhantomData<R>,
    _page_id: PhantomData<I>,
    _stylist: PhantomData<S>,
    _event: PhantomData<E>,
}

impl<R, I, S, E> PartialEq for Wtf<R, I, S, E>
where
    R: Renderer,
{
    fn eq(&self, other: &Self) -> bool {
        self._renderer == other._renderer
            && self._page_id == other._page_id
            && self._stylist == other._stylist
            && self._event == other._event
    }
}

impl<R, I, S, E> Clone for Wtf<R, I, S, E>
where
    R: Renderer,
{
    fn clone(&self) -> Self {
        Self {
            _renderer: self._renderer.clone(),
            _page_id: self._page_id.clone(),
            _stylist: self._stylist.clone(),
            _event: self._event.clone(),
        }
    }
}

impl<R, I, S, E> WidgetCtx for Wtf<R, I, S, E>
where
    R: Renderer + 'static,
    I: PageId + 'static,
    S: InternalStylist<R::Color> + 'static,
    E: Debug + 'static,
{
    type Renderer = R;
    type Color = <R as Renderer>::Color;
    type PageId = I;
    type Stylist = S;
    type CustomEvent = E;
}

pub struct PointerState {
    /// Last known cursor position, updated on every `MouseMove`
    pub pos: Option<Point>,
    /// Widget currently holding pointer capture (receives all pointer events
    /// until released)
    pub captured_by: Option<ElId>,
    /// The deepest `HOVERABLE` widget under the cursor as of the last
    /// `MouseMove`
    pub hovered: Option<ElId>,
}

impl PointerState {
    pub fn new() -> Self {
        Self { pos: None, captured_by: None, hovered: None }
    }
}

// TODO: Need to subscribe to arena changes, so when node is removed, it is
// removed from page state focused, pointer, etc. Otherwise we send updates to
// stale widget
pub struct PageState<W: WidgetCtx> {
    /// Element id + its absolute tree index among all focusable elements (see
    /// [`PageTree`])
    pub focused: Option<(ElId, usize)>,

    /// Page last known pointer state, updated on every `MouseMove` and is
    /// basically only needed on platforms like PC where pointer can go outside
    /// the window and we preserve last known position.
    pub pointer: PointerState,

    ctx: PhantomData<W>,
}

impl<W: WidgetCtx> PageState<W> {
    pub fn new() -> Self {
        Self { focused: None, pointer: PointerState::new(), ctx: PhantomData }
    }

    pub fn is_focused(&self, id: ElId) -> bool {
        self.focused.map(|focused| focused.0 == id).unwrap_or(false)
    }

    // pub fn is_hovered(&self, id: ElId) -> bool {
    //     self.pointer.hovered == Some(id)
    // }
}
