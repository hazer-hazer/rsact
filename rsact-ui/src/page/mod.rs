use crate::{
    el::{arena::ElArena, build::BuildCtx, *},
    event::{
        Capture, Event, EventResponse, MouseButton, MouseEvent, PressEvent,
        UnhandledEvent,
    },
    font::{Font, FontCtx, FontProps},
    layout::{
        LayoutCtx, Limits,
        model::{LayoutModel, PPLayoutModel, model_layout},
    },
    render::prelude::*,
    style::TreeStyle,
};
use alloc::vec::Vec;
use dev::{DevHoveredEl, DevTools};
use log::{debug, info};
use rsact_reactive::prelude::*;

pub mod dev;
pub mod id;

pub struct PageStyle<C: Color> {
    // TODO: Use ColorStyle
    pub background_color: Option<C>,
}

impl<C: Color> PageStyle<C> {
    pub fn base() -> Self {
        // TODO: Base should be None and user should set theme/palette
        Self { background_color: Some(C::default_background()) }
    }

    pub fn background_color(mut self, background_color: C) -> Self {
        self.background_color = Some(background_color);
        self
    }
}

// TODO: As we now have element arena we can split functionality to add
// optimization structures. For example, not all drivers have focusing logic, so
// we can have it behind a generic or other abstraction to toggle, and pre-build
// focusable elements list only when it's needed. Same for hoverable, etc.
// Hoverable is kinda more interesting case because with element arena we can
// build spatial index like an R-Tree or just cache hit-tests which is also
// good.

pub struct Page<W: WidgetCtx> {
    id: W::PageId,
    root: ElId,
    needs_redraw: bool,
    arena: Signal<ElArena<W>>,
    // TODO: MaybeReactive
    layout: Memo<LayoutModel>,
    state: PageState<W>,
    style: Signal<PageStyle<W::Color>>,
    // TODO: Just use Rc<RefCell<R>> because we don't need to track renderer.
    renderer: Signal<W::Renderer>,
    viewport: MaybeReactive<Size>,
    stylist: Inert<W::Stylist>,
    dev_tools: Signal<DevTools>,
    force_redraw: Signal<bool>,
    render_calls: usize,
    fonts: Signal<FontCtx>,
}

impl<W: WidgetCtx> Drop for Page<W> {
    fn drop(&mut self) {
        unsafe {
            self.arena.dispose();
        }
    }
}

impl<W: WidgetCtx> Page<W> {
    pub(crate) fn new(
        id: W::PageId,
        // TODO: Do we really need to accept Into<El> and expect it to always
        // be a El::New, or we can require root to be a Widget non-wrapped?
        // No, ElData can contain additional information raw widget does not
        // provide
        root: impl View<W>,
        arena: Signal<ElArena<W>>,
        viewport: MaybeReactive<Size>,
        stylist: Inert<W::Stylist>,
        dev_tools: Signal<DevTools>,
        renderer: Signal<W::Renderer>,
        fonts: Signal<FontCtx>,
    ) -> Self {
        let mut root: El<W> = root.into_el();
        let state = PageState::new();

        let mut force_redraw = create_signal(false).name("Force redraw");

        let root = BuildCtx::run(&mut root, arena);

        let layout_tree = arena.with(|arena| {
            arena
                .expect(root)
                .expect("Root node must be built")
                .widget
                .layout()
                .name("Layout tree")
        });
        // TODO: If we make fonts MaybeReactive, we can go fully MaybeReactive
        // LayoutModel here
        let layout_model = map!(move |fonts, viewport| {
            info!("Relayout page {:?}", id);

            // TODO: Possible optimization is to use previous memo result.
            // [ ] Pass it to model_layout as tree and don't relayout parents if
            // layouts inside Fixed-sized container changed,
            // returning previous result

            let viewport = *viewport;
            let layout = model_layout(
                &LayoutCtx {
                    fonts,
                    viewport,
                    font_props: FontProps {
                        font: Some(Font::Auto),
                        font_size: None,
                        font_style: None,
                    },
                },
                layout_tree,
                Limits::only_max(viewport),
                viewport.into(),
            );

            // TODO: Do we need full page redraw on layout change?
            // [ ] No, we need smart bottom-up propagation to the nearest fixed
            // parent layout.
            force_redraw.set(true);

            debug!("{}", PPLayoutModel::root(&layout));

            layout
        })
        .name("Layout model");

        let style = PageStyle::base().signal().name("Page style");

        Self {
            id,
            root,
            needs_redraw: true,
            arena,
            layout: layout_model,
            state,
            style,
            // TODO: Signal viewport in Renderer? Windows can change size.
            renderer,
            viewport: viewport.name("Viewport"),
            stylist,
            dev_tools,
            force_redraw,
            render_calls: 0,
            fonts,
        }
    }

    pub(crate) fn id(&self) -> W::PageId {
        self.id
    }

    pub(crate) fn force_redraw(&mut self) -> &mut Self {
        info!("Force redraw page {:?}", self.id);
        self.force_redraw.set(true);
        self
    }

    pub fn take_draw_calls(&mut self) -> usize {
        core::mem::replace(&mut self.render_calls, 0)
    }

    // TODO
    // pub fn style(
    //     mut self,
    //     style: impl IntoSignal<PageStyle<C::Color>>,
    // ) -> Self {
    //     self.style = style.signal();
    //     self
    // }

