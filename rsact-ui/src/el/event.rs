use crate::el::arena::{ArenaChildren, ArenaEls, ElArena, ElNode};
use crate::el::state::ElState;
use crate::event::*;
use crate::{layout::model::LayoutModelNode, widget::prelude::*};
use itertools::Itertools as _;
use log::{debug, error, warn};

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

    fn run_(&mut self, id: ElId, layout: &LayoutModelNode) -> EventResponse {
        // TODO: Is it possible to avoid double-get from arena? The problem is with mutable arena borrowing because event is processed for children first and then for the parent, while we need parent data to know its flags.
        // TODO: Right check get. Better wrap arena in wrapper as ArenaChildren is
        let data =
            self.arena.get_mut(id).and_then(|el| el.data.as_mut()).unwrap();

        // TODO: Generalize/Take out this logic for EventCtx and RenderCtx
        if data.state.flags.transparent_layout {
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
            for (child, child_layout) in
                children_ids.iter().zip_eq(layout.children())
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
        self.state.hovered
    }

    pub fn is_deepest_hovered(&self) -> bool {
        self.page_state.pointer.hovered == Some(self.id)
    }

    /// Returns the cursor position for this event, falling back to the last known position.
    pub fn cursor_pos(&self) -> Option<Point> {
        self.event.cursor_point().or_else(|| self.page_state.pointer.pos)
    }

    /// Called by `HOVERABLE` widgets during a `MouseMove` pass to claim hover for themselves.
    /// The last (deepest) widget to call this during a pass wins.
    pub fn update_hover(&mut self) {
        self.page_state.pointer.hovered = Some(self.id);
    }

    /// Capture the pointer so all subsequent mouse button events are routed directly here,
    /// regardless of cursor position. Call on `ButtonDown`. Pair with `release_pointer`.
    pub fn capture_pointer(&mut self) {
        self.page_state.pointer.captured_by = Some(self.id);
    }

    /// Release pointer capture. Call on `ButtonUp`.
    pub fn release_pointer(&mut self) {
        self.page_state.pointer.captured_by = None;
    }

    #[must_use]
    pub fn handle(&mut self) -> EventResponse {
        if self.state.flags.hoverable {
            self.handle_hover_move()?;
        }

        if self.state.flags.clickable {}

        self.ignore()
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

    // Mouse events //

    /// Handle `MouseMove` for a `HOVERABLE` widget: if cursor is in bounds, claim hover.
    /// Call this at the start of `on_event` for any `HOVERABLE` widget. Always returns `ignore()`.
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
