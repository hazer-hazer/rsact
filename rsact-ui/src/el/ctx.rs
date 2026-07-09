use crate::{el::*, page::id::PageId, style::stylist::InternalStylist};
use core::{fmt::Debug, marker::PhantomData};
use rsact_render::prelude::*;

pub trait WidgetCtx: Sized + PartialEq + Clone + 'static {
    type Renderer: Renderer<Color = Self::Color>;
    type Color: Color;
    type PageId: PageId;
    // WS4.1: `Clone` because the stylist is stored inline in `Inert` now (not a
    // shared runtime-node handle), so the UI must clone it into each page it
    // builds. All concrete stylists (`()`, `Theme<C>`, `BinaryTheme`) are `Copy`.
    type Stylist: InternalStylist<Self::Color> + Clone;
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
    S: InternalStylist<R::Color> + Clone + 'static,
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
    /// The `CLICKABLE` widget currently held down by the mouse button, i.e. the
    /// widget that received `ButtonDown` and has not yet seen the matching
    /// `ButtonUp`. Claimed by the deepest clickable widget under the cursor and
    /// cleared on `ButtonUp` regardless of cursor position. Single-source-of-
    /// truth for the mouse "pressed" pseudo-class (mirrors `hovered`).
    pub pressed: Option<ElId>,
}

impl PointerState {
    pub fn new() -> Self {
        Self { pos: None, captured_by: None, hovered: None, pressed: None }
    }
}

// TODO: Need to subscribe to arena changes, so when node is removed, it is
// removed from page state focused, pointer, etc. Otherwise we send updates to
// stale widget
pub struct PageState<W: WidgetCtx> {
    /// Element id + its absolute tree index among all focusable elements (see
    /// [`PageTree`])
    pub focused: Option<(ElId, usize)>,

    /// The focused widget's activation button (encoder/keyboard) is currently
    /// held down. There is only one focused widget, so the pressed widget is
    /// `focused.0`. This is the focus-driven analogue of `pointer.pressed` and
    /// feeds the same "pressed" pseudo-class.
    pub focus_pressed: bool,

    /// Page last known pointer state, updated on every `MouseMove` and is
    /// basically only needed on platforms like PC where pointer can go outside
    /// the window and we preserve last known position.
    pub pointer: PointerState,

    ctx: PhantomData<W>,
}

impl<W: WidgetCtx> PageState<W> {
    pub fn new() -> Self {
        Self {
            focused: None,
            focus_pressed: false,
            pointer: PointerState::new(),
            ctx: PhantomData,
        }
    }

    pub fn is_focused(&self, id: ElId) -> bool {
        self.focused.map(|focused| focused.0 == id).unwrap_or(false)
    }

    /// Drop every reference to an element id that no longer exists in `arena`
    /// (WS3.3, addressing the TODO above). A removed element — subtree replace,
    /// `Dynamic` rebuild, list update, navigation — must never receive focus or
    /// pointer routing; otherwise an event is dispatched to a stale id (D2-F5),
    /// e.g. the `captured_by` fast-path in `Page::send_event` would deliver a
    /// mouse event to a freed widget. Called before event routing, so it is
    /// lazy (validate-on-use) rather than eager (the arena mutation that removes
    /// a node happens deep inside a reactive flush with no `PageState` in hand).
    pub fn retain_existing(&mut self, arena: &crate::el::arena::ElArena<W>) {
        if let Some((id, _)) = self.focused
            && !arena.contains(id)
        {
            self.focused = None;
            // `focus_pressed` tracks the focused widget's button; with no focus
            // there is nothing pressed.
            self.focus_pressed = false;
        }
        if let Some(id) = self.pointer.captured_by
            && !arena.contains(id)
        {
            self.pointer.captured_by = None;
        }
        if let Some(id) = self.pointer.hovered
            && !arena.contains(id)
        {
            self.pointer.hovered = None;
        }
        if let Some(id) = self.pointer.pressed
            && !arena.contains(id)
        {
            self.pointer.pressed = None;
        }
    }

    // pub fn is_hovered(&self, id: ElId) -> bool {
    //     self.pointer.hovered == Some(id)
    // }
}
