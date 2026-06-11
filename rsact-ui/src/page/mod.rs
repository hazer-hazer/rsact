use crate::{
    el::{arena::ElArena, build::BuildCtx, *},
    event::{Capture, Event, EventResponse, MouseEvent, UnhandledEvent},
    font::{Font, FontCtx, FontProps},
    layout::{
        LayoutCtx, Limits,
        model::{LayoutModel, PPLayoutModel, model_layout},
    },
    render::prelude::*,
    style::{TreeStyle, theme::Theme},
    widget::{Behavior, Widget},
};
use alloc::vec::Vec;
use core::marker::PhantomData;
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

// TODO: As we now have element arena we can split functionality to add optimization structures. For example, not all drivers have focusing logic, so we can have it behind a generic or other abstraction to toggle, and pre-build focusable elements list only when it's needed. Same for hoverable, etc. Hoverable is kinda more interesting case because with element arena we can build spatial index like an R-Tree or just cache hit-tests which is also good.

pub struct Page<W: WidgetCtx> {
    // TODO: root is not used as a Signal but as boxed value, better add StoredValue to rsact_reactive for static storage
    // TODO: Same is about other not-really-reactive states in Page
    id: W::PageId,
    root: ElId,
    arena: Signal<ElArena<W>>,
    // TODO: MaybeReactive
    layout: Memo<LayoutModel>,
    // TODO: State must be a struct of signals, not signal of a struct.
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

impl<W: WidgetCtx> Page<W> {
    pub(crate) fn new(
        id: W::PageId,
        // TODO: Do we really need to accept Into<El> and expect it to always be a El::New, or we can require root to be a Widget non-wrapped?
        // No, ElData can contain additional information raw widget does not provide
        root: impl Into<El<W>>,
        arena: Signal<ElArena<W>>,
        viewport: MaybeReactive<Size>,
        stylist: Inert<W::Stylist>,
        dev_tools: Signal<DevTools>,
        renderer: Signal<W::Renderer>,
        fonts: Signal<FontCtx>,
    ) -> Self {
        let mut root: El<W> = root.into();
        let state = PageState::new();

        let mut force_redraw = create_signal(false).name("Force redraw");

        let root = BuildCtx::run(&mut root, arena);

        // // TODO: Multiple behaviors will lead to multiple memos, I am not sure what is more efficient, to recollect all behaviors when any changed or to store multiple memos.
        // let focusable = create_memo(move || {
        //     info!("Collect page {:?} meta", id);

        //     meta.flat_collect()
        //         .iter()
        //         .filter_map(|el_meta| {
        //             if let Some(id) = el_meta.id {
        //                 if !(el_meta.behavior & Behavior::FOCUSABLE).is_empty()
        //                 {
        //                     return Some(id);
        //                 }
        //             }
        //             None
        //         })
        //         .collect()
        // })
        // .name("Focusable");

        let layout_tree = arena.with(|arena| {
            arena
                .expect(root)
                .expect("Root node must be built")
                .widget
                .layout()
                .name("Layout tree")
        });
        // TODO: If we make fonts MaybeReactive, we can go fully MaybeReactive LayoutModel here
        let layout_model = map!(move |fonts, viewport| {
            info!("Relayout page {:?}", id);

            // TODO: Possible optimization is to use previous memo result, pass
            // it to model_layout as tree and don't relayout parents if layouts
            // inside Fixed-sized container changed, returning previous result

            let viewport = *viewport;
            let layout = model_layout(
                &LayoutCtx {
                    fonts,
                    viewport,
                    font_props: FontProps {
                        font: Some(Font::Auto.maybe_reactive()),
                        font_size: None,
                        font_style: None,
                    },
                },
                layout_tree,
                Limits::only_max(viewport),
                viewport.into(),
            );

            // TODO: Do we need full page redraw on layout change?
            // No, we need smart bottom-up propagation to the nearest fixed parent layout.
            force_redraw.set(true);

            debug!("{}", PPLayoutModel::root(&layout));

            layout
        })
        .name("Layout model");

        let style = PageStyle::base().signal().name("Page style");

        Self {
            id,
            root,
            arena,
            layout: layout_model,
            state,
            style,
            // TODO: Signal viewport in Renderer
            renderer,
            viewport: viewport.name("Viewport"),
            stylist,
            dev_tools,
            force_redraw,
            render_calls: 0,
            fonts,
        }
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
            // TODO: Will not work without background, must always have a background
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

    //     // Set new focus only in case there's a corresponding element by index. Otherwise it means buggy meta collection
    //     if let Some(new_focus_id) = new_focus_id {
    //         Some((new_focus_id, new_focus_offset))
    //     } else {
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
    //             self.send_event(Event::Focus(FocusEvent::Focus(new_focus.0)));

    //         if response.is_none() {
    //             self.apply_focus(new_focus);
    //         }

    //         response
    //     } else {
    //         None
    //     }
    // }

    /// For Dev tools
    fn find_hovered_el(&self, point: Point) -> Option<DevHoveredEl> {
        self.layout.with(|layout| {
            layout
                .tree_root()
                .dev_hover(point)
                .map(|layout| DevHoveredEl { layout })
        })
    }

    /// Apply global logic for unhandled events.
    /// Some events have different interpretations.
    /// For example `MoveEvent` can be treated as focus move, and if no element captured this event, we move the focus.
    fn on_unhandled_event(
        &mut self,
        unhandled: Event<W::CustomEvent>,
    ) -> Option<UnhandledEvent<W>> {
        // if let Some(focus_offset) = unhandled.interpret_as_focus_move() {
        //     // TODO: Focus event is eaten here, even if no element focused. This might be incorrect
        //     return self.focus_el(focus_offset);
        // }

        Some(UnhandledEvent::Event(unhandled))
    }

    #[must_use]
    fn send_specific_event(
        &mut self,
        event: &Event<W::CustomEvent>,
    ) -> EventResponse {
        // Note: Need to have special deferred reactive updates zone. Because if some child node depends on value it's children set, then there will be a BorrowRefMut error because children are borrowed mutably for update on events. This happens for example if flex layout contains a checkbox toggling this flex layout wrap.

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
            //  - No, root is not used reactively, it is a signal only to be usable in reactive contexts. Need `StoredValue`

            response
        });