    pub fn clear(&mut self) -> &mut Self {
        let viewport = self.viewport.get();
        self.style.with(|style| {
            // TODO: Will not work without background, must always have a
            // background
            if let Some(bg) = style.background_color {
                self.renderer
                    .update_untracked(|r| {
                        Renderer::fill_solid(
                            r,
                            Rect::new(Point::zero(), viewport),
                            bg,
                        )
                    })
                    .ok()
                    .unwrap();
            }
        });
        self
    }

    // Focus //

    // /// Focus first focusable element in page
    // pub fn focus_first(&mut self) {
    //     self.focus_el(0);
    // }

    // /// Focus first focusable element in page if no element focused
    // pub fn apply_auto_focus(&mut self) {
    //     if self.state.with(|state| state.focused.is_none()) {
    //         self.focus_first();
    //     }
    // }

    // /// Find element to focus by offset from currently focused element index.
    // fn find_focus(&mut self, offset: i32) -> Option<(ElId, usize)> {
    //     let focusable_count = self.meta.focusable.with(Vec::len);

    //     let current_offset = self.state.with(|state| {
    //         state.focused.as_ref().map(|focused| focused.1).unwrap_or(0)
    //     });

    //     let new_focus_offset = (current_offset as i64 + offset as i64)
    //         .clamp(0, focusable_count as i64)
    //         as usize;

    //     let new_focus_id = self
    //         .meta
    //         .focusable
    //         .with(|focusable| focusable.get(new_focus_offset).copied());

    //     // Set new focus only in case there's a corresponding element by
    // index. Otherwise it means buggy meta collection     if let
    // Some(new_focus_id) = new_focus_id {         Some((new_focus_id,
    // new_focus_offset))     } else {
    //         None
    //     }
    // }

    // fn apply_focus(&mut self, new_focus: (ElId, usize)) {
    //     self.state.update(|state| state.focused = Some(new_focus))
    // }

    // /// Send focus event to tree. Sets new focus if event was captured
    // fn focus_el(&mut self, offset: i32) -> Option<UnhandledEvent<W>> {
    //     if let Some(new_focus) = self.find_focus(offset) {
    //         let response =
    //
    // self.send_event(Event::Focus(FocusEvent::Focus(new_focus.0)));

    //         if response.is_none() {
    //             self.apply_focus(new_focus);
    //         }

    //         response
    //     } else {
    //         None
    //     }
    // }

    /// For Dev tools
    fn find_el_under_cursor(&self, point: Point) -> Option<DevHoveredEl> {
        self.layout.with(|layout| {
            layout
                .tree_root()
                .dev_hover(point)
                .map(|layout| DevHoveredEl { layout })
        })
    }

    /// Apply global logic for unhandled events.
    /// Some events have different interpretations.
    /// For example `MoveEvent` can be treated as focus move, and if no element
    /// captured this event, we move the focus.
    fn on_unhandled_event(
        &mut self,
        unhandled: Event<W::CustomEvent>,
    ) -> Option<UnhandledEvent<W>> {
        // if let Some(focus_offset) = unhandled.interpret_as_focus_move() {
        //     // TODO: Focus event is eaten here, even if no element focused.
        // This might be incorrect     return
        // self.focus_el(focus_offset); }

        Some(UnhandledEvent::Event(unhandled))
    }

    #[must_use]
    fn send_specific_event(
        &mut self,
        event: &Event<W::CustomEvent>,
    ) -> EventResponse {
        // Note: Need to have special deferred reactive updates zone. Because if
        // some child node depends on value it's children set, then there will
        // be a BorrowRefMut error because children are borrowed mutably for
        // update on events. This happens for example if flex layout contains a
        // checkbox toggling this flex layout wrap.

        let defer_effects = defer_effects();

        let res = self.layout.with(|layout| {
            let response = self.arena.update_untracked(|arena| {
                EventPass::run(
                    self.root,
                    arena,
                    event,
                    &mut self.state,
                    &layout.tree_root(),
                )
            });

            // TODO: notify root on event capture?
            //  - No, root is not used reactively, it is a signal only to be
            //    usable in reactive contexts. Need `StoredValue`

            response
        });

        defer_effects.run();

        res
    }

    /// Dispatch an event to a single widget (its layout node is resolved by
    /// walking the tree). Used for pointer capture: the capturing widget
    /// receives mouse events exclusively.
    fn send_event_to(
        &mut self,
        target: ElId,
        event: &Event<W::CustomEvent>,
    ) -> EventResponse {
        let defer_effects = defer_effects();

        let res = self.layout.with(|layout| {
            self.arena.update_untracked(|arena| {
                EventPass::run_to(
                    target,
                    self.root,
                    arena,
                    event,
                    &mut self.state,
                    &layout.tree_root(),
                )
            })
        });

        defer_effects.run();

        res
    }

    fn send_update_inner(
        id: ElId,
        update: Update,
        arena: &mut ElArena<W>,
    ) -> UpdateResult {
        let Some(el) = arena.expect_mut(id) else {
            return UpdateResult::none();
        };

        debug!("Send update {:?} to {}[{:?}]", update, el.state.debug_name, id);
        let result =
            el.widget
                .update(UpdateCtx { id, update, state: &mut el.state });

        if result.should_bubble()
            && let Some(bubble) = update.as_bubble()
            && let Some(parent) = arena.parents.get(id).copied()
        {
            Self::send_update_inner(parent, bubble, arena).merge(result)
        } else {
            result
        }
    }

