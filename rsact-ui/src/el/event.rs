use crate::el::arena::{ArenaChildren, ArenaEls, ElArena, ElNode};
use crate::event::*;
use crate::{layout::model::LayoutModelNode, widget::prelude::*};
use itertools::Itertools as _;
use log::{error, warn};

pub struct EventCtx<'a, W: WidgetCtx> {
    pub id: ElId,
    pub event: &'a Event<W::CustomEvent>,
    pub page_state: Signal<PageState<W>>,
    pub layout: &'a LayoutModelNode<'a>,
    // TODO: Instant now, already can get it from queue!
}

impl<'a, W: WidgetCtx + 'static> EventCtx<'a, W> {
    pub fn run<'arena>(
        id: ElId,
        arena: &'arena mut ElArena<W>,
        event: &'a Event<W::CustomEvent>,
        page_state: Signal<PageState<W>>,
        layout: &'a LayoutModelNode<'a>,
    ) -> EventResponse {
        let mut ctx = Self { id, event, page_state, layout };
        ctx.run_(id, &mut arena.els, &arena.children)
    }

    fn run_<'arena>(
        &mut self,
        el: ElId,
        arena: &'arena mut ArenaEls<W>,
        children: &'arena ArenaChildren,
    ) -> EventResponse {
        if let Some(children_ids) = children.get(el) {
            for child in children_ids {
                self.run_(*child, arena, children)?;
            }
        }

        self.run_el(el, arena)
    }

    fn run_el<'arena>(
        &mut self,
        id: ElId,
        arena: &'arena mut ArenaEls<W>,
    ) -> EventResponse {
        if let Some(el) = arena.get_mut(id).as_mut() {
            if let Some(data) = el.data.as_mut() {
                data.widget.on_event(Self {
                    id,
                    event: self.event,
                    page_state: self.page_state,
                    layout: self.layout,
                })
            } else {
                error!(
                    "Trying to run event on element with id {:?} that has no data",
                    id
                );
                self.ignore()
            }
        } else {
            error!(
                "Trying to run event on non-existent element with id {:?}",
                id
            );
            self.ignore()
        }
    }

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
        EventResponse::Continue(())
    }
}
