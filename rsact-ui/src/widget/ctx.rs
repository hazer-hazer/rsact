use super::Widget;
use crate::{
    el::{El, ElId, WithElId},
    event::{
        Capture, CaptureData, Event, EventResponse, FocusEvent, MouseButton,
        MouseEvent, Propagate,
    },
    font::{Font, FontCtx, FontProps, ResolvedFontProps},
    layout::model::LayoutModelNode,
    page::{PageStyle, id::PageId},
    render::prelude::*,
    style::{TreeStyle, theme::Theme},
};
use alloc::vec::Vec;
use core::{
    fmt::{Debug, Display},
    hash::Hash,
    marker::PhantomData,
};
use itertools::Itertools as _;
use log::debug;
use rsact_reactive::{
    prelude::*, runtime::get_observer, signal::marker::ReadOnly,
};
use rsact_render::color::ACCENT_COUNT;

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

pub struct CtxReady;
pub struct CtxUnready;

pub struct RenderSelf;

pub trait GetStyle<W> {
    type Style;

    fn style(&self) -> Self::Style;
}

// TODO: Make RenderCtx a delegate to renderer so u can do `Primitive::(...).render(ctx)`
pub struct RenderCtx<'a, W: WidgetCtx, S = CtxUnready> {
    pub id: ElId,
    pub state: Signal<PageState<W>, ReadOnly>,
    pub renderer: &'a mut W::Renderer,
    pub layout: &'a LayoutModelNode<'a>,
    pub tree_style: TreeStyle<W::Color>,
    pub page_style: Signal<PageStyle<W::Color>, ReadOnly>,
    pub viewport: MaybeReactive<Size>,
    pub fonts: Signal<FontCtx, ReadOnly>,
    pub theme: Inert<Theme<W::Color>>,
    pub font_props: FontProps,
    pub force_redraw: Signal<bool>,
    pub parent_dirty: bool,

    // Nesting level only used for redraw debugging, making different colors on each call to distinguish between elements.
    pub nesting_level: usize,

    pub call: usize,

    pub ctx_state: PhantomData<S>,
}

impl<'a, W: WidgetCtx> RenderCtx<'a, W, RenderSelf> {
    pub fn renderer(&mut self) -> &mut W::Renderer {
        self.renderer
    }

    #[must_use]
    pub fn render_font(
        &mut self,
        font: Font,
        content: &str,
        props: ResolvedFontProps,
        bounds: Rect,
        color: W::Color,
    ) -> RenderResult {
        self.fonts.with(|fonts| {
            fonts.render::<W>(
                font,
                content,
                props,
                bounds,
                color,
                self.renderer,
            )
        })
    }

    #[must_use]
    pub fn render_focus_outline(&mut self, id: ElId) -> RenderResult {
        if self.state.with(|state| state.is_focused(id)) {
            (Block {
                border: Border::zero()
                    // TODO: Theme focus color
                    .color(Some(<W::Color as Color>::accents()[1]))
                    .width(1),
                rect: self.layout.outer,
                background: None,
            })
            .render(self.renderer)
        } else {
            Ok(())
        }
    }
}