    fn send_update(&mut self, id: ElId, update: Update) {
        let result = self.arena.update_untracked(|arena| {
            Self::send_update_inner(id, update, arena)
        });

        if result.is_redraw_requested() {
            self.needs_redraw = true;
        }
    }

    #[must_use]
    fn send_event(
        &mut self,
        event: Event<W::CustomEvent>,
    ) -> Option<UnhandledEvent<W>> {
        // === Pointer capture ===
        // While a widget holds the pointer capture, every mouse event is
        // delivered to it exclusively — no hit-testing and no hover changes —
        // so it can keep tracking the cursor even when it leaves its own bounds
        // (drags, sliders, scrollbars). A `ButtonUp` ends the capture (and any
        // in-flight press). Non-mouse events fall through to normal handling.
        if let Some(captured_id) = self.state.pointer.captured_by
            && let Event::Mouse(mouse) = &event
        {
            if let MouseEvent::MouseMove(pt) = mouse {
                self.state.pointer.pos = Some(*pt);
            }

            let _ = self.send_event_to(captured_id, &event);

            if matches!(mouse, MouseEvent::ButtonUp(_, _)) {
                self.state.pointer.captured_by = None;

                // End the mouse press and reset the pressed visual.
                // Unconditional so a release outside the widget's bounds
                // (drag-off) still ends the press; a no-op for non-click
                // captures (e.g. a `Scrollable` drag leaves `pressed` `None`).
                if let Some(pressed) = self.state.pointer.pressed {
                    self.send_update(pressed, Update::PressChange(false));
                    self.state.pointer.pressed = None;
                }
            }

            return None;
        }

        if let Event::Mouse(MouseEvent::MouseMove(point)) = event {
            if self.dev_tools.with(|dt| dt.enabled) {
                let hovered_el = self.find_el_under_cursor(point);
                let dev_hovered_changed = self.dev_tools.update(|dt| {
                    let changed = dt.hovered != hovered_el;
                    dt.hovered = hovered_el;
                    changed
                });

                if dev_hovered_changed {
                    // TODO: Real rendering requires smarter dirty rectangles as
                    // dev tools are overlaying and have absolute position.
                    // Clearing whole screen is bad

                    self.force_redraw();
                }
                // TODO: Should dev tools capture mouse movement?
                // return None;
            }

            self.state.pointer.pos = Some(point);

            let old_hovered = self.state.pointer.hovered;
            self.state.pointer.hovered = None;
            let _ = self.send_specific_event(&event);

            // Dispatch Enter/Leave if hovered widget changed
            let new_hovered = self.state.pointer.hovered;
            if old_hovered != new_hovered {
                debug!("Hover change: {:?} -> {:?}", old_hovered, new_hovered);
                if let Some(left) = old_hovered {
                    self.send_update(left, Update::HoverChange(false));
                }
                if let Some(entered) = new_hovered {
                    self.send_update(entered, Update::HoverChange(true));
                }
            }

            // TODO: Should mouse event always be ignored?
            return None;
        }

        // Global, page-level event handling //
        // Press bookkeeping mirrors hover: the event pass claims the press
        // (mouse via `pointer.pressed`, encoder via `focus_pressed`); here we
        // turn that state change into the `pressed` pseudo-class. Clearing on
        // mouse release happens in the capture branch above; focus release is
        // handled below.
        let old_pressed = self.state.pointer.pressed;

        let response = self.send_specific_event(&event);

        match &event {
            // Mouse press just claimed during this pass (normally `None ->
            // Some`); notify the newly pressed widget.
            Event::Mouse(MouseEvent::ButtonDown(MouseButton::Left, _)) => {
                let new_pressed = self.state.pointer.pressed;
                if old_pressed != new_pressed {
                    if let Some(old) = old_pressed {
                        self.send_update(old, Update::PressChange(false));
                    }
                    if let Some(new) = new_pressed {
                        self.send_update(new, Update::PressChange(true));
                    }
                }
            },
            // Encoder/keyboard press claimed on the focused widget.
            Event::Press(PressEvent::Press) => {
                if self.state.focus_pressed
                    && let Some((id, _)) = self.state.focused
                {
                    self.send_update(id, Update::PressChange(true));
                }
            },
            // Encoder/keyboard release: the pass already fired `handle_click`
            // (it saw `focus_pressed == true`); now end the press.
            Event::Press(PressEvent::Release) => {
                if self.state.focus_pressed {
                    if let Some((id, _)) = self.state.focused {
                        self.send_update(id, Update::PressChange(false));
                    }
                    self.state.focus_pressed = false;
                }
            },
            _ => {},
        }

        match response {
            EventResponse::Continue(()) => {
                info!(
                    "Event {:?} was ignored, applying global handling",
                    event
                );
                self.on_unhandled_event(event)
            },
            EventResponse::Break(capture) => match capture {
                // TODO: Captured data may be useful for debugging, for example
                // we can point where on screen user clicked or something
                Capture::Captured(_capture) => {
                    info!(
                        "Event {:?} was captured, stopping propagation",
                        event
                    );
                    None
                },
            },
        }
    }

    pub fn handle_events(
        &mut self,
        events: impl Iterator<Item = Event<W::CustomEvent>>,
    ) -> Vec<UnhandledEvent<W>> {
        let unhandled =
            events.filter_map(|event| self.send_event(event)).collect();

        unhandled
    }

    pub fn render<T: RenderTarget>(&mut self, target: &mut T) -> bool
    where
        W::Renderer: FinishRender<T::Color>,
    {
        self.use_renderer(|renderer| {
            renderer.finish_frame(target);
        })
    }

