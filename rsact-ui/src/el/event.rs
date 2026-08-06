use crate::{
    el::{
        arena::{ArenaEls, ElArena},
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
    event: &'a Event<W::CustomEvent>,
    page_state: &'a mut PageState<W>,
}

impl<'a, W: WidgetCtx> EventPass<'a, W> {
    pub fn run(
        arena: &'a mut ElArena<W>,
        event: &'a Event<W::CustomEvent>,
        page_state: &'a mut PageState<W>,
        layout: &'a LayoutModelNode<'a>,
    ) -> EventResponse {
        let mut this = Self { arena: &mut arena.els, event, page_state };
        this.run_(layout, None)
    }

    /// Dispatch the event to a single `target` widget only (used for pointer
    /// capture). Walks the tree from `root` to locate `target` and its
    /// [`LayoutModelNode`], then runs `on_event` for that widget alone — no
    /// other widget sees the event, so hit-testing and hover are bypassed.
    pub fn run_to(
        target: ElId,
        arena: &'a mut ElArena<W>,
        event: &'a Event<W::CustomEvent>,
        page_state: &'a mut PageState<W>,
        layout: &'a LayoutModelNode<'a>,
    ) -> EventResponse {
        let mut this = Self { arena: &mut arena.els, event, page_state };
        this.run_to_(layout, target, None)
            .unwrap_or(EventResponse::Continue(()))
    }

    /// Returns `Some(response)` once `target` is found and dispatched to,
    /// `None` while searching. Walks the layout tree (each node carries its
    /// `ElId`), so the target's layout node is resolved by identity.
    fn run_to_(
        &mut self,
        layout: &LayoutModelNode,
        target: ElId,
        clip: Option<Rect>,
    ) -> Option<EventResponse> {
        if layout.id() == target {
            return Some(self.run_el(layout.id(), layout, clip));
        }

        // The clip is composed while SEARCHING, so the captured widget still
        // receives a truthful `hit_bounds()`. Note what this deliberately does
        // NOT do: capture routing itself is never clipped. Capture exists to
        // override hit-testing — dragging a scrollbar thumb keeps delivering
        // events with the cursor far outside it — so a captured target hears
        // the event whether or not it is visible.
        let child_clip = self.child_clip(layout, clip);
        for child_layout in layout.children() {
            if let Some(response) =
                self.run_to_(&child_layout, target, child_clip)
            {
                return Some(response);
            }
        }
        None
    }

    fn run_(
        &mut self,
        layout: &LayoutModelNode,
        clip: Option<Rect>,
    ) -> EventResponse {
        // WS6.4c: prune the walk for POSITIONAL events — the same predicate the
        // render pass prunes on, with a 1x1 "region", so it bites much harder
        // here. A subtree whose visible area cannot contain the pointer cannot
        // respond to it.
        //
        // Strictly positional-only: `run_` also carries keyboard, encoder and
        // custom events, which every widget may care about regardless of where
        // the cursor happens to be. Pruning those would silently break focus
        // handling.
        if let Some(pos) = positional_event_pos(self.event)
            && !hit_bounds(layout, clip).contains(pos)
        {
            return EventResponse::Continue(());
        }

        // WS5.1: visit children by identity from the layout tree (each node
        // carries its ElId; transparent nodes already flattened). Children
        // first, then this node, preserving the bottom-up dispatch order.
        let child_clip = self.child_clip(layout, clip);
        for child_layout in layout.children() {
            self.run_(&child_layout, child_clip)?;
        }

        self.run_el(layout.id(), layout, clip)
    }

    /// The clip that applies to `layout`'s CHILDREN: the inherited one, narrowed
    /// by this widget's own inner rect if it declares `CLIPS_CHILDREN`.
    ///
    /// The event pass keeps its own clip stack because it has no renderer to ask
    /// — but it must compose exactly the way `Renderer::clip_bounds` does, or
    /// hit-testing and painting disagree about what is visible.
    fn child_clip(
        &self,
        layout: &LayoutModelNode,
        clip: Option<Rect>,
    ) -> Option<Rect> {
        let clips = self
            .arena
            .expect(layout.id())
            .is_some_and(|data| data.state.flags.clips_children_set());

        if clips {
            Some(match clip {
                Some(clip) => clip.intersection(&layout.inner),
                None => layout.inner,
            })
        } else {
            clip
        }
    }

    fn run_el(
        &mut self,
        id: ElId,
        layout: &LayoutModelNode,
        clip: Option<Rect>,
    ) -> EventResponse {
        if let Some(el) = self.arena.get_mut(id).as_mut() {
            if let Some(data) = el.data.as_mut() {
                if let Some(widget) = data.stage.built_mut() {
                    widget.on_event(EventCtx {
                        id,
                        state: &mut data.state,
                        event: self.event,
                        page_state: self.page_state,
                        layout,
                        clip,
                    })
                } else {
                    error!("Element {id:?} has no built widget on event path");
                    EventResponse::Continue(())
                }
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
    /// WS6.4c: the composed clip at this widget — every enclosing
    /// `CLIPS_CHILDREN` ancestor's inner rect, intersected. `None` means
    /// unclipped. Read through [`hit_bounds`](EventCtx::hit_bounds), never
    /// directly.
    clip: Option<Rect>,
    // TODO: Instant now, already can get it from queue!
}

/// The area of `layout` a pointer can actually hit: its outer rect, narrowed by
/// every enclosing clip (WS6.4c).
///
/// **Deliberately NOT outset by `ext_draw`**, which is the one place hit-testing
/// and painting differ:
///
/// ```text
///   paint area = outer.outset(ext_draw) ∩ clips     (an outline is drawn)
///   hit   area = outer                  ∩ clips     (an outline is not a target)
/// ```
///
/// They share the clip because "clipped" means invisible, and hit-testing
/// something invisible is how you click a button that is not there. They differ
/// on the outset because a shadow or focus ring is decoration: growing the hit
/// area with it would make neighbouring widgets' targets overlap.
fn hit_bounds(layout: &LayoutModelNode, clip: Option<Rect>) -> Rect {
    match clip {
        Some(clip) => layout.outer.intersection(&clip),
        None => layout.outer,
    }
}

/// The pointer position an event may be pruned by — deliberately **only**
/// `MouseMove`.
///
/// `MouseMove` is the event worth pruning: a mouse emits hundreds per second
/// against a handful of clicks, so it carries essentially all of the traversal
/// saving. The other positional variants are excluded on purpose, and the
/// reasons are not symmetric:
///
/// - **`ButtonUp`** must reach a widget that is `pressed` even when the cursor
///   has left it, or the widget is stuck pressed forever. Pressing usually
///   captures the pointer (so the release arrives via `run_to`, which is never
///   pruned), but a widget that presses *without* capturing would be stranded —
///   a correctness risk traded against a saving of a few events per second.
/// - **`ButtonDown`/`ButtonUp`** may also carry `None` and fall back to the last
///   known position, so "the event's position" is not even well defined here.
/// - **`Wheel`** is left alone until it is clear nothing wants it globally.
///
/// Keyboard, focus, encoder and custom events are never positional and so are
/// never pruned — pruning those would silently break focus handling.
fn positional_event_pos<C>(event: &Event<C>) -> Option<Point> {
    match event {
        Event::Mouse(MouseEvent::MouseMove(pt)) => Some(*pt),
        _ => None,
    }
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

    /// The area a pointer can hit this widget in: its outer rect narrowed by
    /// every enclosing clip (WS6.4c).
    ///
    /// One definition, and both hit tests below go through it — the same
    /// discipline `paint_bounds` uses on the render side, and the reason those
    /// two can be reasoned about together at all.
    ///
    /// TODO: Customizable bounds. This may be required for widgets like
    /// scrollable that need to handle mouse events at scrollbar only. This is
    /// where that belongs: a widget-declared hit rect would narrow the result
    /// here, and every caller inherits it.
    pub fn hit_bounds(&self) -> Rect {
        hit_bounds(self.layout, self.clip)
    }

    /// Whether the event cursor position (or last known position) lies within
    /// this widget's hittable area.
    ///
    /// WS6.4c: clipped-away area does not count. Before clipping existed this
    /// was `layout.outer` and agreed with what was painted; once `render_part`
    /// started honouring clips, testing `outer` here would mean a scrolled-away
    /// button is invisible and still clickable.
    pub fn cursor_in_bounds(&self) -> bool {
        self.cursor_pos()
            .map(|pt| self.hit_bounds().contains(pt))
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
            // WS6.4c: the hittable area, not the layout rect — a widget scrolled
            // out of its parent's window must not claim hover.
            if self.hit_bounds().contains(*pt) {
                debug!(
                    "Update hover to {}[{:?}] ({})",
                    self.state.debug_name,
                    self.id,
                    self.hit_bounds()
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