impl<'a, W: WidgetCtx, S> RenderCtx<'a, W, S> {
    #[must_use]
    pub fn for_child<R>(
        &mut self,
        id: ElId,
        child_layout: &LayoutModelNode,
        f: impl FnOnce(RenderCtx<'_, W, CtxUnready>) -> R,
    ) -> R {
        let font_props = child_layout.font_props().unwrap_or(self.font_props);
        f(RenderCtx {
            id,
            state: self.state,
            renderer: self.renderer,
            layout: child_layout,
            tree_style: self.tree_style,
            page_style: self.page_style,
            viewport: self.viewport,
            fonts: self.fonts,
            theme: self.theme,
            font_props,
            force_redraw: self.force_redraw,
            parent_dirty: self.parent_dirty,
            nesting_level: self.nesting_level + 1,
            call: self.call,
            ctx_state: PhantomData,
        })
    }

    #[must_use]
    pub fn with_tree_style<R>(
        &mut self,
        tree_style: impl FnOnce(TreeStyle<W::Color>) -> TreeStyle<W::Color>,
        f: impl FnOnce(RenderCtx<'_, W, S>) -> R,
    ) -> R {
        f(RenderCtx {
            id: self.id,
            state: self.state,
            renderer: self.renderer,
            layout: self.layout,
            tree_style: tree_style(self.tree_style),
            page_style: self.page_style,
            viewport: self.viewport,
            fonts: self.fonts,
            theme: self.theme,
            font_props: self.font_props,
            force_redraw: self.force_redraw,
            parent_dirty: self.parent_dirty,
            nesting_level: self.nesting_level,
            call: self.call,
            ctx_state: PhantomData,
        })
    }

    pub fn clip_inner(
        &mut self,
        f: impl FnOnce(RenderCtx<'_, W, S>) -> RenderResult,
    ) -> RenderResult {
        self.renderer.clipped(self.layout.inner, |renderer| {
            f(RenderCtx {
                id: self.id,
                state: self.state,
                renderer,
                layout: self.layout,
                tree_style: self.tree_style,
                page_style: self.page_style,
                viewport: self.viewport,
                fonts: self.fonts,
                theme: self.theme,
                font_props: self.font_props,
                force_redraw: self.force_redraw,
                parent_dirty: self.parent_dirty,
                nesting_level: self.nesting_level + 1,
                call: self.call,
                ctx_state: PhantomData,
            })
        })
    }

    pub fn get_style<Style: Copy>(
        &self,
        base: impl FnOnce(&Theme<W::Color>) -> Style,
        style: Option<&dyn Fn(Style) -> Style>,
    ) -> Style {
        let base = self.theme.with(base);
        style.map(|f| f(base)).unwrap_or(base)
    }
}

impl<'a, W: WidgetCtx + 'static> RenderCtx<'a, W, CtxUnready> {
    /// Render part of the widget that is dependent on some reactive state.
    // Note: Display is required for logs, but as for now, all render_part calls are used with a string to be hashed, so we either require it to always be a string or keep it so, idk.
    pub fn render_part<H: Display + Hash + Copy>(
        &mut self,
        hash_source: H,
        f: impl FnOnce(RenderCtx<'_, W, RenderSelf>) -> RenderResult,
    ) -> RenderResult {
        let render_id = WithElId::new(self.id, hash_source);

        // If the parent already cleared the area, force this child observer
        // to re-run so it redraws into the now-cleared region.
        if self.parent_dirty {
            get_observer(render_id).map(|observer| observer.dirten());
        }

        let result = observe(render_id, || {
            debug!(
                "{:indent$}Render {} [#{:?}]",
                "",
                hash_source,
                self.id,
                indent = self.nesting_level
            );

            // Clear our own rect only when the parent hasn't already cleared
            // the containing area (avoids redundant smaller fills inside a
            // larger background that was already repainted).
            if self.force_redraw.get() {
                self.clear_outer()?;
            }

            // Pass parent_dirty=true into the closure so children of this
            // render_part know the area is already cleared.
            f(RenderCtx {
                id: self.id,
                state: self.state,
                renderer: self.renderer,
                layout: self.layout,
                tree_style: self.tree_style,
                page_style: self.page_style,
                viewport: self.viewport,
                fonts: self.fonts,
                theme: self.theme,
                font_props: self.font_props,
                force_redraw: self.force_redraw,
                parent_dirty: true,
                nesting_level: self.nesting_level + 1,
                call: self.call + 1,
                ctx_state: PhantomData,
            })
        });

        // Propagate dirty state back to self so sibling render_part / render_child
        // calls on the same ctx see that the area has been painted.
        if result.is_some() {
            self.parent_dirty = true;
        }

        result.unwrap_or(RenderResult::Ok(()))
    }

    #[must_use]
    pub fn render_self(
        &mut self,
        widget_name: &str,
        f: impl FnOnce(RenderCtx<'_, W, RenderSelf>) -> RenderResult,
    ) -> RenderResult {
        let render_id = format!("{widget_name}_[render_self]");
        self.render_part(&render_id, f)
    }

    #[must_use]
    pub fn render_child(&mut self, child: &El<W>) -> RenderResult {
        self.render_children_inner(core::iter::once(child))
    }

    #[must_use]
    fn render_children_inner<'c, C: Iterator<Item = &'c El<W>> + 'c>(
        &mut self,
        children: C,
    ) -> RenderResult {
        children.zip_eq(self.layout.children()).try_for_each(
            |(child, child_layout)| {
                self.for_child(child.id(), &child_layout, |ctx| {
                    child.render(ctx)
                })
            },
        )
    }

    #[must_use]
    pub fn render_children<'c>(
        &mut self,
        children: &MaybeSignal<Vec<El<W>>>,
    ) -> RenderResult {
        let render_id = WithElId::new(self.id, "render_children");

        // If the parent already cleared the area, force the render_children
        // observer to re-run so children repaint into the cleared region.
        if self.parent_dirty {
            get_observer(render_id).map(|observer| observer.dirten());
        }

        // TODO: Create observe_with_force that forces execution based on boolean (for this case -- parent_dirty)
        let result = observe(render_id, || {
            debug!(
                "{:indent$}Render children [#{:?}]",
                "",
                self.id,
                indent = self.nesting_level
            );

            // Rendering children does not require to clear the rect unless force redraw is called. Because rendering children is done in such widgets that may not have render_self, meaning that nothing is drawn before the children, this allows safely redrawing only the children that are changed.
            if !self.parent_dirty && self.force_redraw.get() {
                self.clear_outer()?;
            }

            children.with(|children| {
                children.iter().zip_eq(self.layout.children()).try_for_each(
                    |(child, child_layout)| {
                        // Pass parent_dirty through so children know whether
                        // the containing area has already been cleared.
                        self.for_child(child.id(), &child_layout, |ctx| {
                            child.render(ctx)
                        })
                    },
                )
            })
        });

        result.unwrap_or(RenderResult::Ok(()))
    }

    #[must_use]
    fn clear_outer(&mut self) -> RenderResult {
        // TODO: Feature-gated or debug-redraw flag
        // Debug redraws, works good only for colors with alpha. But we can use some bright background too
        // self.renderer.rect(
        //     self.layout.outer,
        //     &DrawStyle::default().fill(
        //         W::Color::accents()
        //             [(self.nesting_level + self.call) % ACCENT_COUNT],
        //     ),
        //     // .stroke(W::Color::accents()[4])
        //     // .stroke_width(1),
        // )

        self.page_style.with(|style| {
            if let Some(bg) = style.background_color {
                self.renderer.fill_solid(self.layout.outer, bg).map_err(|_| ())
            } else {
                Ok(())
            }
        })
    }
}

// TODO: Move to event mod?
pub struct EventCtx<'a, W: WidgetCtx> {
    pub id: ElId,
    pub event: &'a Event<W::CustomEvent>,
    pub page_state: Signal<PageState<W>>,
    pub layout: &'a LayoutModelNode<'a>,
    // TODO: Instant now, already can get it from queue!
}

impl<'a, W: WidgetCtx> Copy for EventCtx<'a, W> {}

impl<'a, W: WidgetCtx> Clone for EventCtx<'a, W> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            event: self.event,
            page_state: self.page_state.clone(),
            layout: self.layout,
        }
    }
}