    pub fn use_renderer(&mut self, f: impl FnOnce(&mut W::Renderer)) -> bool {
        let mut renderer = self.renderer;

        #[cfg(feature = "debug-info")]
        {
            observe(("page_force_redraw", self.id), || {
                self.force_redraw.track();

                debug!("Force redraw page {:?}", self.id);
            });
        }

        let redraw_reason = self.arena.update_untracked(|arena| {
            arena
                .expect_mut(self.root)
                .unwrap()
                .state
                .take_needs_redraw()
        });
        let needs_redraw = redraw_reason.is_some() || self.needs_redraw;

        let drawn =
            observe_with_force(("render_page", self.id), needs_redraw, || {
                info!(
                    "Render page {:?} (call: {})",
                    self.id,
                    self.render_calls + 1
                );

                #[cfg(feature = "debug-info")]
                {
                    rsact_reactive::debug::observer_debug_info().map(|di| {
                        info!("Rerender debug info: {di}");
                    });
                }

                self.force_redraw.track();

                self.render_calls += 1;

                renderer
                .update_untracked(|renderer| {
                    // self.style
                    //     .with(|style| {
                    //         if let Some(background_color) =
                    //             style.background_color
                    //         {
                    //             debug!(
                    //                 "Clear page {:?} with color {:?}",
                    //                 self.id, background_color
                    //             );
                    //             let viewport = self.viewport.get();
                    //             Renderer::fill_solid(
                    //                 renderer,
                    //                 Rect::new(Point::zero(), viewport),
                    //                 background_color,
                    //             )
                    //         } else {
                    //             Ok(())
                    //         }
                    //     })
                    //     .ok()
                    //     .unwrap();

                    let fonts = self.fonts.read_only();
                    let layout = self.layout;
                    let stylist = self.stylist;

                    with!(|layout, stylist| {
                        debug!("Force redraw: {}", self.force_redraw.get());
                        self.arena.update_untracked(|arena| {
                            RenderPass::new(
                                arena,
                                renderer,
                                RenderShared {
                                    page_state: &self.state,
                                    page_style: self.style.read_only(),
                                    viewport: self.viewport,
                                    fonts,
                                    stylist,
                                    force_redraw: self.force_redraw,
                                },
                            )
                            .render(
                                self.root,
                                &layout.tree_root(),
                                RenderVisual {
                                    tree_style: TreeStyle::base(),
                                    font_props: FontProps {
                                        font: Some(Font::Auto),
                                        font_size: None,
                                        font_style: None,
                                    },
                                },
                                RenderFrame::root(self.render_calls),
                            )
                        })
                    })?;

                    self.dev_tools.with(|dev_tools| {
                        if dev_tools.enabled {
                            if let Some(hovered) = &dev_tools.hovered {
                                return hovered
                                    .draw::<W>(renderer,
                                        fonts,self.viewport.get());
                            }
                        }
                        Ok(())
                    })
                })
                // A render error must not abort the device: log and continue.
                // A dropped frame is recoverable; a panic in the render loop
                // (which runs every frame) is not.
                .unwrap_or_else(|_| {
                    log::error!("page render failed; skipping this frame");
                });
            });

        //
        self.force_redraw.set_untracked(false);
        self.needs_redraw = false;

        // TODO: Can be put directly into the observe
        if drawn.is_some() {
            self.renderer.update_untracked(|renderer| f(renderer));

            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Page, dev::DevTools};
    use crate::{
        el::{El, arena::ElArena, ctx::*, view::View},
        font::FontCtx,
        prelude::*,
        style::theme::Theme,
        widget::Widget,
    };
    use alloc::string::String;
    use rsact_reactive::prelude::*;

    type NullWtf = Wtf<NullRenderer, (), (), ()>;

    fn create_null_page(root: impl View<NullWtf>) -> Page<NullWtf> {
        let arena = create_signal(ElArena::new()).name("Page arena");

        Page::new(
            (),
            root,
            arena,
            Size::new_equal(1).maybe_reactive(),
            ().inert(),
            DevTools::default().signal(),
            NullRenderer::default().signal(),
            FontCtx::new().signal(),
        )
    }

    /// Rebuilding a subtree must not leak the old subtree in the arena. A
    /// `Dynamic` (factory closure) rebuilds its child on each signal change; the
    /// arena's live-node count must stay constant across many rebuilds (before
    /// the fix, each rebuild orphaned the old child subtree in `els`).
    #[test]
    fn arena_rebuild_does_not_leak_subtree() {
        use crate::widget::{container::Container, label::Label};
        use rsact_reactive::runtime::with_new_runtime;

        with_new_runtime(|_| {
            let mut rebuild = create_signal(0i32);
            // Rebuilds are driven by the Dynamic's build effect on each `set`,
            // so no render is needed (rendering the null theme would hit an
            // unrelated pre-existing ColorStyle panic).
            let page = create_null_page(move || {
                rebuild.get(); // track: re-run the factory on each change
                Container::new(Label::new("x".inert()).el())
            });

            let baseline = page.arena.with(|arena| arena.el_count());
            assert!(baseline > 0, "arena should have nodes after first build");

            for i in 1..=20 {
                rebuild.set(i);
            }

            let after = page.arena.with(|arena| arena.el_count());
            assert_eq!(
                after,
                baseline,
                "arena leaked {} node(s) across 20 rebuilds",
                after as i64 - baseline as i64
            );
        });
    }

    #[test]
    fn draw_on_demand() {
        let mut redraw_signal_data = String::new().signal();

        let mut page = create_null_page(Label::new(redraw_signal_data).el());

        assert_eq!(page.take_draw_calls(), 0);

        // First draw request without changes subscribes to reactive values
        // inside drawing context.
        page.use_renderer(|_| {});
        assert_eq!(page.take_draw_calls(), 1);

        // Nothing changed inside drawing context
        page.use_renderer(|_| {});
        assert_eq!(page.take_draw_calls(), 0);

        // Something's changed
        redraw_signal_data.update(|string| string.push_str("kek"));
        page.use_renderer(|_| {});
        assert_eq!(page.take_draw_calls(), 1);

        page.use_renderer(|_| {});
        assert_eq!(page.take_draw_calls(), 0);
        page.use_renderer(|_| {});
        assert_eq!(page.take_draw_calls(), 0);
    }

    // Regression: a `Checkbox` built from a plain `bool` (the "uncontrolled"
    // case) must still redraw when toggled by a click/press. Previously the
    // checked value was stored as a `MaybeSignal<bool>`, which is `Inert` for a
    // plain value: the `value.get()` in `render` didn't track and the
    // `value.update()` in `on_event` didn't notify, so the toggle changed the
    // internal value but never triggered a redraw.
    #[test]
    fn checkbox_redraws_on_toggle() {
        use crate::event::{Event, PressEvent};

        // Isolate in a fresh runtime: all null pages share page id `()`, so the
        // page-observer key `("render_page", ())` collides in the shared
        // `static_observers` and tests would pollute each other otherwise.
        with_new_runtime(|_| {
            let mut page = create_null_page(Checkbox::new(false).el());

            // Render until the page settles (the first passes warm up
            // layout/style), then confirm it is quiescent with nothing changing.
            for _ in 0..4 {
                page.use_renderer(|_| {});
            }
            page.take_draw_calls();
            page.use_renderer(|_| {});
            assert_eq!(
                page.take_draw_calls(),
                0,
                "page must be quiescent before the toggle"
            );

            // Focus the checkbox (it is the page root) and click it (press then
            // release) — the checkbox toggles its value on release.
            page.state.focused = Some((page.root, 0));
            let _ = page.handle_events(
                [
                    Event::Press(PressEvent::Press),
                    Event::Press(PressEvent::Release),
                ]
                .into_iter(),
            );

            // Toggling the checked value must redraw the checkbox. Before the
            // fix the inert `MaybeSignal<bool>` neither tracked the read in
            // `render` nor notified the write in `on_event`, so this stayed 0.
            page.use_renderer(|_| {});
            assert_eq!(
                page.take_draw_calls(),
                1,
                "toggling the checkbox must trigger a redraw"
            );
        });
    }

    // Reproduce the real user scenario: a full mouse click (press + release,
    // with the pointer hovering the widget). The checkbox toggles on release.
    #[test]
    fn checkbox_redraws_on_mouse_click() {
        use crate::event::{Event, MouseButton, MouseEvent};

        with_new_runtime(|_| {
            let mut page = create_null_page(Checkbox::new(false).el());

            // Reading the layout memo builds/lays out the tree; no rendering is
            // needed. (Rendering a `Label` in the null theme panics on the
            // unset text color — the same pre-existing limitation as
            // `draw_on_demand` — and the click logic under test runs entirely in
            // `handle_events`, independent of rendering.)
            let pt = page.layout.with(|m| m.tree_root().outer.center());

            // Pointer hovers the checkbox, then settle any hover-driven redraw.
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::MouseMove(pt),
            )));
            page.use_renderer(|_| {});
            page.take_draw_calls();

            // Full left-button click (down then up) toggles the checkbox.
            let _ = page.handle_events(
                [
                    Event::Mouse(MouseEvent::ButtonDown(
                        MouseButton::Left,
                        Some(pt),
                    )),
                    Event::Mouse(MouseEvent::ButtonUp(
                        MouseButton::Left,
                        Some(pt),
                    )),
                ]
                .into_iter(),
            );

            page.use_renderer(|_| {});
            assert_eq!(
                page.take_draw_calls(),
                1,
                "a mouse click must trigger a redraw of the checkbox"
            );
        });
    }

    // The click action fires once, on the release edge, and only when the
    // release lands on the same widget that received the press (the globally
    // tracked `pointer.pressed`). This is the release-edge semantics that
    // replaced the per-widget press bool.
    #[test]
    fn button_click_fires_on_release_not_press() {
        use crate::event::{Event, MouseButton, MouseEvent};

        with_new_runtime(|_| {
            let mut clicks = create_signal(0u32);
            let mut page = create_null_page(
                Button::new(Label::new("x"))
                    .on_click(move || clicks.update(|c| *c += 1))
                    .el(),
            );

            // Reading the layout memo builds/lays out the tree; no rendering is
            // needed. (Rendering a `Label` in the null theme panics on the
            // unset text color — the same pre-existing limitation as
            // `draw_on_demand` — and the click logic under test runs entirely in
            // `handle_events`, independent of rendering.)
            let pt = page.layout.with(|m| m.tree_root().outer.center());
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::MouseMove(pt),
            )));

            // Press alone must NOT fire the click.
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::ButtonDown(MouseButton::Left, Some(pt)),
            )));
            assert_eq!(clicks.get(), 0, "click must not fire on press-down");
            assert_eq!(
                page.state.pointer.pressed,
                Some(page.root),
                "press-down claims the global press target"
            );

            // Release on the same widget fires the click exactly once and
            // clears the global press state.
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::ButtonUp(MouseButton::Left, Some(pt)),
            )));
            assert_eq!(clicks.get(), 1, "click fires once on release");
            assert_eq!(
                page.state.pointer.pressed, None,
                "press is cleared after release"
            );
        });
    }

    // Press on the widget then release outside its bounds (drag-off) must NOT
    // fire the click, but the global press state is still cleared.
    #[test]
    fn button_click_cancelled_when_released_outside() {
        use crate::event::{Event, MouseButton, MouseEvent};

        with_new_runtime(|_| {
            let mut clicks = create_signal(0u32);
            let mut page = create_null_page(
                Button::new(Label::new("x"))
                    .on_click(move || clicks.update(|c| *c += 1))
                    .el(),
            );

            // Reading the layout memo builds/lays out the tree; no rendering is
            // needed. (Rendering a `Label` in the null theme panics on the
            // unset text color — the same pre-existing limitation as
            // `draw_on_demand` — and the click logic under test runs entirely in
            // `handle_events`, independent of rendering.)
            let pt = page.layout.with(|m| m.tree_root().outer.center());
            let outside = Point::new(10_000, 10_000);

            let _ = page.handle_events(
                [
                    Event::Mouse(MouseEvent::MouseMove(pt)),
                    Event::Mouse(MouseEvent::ButtonDown(
                        MouseButton::Left,
                        Some(pt),
                    )),
                    Event::Mouse(MouseEvent::ButtonUp(
                        MouseButton::Left,
                        Some(outside),
                    )),
                ]
                .into_iter(),
            );

            assert_eq!(
                clicks.get(),
                0,
                "release outside bounds must not click"
            );
            assert_eq!(
                page.state.pointer.pressed, None,
                "press state is cleared on release even when released outside"
            );
        });
    }

    // Pointer capture: while a widget holds the pointer, mouse moves are routed
    // to it exclusively — hover is frozen and the cursor is tracked even far
    // outside the widget's bounds. Releasing ends the capture and resumes
    // normal hit-testing.
    #[test]
    fn capture_freezes_hover_and_tracks_cursor_outside() {
        use crate::event::{Event, MouseButton, MouseEvent};

        with_new_runtime(|_| {
            let mut page = create_null_page(Checkbox::new(false).el());
            let cb = page.root;

            let pt = page.layout.with(|m| m.tree_root().outer.center());
            let outside = Point::new(10_000, 10_000);

            // Hover the checkbox.
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::MouseMove(pt),
            )));
            assert_eq!(
                page.state.pointer.hovered,
                Some(cb),
                "hovered before press"
            );

            // Press to capture the pointer, then move the cursor far outside.
            let _ = page.handle_events(
                [
                    Event::Mouse(MouseEvent::ButtonDown(
                        MouseButton::Left,
                        Some(pt),
                    )),
                    Event::Mouse(MouseEvent::MouseMove(outside)),
                ]
                .into_iter(),
            );

            assert_eq!(
                page.state.pointer.captured_by,
                Some(cb),
                "press captures the pointer"
            );
            // Capture freezes hover: moving outside does NOT clear it.
            assert_eq!(
                page.state.pointer.hovered,
                Some(cb),
                "hover is frozen while captured"
            );
            // The move was still processed — the captured widget follows the
            // cursor even outside its own bounds.
            assert_eq!(page.state.pointer.pos, Some(outside));

            // Releasing (even outside) ends the capture.
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::ButtonUp(MouseButton::Left, Some(outside)),
            )));
            assert_eq!(
                page.state.pointer.captured_by, None,
                "release ends capture"
            );

            // With capture ended, moves resume normal hit-testing (the cursor
            // is outside, so nothing is hovered).
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::MouseMove(outside),
            )));
            assert_eq!(
                page.state.pointer.hovered, None,
                "hover resumes after capture ends"
            );
        });
    }

    // Reproduce the widget_gallery structure: the checkbox lives inside a
    // `dynamic(...)` factory (built in a `create_effect`, held in a Signal,
    // inserted via `set_single_child`). This is the real-world embedding.
    #[test]
    fn dynamic_checkbox_redraws_on_toggle() {
        use crate::el::ElId;
        use crate::event::{Event, PressEvent};

        fn find_checkbox<W: WidgetCtx>(
            arena: &ElArena<W>,
            id: ElId,
        ) -> Option<ElId> {
            if arena.expect(id).map_or(false, |d| {
                d.state.flags.focusable && d.state.flags.clickable
            }) {
                return Some(id);
            }
            for &child in arena.children(id).unwrap_or(&[]) {
                if let Some(found) = find_checkbox(arena, child) {
                    return Some(found);
                }
            }
            None
        }

        with_new_runtime(|_| {
            // Factory reads an external signal (like widget_gallery's tab
            // signal), so the checkbox's own signals are created inside the
            // tracking effect.
            let tab = create_signal(0u8);
            let mut page = create_null_page(dynamic(move || {
                let _ = tab.get();
                Checkbox::new(false)
            }));

            for _ in 0..4 {
                page.use_renderer(|_| {});
            }
            page.take_draw_calls();
            page.use_renderer(|_| {});
            assert_eq!(
                page.take_draw_calls(),
                0,
                "page must be quiescent before the toggle"
            );

            let checkbox_id = page
                .arena
                .with(|arena| find_checkbox(arena, page.root))
                .expect("checkbox must be in the tree");
            page.state.focused = Some((checkbox_id, 0));
            let _ = page.handle_events(
                [
                    Event::Press(PressEvent::Press),
                    Event::Press(PressEvent::Release),
                ]
                .into_iter(),
            );

            page.use_renderer(|_| {});
            assert_eq!(
                page.take_draw_calls(),
                1,
                "toggling a checkbox inside dynamic() must trigger a redraw"
            );
        });
    }

    // A renderer that records primitive draw calls. NullRenderer is a no-op and
    // cannot reveal whether a primitive (e.g. the check-icon path) was actually
    // drawn.
    mod recording_renderer {
        use crate::prelude::*;
        use alloc::rc::Rc;
        use core::cell::Cell;

        #[derive(Clone, Default)]
        pub struct RecordingRenderer {
            pub paths: Rc<Cell<usize>>,
        }

        impl RenderTarget for RecordingRenderer {
            type Color = NullColor;
            fn draw(
                &mut self,
                _pixels: impl Iterator<
                    Item = crate::render::output::pixel::Pixel<Self::Color>,
                >,
            ) {
            }
        }

        impl<C> FinishRender<C> for RecordingRenderer {
            fn finish_frame(
                &mut self,
                _target: &mut impl RenderTarget<Color = C>,
            ) {
            }
        }

        impl Renderer for RecordingRenderer {
            type Color = NullColor;
            type Options = ();

            fn set_options(&mut self, _options: Self::Options) {}
            fn size(&self) -> Size {
                Size::new_equal(64)
            }
            fn clipped(
                &mut self,
                _area: Rect,
                f: impl FnOnce(&mut Self) -> RenderResult,
            ) -> RenderResult {
                f(self)
            }
            fn fill_solid(
                &mut self,
                _rect: Rect,
                _color: Self::Color,
            ) -> RenderResult {
                Ok(())
            }
            fn pixel(
                &mut self,
                _point: Point,
                _color: Self::Color,
            ) -> RenderResult {
                Ok(())
            }
            fn line(
                &mut self,
                _from: Point,
                _to: Point,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn rect(
                &mut self,
                _rect: Rect,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn rounded_rect(
                &mut self,
                _rect: Rect,
                _corners: CornerRadii,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn circle(
                &mut self,
                _top_left: Point,
                _diameter: u32,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn arc(
                &mut self,
                _top_left: Point,
                _diameter: u32,
                _start: Angle,
                _sweep: Angle,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn ellipse(
                &mut self,
                _bounding_box: Rect,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn sector(
                &mut self,
                _top_left: Point,
                _diameter: u32,
                _start: Angle,
                _sweep: Angle,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn polygon(
                &mut self,
                _points: &[Point],
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn path(
                &mut self,
                _path: &Path,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                self.paths.set(self.paths.get() + 1);
                Ok(())
            }
            fn image<'a>(
                &mut self,
                _image: crate::render::image::DrawImage<'a, Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
        }
    }

    // The exact user scenario, end-to-end: a checkbox that starts UNCHECKED,
    // then is toggled to checked via a click. The resulting redraw must
    // actually draw the check-icon path (the NullRenderer-based redraw tests
    // only prove the page re-renders, not that the icon is drawn).
    #[test]
    fn toggling_checkbox_to_checked_draws_icon() {
        use crate::event::{Event, PressEvent};
        use recording_renderer::RecordingRenderer;

        type RecWtf = Wtf<RecordingRenderer, (), (), ()>;

        with_new_runtime(|_| {
            let renderer = RecordingRenderer::default();
            let paths = renderer.paths.clone();
            let arena = create_signal(ElArena::new()).name("Page arena");
            let mut page: Page<RecWtf> = Page::new(
                (),
                Checkbox::<RecWtf>::new(false),
                arena,
                Size::new_equal(64).maybe_reactive(),
                ().inert(),
                DevTools::default().signal(),
                renderer.signal(),
                FontCtx::new().signal(),
            );

            // Settle; value is false, so no icon is drawn yet.
            for _ in 0..4 {
                page.use_renderer(|_| {});
            }
            let before = paths.get();

            // Click (press + release) the focused checkbox -> toggle to checked.
            page.state.focused = Some((page.root, 0));
            let _ = page.handle_events(
                [
                    Event::Press(PressEvent::Press),
                    Event::Press(PressEvent::Release),
                ]
                .into_iter(),
            );

            page.use_renderer(|_| {});
            let after = paths.get();

            assert!(
                after > before,
                "toggling a checkbox to checked must draw the icon on the \
                 redraw (path calls before={before}, after={after})"
            );
        });
    }

    // Same as above but the checkbox is NESTED (inside `dynamic`, like
    // widget_gallery). Reproduces the real bug: the page re-renders on toggle,
    // but the nested checkbox's render observer is skipped so the icon is never
    // drawn.
    #[test]
    fn nested_checkbox_toggle_draws_icon() {
        use crate::el::ElId;
        use crate::event::{Event, PressEvent};
        use recording_renderer::RecordingRenderer;

        type RecWtf = Wtf<RecordingRenderer, (), (), ()>;

        fn find_checkbox(arena: &ElArena<RecWtf>, id: ElId) -> Option<ElId> {
            if arena.expect(id).map_or(false, |d| {
                d.state.flags.focusable && d.state.flags.clickable
            }) {
                return Some(id);
            }
            for &child in arena.children(id).unwrap_or(&[]) {
                if let Some(found) = find_checkbox(arena, child) {
                    return Some(found);
                }
            }
            None
        }

        with_new_runtime(|_| {
            let renderer = RecordingRenderer::default();
            let paths = renderer.paths.clone();
            let arena = create_signal(ElArena::new()).name("Page arena");
            let mut page: Page<RecWtf> = Page::new(
                (),
                dynamic(|| Checkbox::<RecWtf>::new(false)),
                arena,
                Size::new_equal(64).maybe_reactive(),
                ().inert(),
                DevTools::default().signal(),
                renderer.signal(),
                FontCtx::new().signal(),
            );

            for _ in 0..4 {
                page.use_renderer(|_| {});
            }
            let before = paths.get();

            let checkbox_id = page
                .arena
                .with(|arena| find_checkbox(arena, page.root))
                .expect("checkbox must be in the tree");
            page.state.focused = Some((checkbox_id, 0));
            let _ = page.handle_events(
                [
                    Event::Press(PressEvent::Press),
                    Event::Press(PressEvent::Release),
                ]
                .into_iter(),
            );

            page.use_renderer(|_| {});
            let after = paths.get();

            assert!(
                after > before,
                "toggling a NESTED checkbox must draw the icon on the redraw \
                 (path calls before={before}, after={after})"
            );
        });
    }

    // The `View` migration: `row!`/`col!` and `impl View<W>` APIs accept bare
    // widgets *and* leaf values (`&str`, `String`, `Option<View>`, existing
    // `El`) uniformly, without an explicit `.el()`. A bare `Button` in `row!`
    // did not compile under the old `Into<El>` path (no `From<Button> for El`);
    // it works now because every widget gets its own concrete `View` impl
    // (a one-liner next to its `Widget` impl), which coexists with the leaf
    // impls.
    #[test]
    fn view_accepts_bare_widgets_and_leaves() {
        fn build(v: impl View<NullWtf>) -> El<NullWtf> {
            v.into_el()
        }

        let _: El<NullWtf> = build(Flex::row((
            Button::new("bare button"), // bare widget, no `.el()`
            "string literal",           // &str leaf
            String::from("owned string"), // String leaf
            Container::new("nested str"), // container takes `impl View`
            Label::new("explicit").el(), // existing El still fine
            Some(Button::new("optional")), // Option<View>
        )));

        let _: El<NullWtf> = build(Flex::col((Button::new("a"), "b")));

        // And it composes as a real page root (`PageInitFn` is `View`-based).
        let _ = create_null_page(Flex::row((Button::new("root"), "title")));
    }

    // ViewSequence: Flex accepts a heterogeneous tuple of widget types, a
    // homogeneous widget array, a Vec of views, and the row!/col! macros,
    // auto-erasing each element via `View::into_el` (no manual `.el()`), with
    // static children stored inert (`MaybeSignal::new_inert`).
    #[test]
    fn flex_view_sequence() {
        // Heterogeneous tuple of different widget types + a `&str` leaf:
        let _ = create_null_page(Flex::row((
            Button::new("a"),
            "b",
            Container::new("c"),
        )));
        // Homogeneous array of widgets:
        let _ =
            create_null_page(Flex::col([Button::new("x"), Button::new("y")]));
        // Vec of views (here `&str`):
        let _ = create_null_page(Flex::row(alloc::vec!["a", "b", "c"]));
        // Macros still accept bare widgets + leaves:
        let _ = create_null_page(Flex::col((
            Container::new("x"),
            "y",
            Button::new("z"),
        )));
    }

    // Regression: a reactive source set through the trait-default setter
    // (`SizedWidget::width` -> `self.layout_mut().setter(...)`) must persist
    // the reactive-on-write upgrade in the widget's own `Layout`.
    // Previously the upgrade landed on a discarded `self.layout()` copy,
    // which disposed the inert id and panicked when the layout was later
    // read.
    #[test]
    fn reactive_width_setter_persists_and_reacts() {
        use crate::{layout::length::Length, widget::edge::Edge};

        with_new_runtime(|_| {
            let mut w = create_signal(Length::fill());
            let edge = Edge::<NullWtf>::new().width(w);
            let layout = edge.layout();

            // Reading the layout must not panic (the disposed-id bug) and must
            // be tracked so observers re-run on change.
            let mut runs = create_signal(0u32);
            create_effect(move |_| {
                runs.update_untracked(|r| *r += 1);
                layout.with(|l| {
                    let _ = l.size.width();
                });
            });
            assert_eq!(runs.get_untracked(), 1);

            w.set(Length::Fixed(50));
            assert_eq!(runs.get_untracked(), 2);
        });
    }

    // Same guarantee for `FontSettingWidget::font_size` on a `Label` (whose
    // Text layout owns `FontProps`). Font props are now plain data;
    // reactivity flows through the `Layout` signal driven by the setter.
    #[test]
    fn reactive_font_size_setter_persists_and_reacts() {
        use crate::font::FontSize;

        with_new_runtime(|_| {
            let mut fs = create_signal(FontSize::Fixed(10));
            let label = Label::<NullWtf>::new("x").font_size(fs);
            let layout = label.layout();

            let mut runs = create_signal(0u32);
            create_effect(move |_| {
                runs.update_untracked(|r| *r += 1);
                layout.with(|l| {
                    let _ = l.font_props().map(|fp| fp.font_size);
                });
            });
            assert_eq!(runs.get_untracked(), 1);

            fs.set(FontSize::Fixed(20));
            assert_eq!(runs.get_untracked(), 2);
        });
    }
}
