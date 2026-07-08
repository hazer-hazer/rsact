use crate::{
    el::{
        arena::{ArenaChildren, ArenaEls, ElArena},
        state::ElState,
    },
    event::*,
    layout::model::LayoutModelNode,
    widget::prelude::*,
};
use log::{debug, error};

// TODO: Do we need request_redraw flag as in update pass? Or all updates in event pass are reactive-only?

pub struct EventPass<'a, W: WidgetCtx> {
    arena: &'a mut ArenaEls<W>,
    children: &'a ArenaChildren,
    event: &'a Event<W::CustomEvent>,
    page_state: &'a mut PageState<W>,
}

impl<'a, W: WidgetCtx> EventPass<'a, W> {
    pub fn run(
        root: ElId,
        arena: &'a mut ElArena<W>,
        event: &'a Event<W::CustomEvent>,
        page_state: &'a mut PageState<W>,
        layout: &'a LayoutModelNode<'a>,
    ) -> EventResponse {
        let mut this = Self {
            arena: &mut arena.els,
            children: &arena.children,
            event,
            page_state,
        };
        this.run_(root, layout)
    }

    /// Dispatch the event to a single `target` widget only (used for pointer
    /// capture). Walks the tree from `root` to locate `target` and its
    /// [`LayoutModelNode`], then runs `on_event` for that widget alone — no
    /// other widget sees the event, so hit-testing and hover are bypassed.
    pub fn run_to(
        target: ElId,
        root: ElId,
        arena: &'a mut ElArena<W>,
        event: &'a Event<W::CustomEvent>,
        page_state: &'a mut PageState<W>,
        layout: &'a LayoutModelNode<'a>,
    ) -> EventResponse {
        let mut this = Self {
            arena: &mut arena.els,
            children: &arena.children,
            event,
            page_state,
        };
        this.run_to_(root, layout, target)
            .unwrap_or(EventResponse::Continue(()))
    }

    /// Returns `Some(response)` once `target` is found and dispatched to,
    /// `None` while searching. Mirrors [`run_`]'s transparent-layout / children
    /// descent so the target's layout node is resolved correctly.
    fn run_to_(
        &mut self,
        id: ElId,
        layout: &LayoutModelNode,
        target: ElId,
    ) -> Option<EventResponse> {
        if id == target {
            return Some(self.run_el(id, layout));
        }

        // Extract the flag before recursing so the arena borrow ends.
        let transparent = match self.arena.expect(id) {
            Some(data) => data.state.flags.is_transparent_layout(),
            None => return None,
        };

        if transparent {
            if let Some(children_ids) = self.children.get(id)
                && children_ids.len() == 1
            {
                return self.run_to_(children_ids[0], layout, target);
            }
            None
        } else if let Some(children_ids) = self.children.get(id) {
            crate::el::check_children_parallel(
                "event(run_to)",
                id,
                children_ids.len(),
                layout.children_len(),
            );
            for (child, child_layout) in
                children_ids.iter().zip(layout.children())
            {
                if let Some(response) =
                    self.run_to_(*child, &child_layout, target)
                {
                    return Some(response);
                }
            }
            None
        } else {
            None
        }
    }

    fn run_(&mut self, id: ElId, layout: &LayoutModelNode) -> EventResponse {
        // TODO: Is it possible to avoid double-get from arena? The problem is
        // with mutable arena borrowing because event is processed for children
        // first and then for the parent, while we need parent data to know its
        // flags.
        let Some(data) = self.arena.expect(id) else {
            return EventResponse::Continue(());
        };

        // TODO: Generalize/Take out this logic for EventCtx and RenderCtx
        if data.state.flags.is_transparent_layout() {
            if let Some(children_ids) = self.children.get(id)
                && children_ids.len() == 1
            {
                let child_id = children_ids[0];
                self.run_(child_id, layout)?;
            } else {
                error!(
                    "Transparent widget with id {id:?} should have exactly one child"
                );
            }
        } else if let Some(children_ids) = self.children.get(id) {
            crate::el::check_children_parallel(
                "event(run)",
                id,
                children_ids.len(),
                layout.children_len(),
            );
            for (child, child_layout) in
                children_ids.iter().zip(layout.children())
            {
                self.run_(*child, &child_layout)?;
            }
        }

        self.run_el(id, layout)
    }