        defer_effects.run();

        res
    }

    // TODO: Better have something like Widget::update for system events
    fn bubble_from_child(child: ElId, update: Update, arena: &mut ElArena<W>) {
        let Some(parent) = arena.parents.get(child).copied() else { return };
        Self::send_update_inner(parent, update, arena);
    }

    fn send_update_inner(id: ElId, update: Update, arena: &mut ElArena<W>) {
        let Some(el) = arena.expect_mut(id) else { return };

        el.widget.update(UpdateCtx { id, update, state: &mut el.state });
    }

    fn send_update(&mut self, id: ElId, update: Update) {
        self.arena.update_untracked(|arena| {
            Self::send_update_inner(id, update, arena);

            if let Some(bubble) = update.as_bubble() {
                Self::bubble_from_child(id, bubble, arena);
            }
        });
    }

    #[must_use]
    fn send_event(
        &mut self,
        event: Event<W::CustomEvent>,
    ) -> Option<UnhandledEvent<W>> {
        // MouseMove: consumed at page level — update cursor pos, run hover pass, dispatch Enter/Leave
        if let Event::Mouse(MouseEvent::MouseMove(point)) = event {
            // Update dev tools hover using layout geometry (unchanged)
            if self.dev_tools.with(|dt| dt.enabled) {
                let hovered_el = self.find_hovered_el(point);
                let dev_hovered_changed = self.dev_tools.update(|dt| {
                    let changed = dt.hovered != hovered_el;
                    dt.hovered = hovered_el;
                    changed
                });

                if dev_hovered_changed {
                    // TODO: Real rendering requires smarter dirty rectangles as dev tools are overlaying and have absolute position. Clearing whole screen is bad

                    self.force_redraw();
                }
                // TODO: Should dev tools capture mouse movement?
                // return None;
            }

            // Update persistent cursor position
            self.state.pointer.pos = Some(point);

            // Hover pass: dispatch MouseMove through the tree so HOVERABLE widgets can self-report
            let old_hovered = self.state.pointer.hovered;
            self.state.pointer.hovered = None;
            let _ = self.send_specific_event(&event);

            // Dispatch Enter/Leave if hovered widget changed
            let new_hovered = self.state.pointer.hovered;
            if old_hovered != new_hovered {
                if let Some(left) = old_hovered {
                    self.send_update(left, Update::MouseLeave);
                }
                if let Some(entered) = new_hovered {
                    self.send_update(entered, Update::MouseEnter);
                }
            }

            // TODO: Should mouse event always be ignored?
            return None;
        }

        // TODO: Capture mouse movement? - Yes, it is required for sliders, knobs, etc.
        // Pointer capture: route mouse button events directly to the capturing widget first
        let captured_by = self.state.pointer.captured_by;
        if let Some(_captured_id) = captured_by {
            if matches!(
                event,
                Event::Mouse(
                    MouseEvent::ButtonDown(_, _) | MouseEvent::ButtonUp(_, _)
                )
            ) {
                // Route through the normal tree — the capturing widget receives the
                // event via pass_to_children and uses its own drag state to handle it.
                let _ = self.send_specific_event(&event);

                // Clear capture on any ButtonUp
                if matches!(event, Event::Mouse(MouseEvent::ButtonUp(_, _))) {
                    self.state.pointer.captured_by = None;
                }

                return None;
            }
        }

        // Global, page-level event handling //
        let response = self.send_specific_event(&event);

        match response {
            EventResponse::Continue(()) => {
                info!(
                    "Event {:?} was ignored, applying global handling",
                    event
                );
                self.on_unhandled_event(event)
            },
            EventResponse::Break(capture) => match capture {
                // TODO: Captured data may be useful for debugging, for example we can point where on screen user clicked or something
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

        let drawn = observe(("render_page", self.id), || {
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
                        self.arena.with(|arena| {
                            RenderCtx {
                                id: self.root,
                                arena,
                                page_state: &self.state,
                                renderer,
                                layout: &layout.tree_root(),
                                tree_style: TreeStyle::base(),
                                page_style: self.style.read_only(),
                                viewport: self.viewport,
                                fonts,
                                stylist,
                                font_props: FontProps {
                                    font: Some(Font::Auto.maybe_reactive()),
                                    font_size: None,
                                    font_style: None,
                                },
                                force_redraw: self.force_redraw,
                                parent_dirty: false,
                                nesting_level: 0,
                                call: self.render_calls,
                                _marker: PhantomData::<CtxUnready>,
                            }.render()
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
                .ok()
                // TODO: What do we do with errors, huh?
                .unwrap();
        });

        //
        self.force_redraw.set_untracked(false);

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
        el::{El, arena::ElArena, ctx::*},
        font::FontCtx,
        prelude::*,
        style::theme::Theme,
        widget::Widget,
    };
    use alloc::string::String;
    use rsact_reactive::prelude::*;

    type NullWtf = Wtf<NullRenderer, (), (), ()>;

    fn create_null_page(root: impl Into<El<NullWtf>>) -> Page<NullWtf> {
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

    #[test]
    fn draw_on_demand() {
        let mut redraw_signal_data = String::new().signal();

        let mut page = create_null_page(Label::new(redraw_signal_data).el());

        assert_eq!(page.take_draw_calls(), 0);

        // First draw request without changes subscribes to reactive values inside drawing context.
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
}
