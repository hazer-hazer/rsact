use crate::{el::*, page::id::PageId};
use core::{fmt::Debug, marker::PhantomData};
use rsact_render::prelude::*;

// TODO: Not an actual context, rename to something like `WidgetTypeFamily`
pub trait WidgetCtx: Sized + PartialEq + Clone + 'static {
    type Renderer: Renderer<Color = Self::Color>;
    type Color: Color;
    type PageId: PageId;
    type CustomEvent: Debug;

    // Methods delegated from renderer //
    fn default_background() -> Self::Color {
        Self::Color::default_background()
    }

    fn default_foreground() -> Self::Color {
        Self::Color::default_foreground()
    }
}

// TODO: This is a pure WidgetCtx, but for most users we want such WTF that constraints over all stylists and events for all native widgets. Is it possible to create such keeping UI implementation untouched?
/// WidgetTypeFamily
/// Type family of types used in Widgets
pub struct Wtf<R, I, E = ()>
where
    R: Renderer,
{
    _renderer: PhantomData<R>,
    _page_id: PhantomData<I>,
    _event: PhantomData<E>,
}

impl<R, I, E> PartialEq for Wtf<R, I, E>
where
    R: Renderer,
{
    fn eq(&self, other: &Self) -> bool {
        self._renderer == other._renderer
            && self._page_id == other._page_id
            && self._event == other._event
    }
}

impl<R, I, E> Clone for Wtf<R, I, E>
where
    R: Renderer,
{
    fn clone(&self) -> Self {
        Self {
            _renderer: self._renderer.clone(),
            _page_id: self._page_id.clone(),
            _event: self._event.clone(),
        }
    }
}

impl<R, I, E> WidgetCtx for Wtf<R, I, E>
where
    R: Renderer + 'static,
    I: PageId + 'static,
    E: Debug + 'static,
{
    type Renderer = R;
    type Color = <R as Renderer>::Color;
    type PageId = I;
    type CustomEvent = E;
}

pub struct PointerState {
    /// Last known cursor position, updated on every `MouseMove`
    pub pos: Option<Point>,
    /// Widget currently holding pointer capture (receives all pointer events until released)
    pub captured_by: Option<ElId>,
    /// The deepest `HOVERABLE` widget under the cursor as of the last `MouseMove`
    pub hovered: Option<ElId>,
}

impl PointerState {
    pub fn new() -> Self {
        Self { pos: None, captured_by: None, hovered: None }
    }
}

pub struct PageState<W: WidgetCtx> {
    /// Element id + its absolute tree index among all focusable elements (see [`PageTree`])
    pub focused: Option<(ElId, usize)>,

    /// Page last known pointer state, updated on every `MouseMove` and is basically only needed on platforms like PC where pointer can go outside the window and we preserve last known position.
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

    pub fn is_hovered(&self, id: ElId) -> bool {
        self.pointer.hovered == Some(id)
    }
}