    fn run_el(&mut self, id: ElId, layout: &LayoutModelNode) -> EventResponse {
        if let Some(el) = self.arena.get_mut(id).as_mut() {
            if let Some(data) = el.data.as_mut() {
                data.widget.on_event(EventCtx {
                    id,
                    state: &mut data.state,
                    event: self.event,
                    page_state: self.page_state,
                    layout,
                })
            } else {
                error!(
                    "Trying to run event on element with id {:?} that has no data",
                    id
                );
                EventResponse::Continue(())
            }
        } else {
            error!(
                "Trying to run event on non-existent element with id {:?}",
                id
            );
            EventResponse::Continue(())
        }
    }
}

pub struct EventCtx<'a, W: WidgetCtx> {
    pub id: ElId,
    state: &'a mut ElState<W>,
    pub event: &'a Event<W::CustomEvent>,
    pub page_state: &'a mut PageState<W>,
    pub layout: &'a LayoutModelNode<'a>,
    // TODO: Instant now, already can get it from queue!
}

impl<'a, W: WidgetCtx + 'static> EventCtx<'a, W> {
    // #[must_use]
    // pub fn pass_to_children(
    //     &mut self,
    //     children: &mut [El<W>],
    // ) -> EventResponse {
    //     for (child, child_layout) in
    //         children.iter_mut().zip_eq(self.layout.children())
    //     {
    //         let (child_id, child) = self.arena.expect_stored_mut(child);
    //         child.on_event(EventCtx {
    //             id: child_id,
    //             event: self.event,
    //             page_state: self.page_state,
    //             layout: &child_layout,
    //         })?;
    //     }
    //     self.ignore()
    // }

    // pub fn pass_to_child(&mut self, child: &mut El<W>) -> EventResponse {
    //     self.pass_to_children(core::slice::from_mut(child))
    // }

    pub fn is_focused(&self) -> bool {
        self.page_state.is_focused(self.id)
    }

    pub fn is_hovered(&self) -> bool {
        self.state.hovered()
    }

    pub fn is_deepest_hovered(&self) -> bool {
        self.page_state.pointer.hovered == Some(self.id)
    }

    /// Whether this widget is the globally pressed widget, from either input
    /// source: the mouse (`pointer.pressed`) or the focus/encoder button
    /// (`focus_pressed` on the focused widget). Event logic reads this global
    /// state; rendering reads the [`ElState`] cache via the `pressed`
    /// pseudo-class.
    pub fn is_pressed(&self) -> bool {
        self.page_state.pointer.pressed == Some(self.id)
            || (self.is_focused() && self.page_state.focus_pressed)
    }

    /// Whether this widget currently holds the pointer capture. While it does,
    /// it receives every mouse event exclusively (see [`capture_pointer`]),
    /// even when the cursor leaves its bounds — the basis for dragging.
    pub fn is_captured(&self) -> bool {
        self.page_state.pointer.captured_by == Some(self.id)
    }

    /// Returns the cursor position for this event, falling back to the last
    /// known position.
    pub fn cursor_pos(&self) -> Option<Point> {
        self.event
            .cursor_point()
            .or_else(|| self.page_state.pointer.pos)
    }

    // TODO: Customizable bounds. This may be required for widgets like scrollable that need to handle mouse events at scrollbar only.
    /// Whether the event cursor position (or last known position) lies within
    /// this widget's outer layout rect.
    pub fn cursor_in_bounds(&self) -> bool {
        self.cursor_pos()
            .map(|pt| self.layout.outer.contains(pt))
            .unwrap_or(false)
    }

    /// Called by `HOVERABLE` widgets during a `MouseMove` pass to claim hover
    /// for themselves. The last (deepest) widget to call this during a pass
    /// wins.
    pub fn update_hover(&mut self) {
        self.page_state.pointer.hovered = Some(self.id);
    }

    /// Capture the pointer so all subsequent mouse button events are routed
    /// directly here, regardless of cursor position. Call on `ButtonDown`.
    /// Pair with `release_pointer`.
    pub fn capture_pointer(&mut self) {
        self.page_state.pointer.captured_by = Some(self.id);
    }

    /// Release pointer capture. Call on `ButtonUp`.
    pub fn release_pointer(&mut self) {
        self.page_state.pointer.captured_by = None;
    }

    // TODO: Maybe better rename to `handle_behavior` or `handle_behavioral`?
    /// Automatic, source-of-truth behavioral bookkeeping for a widget: hover
    /// tracking plus **press claiming**. Call this first in `on_event`. It does
    /// NOT run the widget's action — pair it with [`handle_click`] for that.
    ///
    /// On mouse `ButtonDown` in bounds a `CLICKABLE` widget claims the global
    /// press ([`PointerState::pressed`]), captures the pointer, and breaks
    /// propagation so the deepest clickable widget under the cursor wins. On a
    /// focus/encoder `Press` a `FOCUSABLE` focused widget sets
    /// [`PageState::focus_pressed`]. The page turns these state changes into
    /// the `pressed` pseudo-class and clears them on the matching release.
    #[must_use]
    pub fn handle(&mut self) -> EventResponse {
        if self.state.flags.is_hoverable() {
            self.handle_hover_move()?;
        }

        if self.state.flags.is_clickable()
            && let Event::Mouse(MouseEvent::ButtonDown(MouseButton::Left, _)) =
                self.event
            && self.cursor_in_bounds()
        {
            self.page_state.pointer.pressed = Some(self.id);
            self.capture_pointer();
            return self.capture();
        }

        if self.state.flags.is_focusable()
            && self.is_focused()
            && let Event::Press(PressEvent::Press) = self.event
        {
            self.page_state.focus_pressed = true;
            return self.capture();
        }

        self.ignore()
    }

    /// Run `on_click` exactly when a **completed click** targets this widget: a
    /// mouse `ButtonUp` on the same widget that received the press (with the
    /// cursor still in bounds), or a focus/encoder `Release` while this focused
    /// widget was press-claimed. Behavior (callbacks, value toggles) lives in
    /// the widget; the press *state* is managed globally by [`handle`] and the
    /// page. `on_click` should return [`capture`] to stop propagation.
    #[must_use]
    pub fn handle_click(
        &mut self,
        on_click: impl FnOnce(&mut Self) -> EventResponse,
    ) -> EventResponse {
        // Mouse: release on the same widget that received the press.
        if let Event::Mouse(MouseEvent::ButtonUp(MouseButton::Left, _)) =
            self.event
            && self.page_state.pointer.pressed == Some(self.id)
            && self.cursor_in_bounds()
        {
            return on_click(self);
        }

        // Encoder/keyboard: release while focused after a focus-press.
        if self.is_focused()
            && self.page_state.focus_pressed
            && matches!(self.event, Event::Press(PressEvent::Release))
        {
            return on_click(self);
        }

        self.ignore()
    }

    // Mouse events //

    /// Handle `MouseMove` for a `HOVERABLE` widget: if cursor is in bounds,
    /// claim hover. Call this at the start of `on_event` for any
    /// `HOVERABLE` widget. Always returns `ignore()`.
    #[must_use]
    pub fn handle_hover_move(&mut self) -> EventResponse {
        if let Event::Mouse(MouseEvent::MouseMove(pt)) = self.event {
            if self.layout.outer.contains(*pt) {
                debug!(
                    "Update hover to {}[{:?}] ({})",
                    self.state.debug_name, self.id, self.layout.outer
                );
                self.update_hover();
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
        EventResponse::Continue(())
    }
}