impl<'a, W: WidgetCtx + 'static> EventCtx<'a, W> {
    #[must_use]
    pub fn pass_to_children(
        &mut self,
        children: &mut [El<W>],
    ) -> EventResponse {
        for (child, child_layout) in
            children.iter_mut().zip_eq(self.layout.children())
        {
            child.on_event(EventCtx {
                id: child.id(),
                event: self.event,
                page_state: self.page_state,
                layout: &child_layout,
            })?;
        }
        self.ignore()
    }

    pub fn pass_to_child(&mut self, child: &mut El<W>) -> EventResponse {
        self.pass_to_children(core::slice::from_mut(child))
    }

    pub fn is_focused(&self) -> bool {
        self.page_state.with(|page_state| page_state.is_focused(self.id))
    }

    pub fn is_hovered(&self) -> bool {
        self.page_state.with(|page_state| page_state.is_hovered(self.id))
    }

    /// Returns the cursor position for this event, falling back to the last known position.
    pub fn cursor_pos(&self) -> Option<Point> {
        self.event
            .cursor_point()
            .or_else(|| self.page_state.with(|s| s.pointer.pos))
    }

    /// Called by `HOVERABLE` widgets during a `MouseMove` pass to claim hover for themselves.
    /// The last (deepest) widget to call this during a pass wins.
    pub fn update_hover(&mut self) {
        self.page_state.update(|s| s.pointer.hovered = Some(self.id));
    }

    /// Capture the pointer so all subsequent mouse button events are routed directly here,
    /// regardless of cursor position. Call on `ButtonDown`. Pair with `release_pointer`.
    pub fn capture_pointer(&mut self) {
        self.page_state.update(|s| s.pointer.captured_by = Some(self.id));
    }

    /// Release pointer capture. Call on `ButtonUp`.
    pub fn release_pointer(&mut self) {
        self.page_state.update(|s| s.pointer.captured_by = None);
    }

    // TODO: Automatic handle based on behavior?
    #[must_use]
    pub fn handle_focusable(
        &mut self,
        press: impl FnOnce(&mut Self, bool) -> EventResponse,
    ) -> EventResponse {
        if let &Event::Focus(FocusEvent::Focus(new_focus)) = self.event {
            if new_focus == self.id {
                return self.capture();
            }
        }

        if self.is_focused() {
            match self.event {
                Event::Press(press_event) => {
                    let pressed = match press_event {
                        crate::event::PressEvent::Press => true,
                        crate::event::PressEvent::Release => false,
                    };

                    press(self, pressed)
                },
                _ => self.ignore(),
            }
        } else {
            self.ignore()
        }
    }

    /// Handle a mouse `ButtonDown`/`ButtonUp` where the cursor is within this widget's bounds.
    /// `press` receives `(ctx, button, is_pressed)`.
    #[must_use]
    pub fn handle_clickable(
        &mut self,
        press: impl FnOnce(&mut Self, MouseButton, bool) -> EventResponse,
    ) -> EventResponse {
        let pos = self.cursor_pos();
        match self.event {
            Event::Mouse(MouseEvent::ButtonDown(btn, _)) => {
                if pos.map(|pt| self.layout.outer.contains(pt)).unwrap_or(false)
                {
                    press(self, *btn, true)
                } else {
                    self.ignore()
                }
            },
            Event::Mouse(MouseEvent::ButtonUp(btn, _)) => {
                if pos.map(|pt| self.layout.outer.contains(pt)).unwrap_or(false)
                {
                    press(self, *btn, false)
                } else {
                    self.ignore()
                }
            },
            _ => self.ignore(),
        }
    }

    /// Combines keyboard/encoder focus handling with left-button mouse click.
    /// Prefer this over `handle_focusable` for interactive widgets that support both input modes.
    #[must_use]
    pub fn handle_focusable_or_clickable(
        &mut self,
        press: impl FnOnce(&mut Self, bool) -> EventResponse,
    ) -> EventResponse {
        // Focus event: capture to establish focus
        if let &Event::Focus(FocusEvent::Focus(new_focus)) = self.event {
            if new_focus == self.id {
                return self.capture();
            }
        }

        // Keyboard/encoder press when focused
        if self.is_focused() {
            if let Event::Press(press_event) = self.event {
                let pressed =
                    matches!(press_event, crate::event::PressEvent::Press);
                return press(self, pressed);
            }
        }

        // Mouse left-button click (in bounds)
        let pos = self.cursor_pos();
        match self.event {
            Event::Mouse(MouseEvent::ButtonDown(MouseButton::Left, _)) => {
                if pos.map(|pt| self.layout.outer.contains(pt)).unwrap_or(false)
                {
                    press(self, true)
                } else {
                    self.ignore()
                }
            },
            Event::Mouse(MouseEvent::ButtonUp(MouseButton::Left, _)) => {
                if pos.map(|pt| self.layout.outer.contains(pt)).unwrap_or(false)
                {
                    press(self, false)
                } else {
                    self.ignore()
                }
            },
            _ => self.ignore(),
        }
    }

    /// Handle `MouseMove` for a `HOVERABLE` widget: if cursor is in bounds, claim hover.
    /// Call this at the start of `on_event` for any `HOVERABLE` widget. Always returns `ignore()`.
    pub fn handle_hover_move(&mut self) -> EventResponse {
        if let Event::Mouse(MouseEvent::MouseMove(pt)) = self.event {
            if self.layout.outer.contains(*pt) {
                self.update_hover();
            }
        }
        self.ignore()
    }

    /// Handle `MouseEnter` targeted at this widget.
    #[must_use]
    pub fn handle_mouse_enter(
        &mut self,
        f: impl FnOnce(&mut Self) -> EventResponse,
    ) -> EventResponse {
        if let Event::Mouse(MouseEvent::MouseEnter { target }) = self.event {
            if *target == self.id {
                return f(self);
            }
        }
        self.ignore()
    }

    /// Handle `MouseLeave` targeted at this widget.
    #[must_use]
    pub fn handle_mouse_leave(
        &mut self,
        f: impl FnOnce(&mut Self) -> EventResponse,
    ) -> EventResponse {
        if let Event::Mouse(MouseEvent::MouseLeave { target }) = self.event {
            if *target == self.id {
                return f(self);
            }
        }
        self.ignore()
    }

    #[inline]
    pub fn capture(&self) -> EventResponse {
        EventResponse::Break(Capture::Captured(CaptureData {
            absolute_position: self.layout.outer.top_left,
        }))
    }

    #[inline]
    pub fn ignore(&self) -> EventResponse {
        EventResponse::Continue(Propagate::Ignored)
    }
}
