use crate::{
    el::{El, ElId},
    event::{
        dev::DevElHover as _, Capture, EventResponse, FocusEvent, Propagate,
        UnhandledEvent,
    },
    layout::{model_layout, size::Size, LayoutModel, Limits},
    render::{color::Color, Renderer},
    style::TreeStyle,
    widget::{
        Behavior, DrawCtx, EventCtx, FocusedEl, MountCtx, PageState, Widget,
        WidgetCtx,
    },
};
use alloc::vec::Vec;
use dev::{DevHoveredEl, DevTools};
use embedded_graphics::prelude::{DrawTarget, Point};
use num::traits::WrappingAdd as _;
use rsact_reactive::{prelude::*, runtime::new_deny_new_scope};

pub mod dev;
pub mod id;

pub struct PageStyle<C: Color> {
    // TODO: Use ColorStyle
    background_color: Option<C>,
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

/// Tree of info about widget tree.
struct PageTree {
    // /// Page elements meta tree
    // meta: MetaTree,
    /// Count of focusable elements
    focusable: Memo<Vec<ElId>>,
}

pub struct Page<W: WidgetCtx> {
    // TODO: root is not used as a Signal but as boxed value, better add StoredValue to rsact_reactive for static storage
    // TODO: Same is about other not-really-reactive states in Page
    root: Signal<El<W>>,
    meta: PageTree,
    layout: Memo<LayoutModel>,
    state: Signal<PageState<W>>,
    style: Signal<PageStyle<W::Color>>,
    renderer: Signal<W::Renderer>,
    viewport: Memo<Size>,
    dev_tools: Signal<DevTools>,
    force_redraw: Signal<bool>,
    drawing: Memo<bool>,
    draw_calls: Signal<usize>,
}

impl<W: WidgetCtx> Page<W> {
    pub(crate) fn new(
        root: impl Into<El<W>>,
        viewport: Memo<Size>,
        styler: Memo<W::Styler>,
        dev_tools: Signal<DevTools>,
        mut renderer: Signal<W::Renderer>,
    ) -> Self {
        let mut root: El<W> = root.into();
        let state = PageState::new().signal();

        // Raw root initialization //
        root.on_mount(MountCtx { viewport, styler });

        // TODO: `on_mount` dependency on viewport can be removed for text,
        //  icon, etc. by adding special LayoutKind dependent on viewport and
        //  pass viewport to model_layout. In such a way, layout becomes much
        //  more straightforward and single-pass process.
        let meta = root.meta();

        let focusable = create_memo(move |_| {
            meta.flat_collect()
                .iter()
                .filter_map(|el| {
                    el.with(|el_meta| {
                        if let Some(id) = el_meta.id {
                            if !(el_meta.behavior & Behavior::FOCUSABLE)
                                .is_empty()
                            {
                                return Some(id);
                            }
                        }
                        None
                    })
                })
                .collect()
        });

        // TODO: Should be `mapped`? Now, root is kind of partially-reactive
        let layout_tree = root.build_layout_tree();
        let layout = viewport.map(move |&viewport_size| {
            // println!("Relayout");
            // TODO: Possible optimization is to use previous memo result, pass
            // it to model_layout as tree and don't relayout parents if layouts
            // inside Fixed-sized container changed, returning previous result
            let layout = model_layout(
                layout_tree,
                Limits::only_max(viewport_size),
                viewport_size.into(),
                // viewport,
            );

            // std::println!("Relayout {:#?}", layout.tree_root());

            layout
        });

        let style = PageStyle::base().signal();

        let mut draw_calls = create_signal(0);
        let mut force_redraw = create_signal(false);

        // Now root is boxed //
        let mut root = root.signal();

        let drawing = create_memo(move |_| {
            if force_redraw.get() {
                force_redraw.set_untracked(false);
            }

            with!(|state| {
                renderer.update_untracked(|renderer| {
                    // FIXME: Performance?
                    // TODO: Not only performance, this is very wrong for Canvas widget, as this clear also clears all canvases which should be manually controlled and cleared. This needs to be solved (also check Canvas and animations after any change). I think that Widget Behavior can have some flag such as "auto_clear" which will clear its layout rect before redraw. But this complicates absolutely positioned elements a lot as we need to clear them too but then elements overlapped by it won't be cleared!
                    style
                        .with(|style| {
                            if let Some(background_color) =
                                style.background_color
                            {
                                Renderer::clear(renderer, background_color)
                            } else {
                                Ok(())
                            }
                        })
                        .unwrap();

                    // TODO: How to handle results?
                    let _result = layout
                        .with(|layout| {
                            let _deny_new = new_deny_new_scope();
                            root.update_untracked(|root| {
                                root.draw(&mut DrawCtx {
                                    state,
                                    renderer,
                                    layout: &layout.tree_root(),
                                    tree_style: TreeStyle::base(),
                                })
                            })
                        })
                        .unwrap();

                    with!(|dev_tools| {
                        if dev_tools.enabled {
                            if let Some(hovered) = &dev_tools.hovered {
                                hovered.draw(renderer, viewport.get()).unwrap();
                            }
                        }
                    });
                })
            });

            draw_calls
                .update(|draw_calls| *draw_calls = draw_calls.wrapping_add(&1));

            true
        });

        Self {
            root,
            layout,
            state,
            style,
            meta: PageTree { focusable },
            // TODO: Signal viewport in Renderer
            renderer,
            viewport,
            dev_tools,
            force_redraw,
            drawing,
            draw_calls,
        }
    }

    pub(crate) fn force_redraw(&mut self) {
        self.force_redraw.set(true);
    }

    pub fn take_draw_calls(&mut self) -> usize {
        let draw_calls = self.draw_calls.get();
        self.draw_calls.set(0);
        draw_calls
    }

    // TODO
    // pub fn style(
    //     mut self,
    //     style: impl IntoSignal<PageStyle<C::Color>>,
    // ) -> Self {
    //     self.style = style.signal();
    //     self
    // }

    // Focus //

    /// Focus first focusable element in page
    pub fn focus_first(&mut self) {
        if self.meta.focusable.with(|focusable| focusable.len() > 0) {
            self.handle_events([<W::Event as FocusEvent>::zero()].into_iter());
        }
    }

    /// Focus first focusable element in page if no element focused
    pub fn auto_focus(&mut self) {
        if self.state.with(|state| state.focused.is_none()) {
            self.focus_first();
        }
    }

    fn find_focus(&mut self, offset: i32) -> Option<(ElId, usize)> {
        let focusable_count = self.meta.focusable.with(Vec::len);

        let current_offset = self.state.with(|state| {
            state.focused.as_ref().map(|focused| focused.index).unwrap_or(0)
        });

        let new_focus_offset = (current_offset as i64 + offset as i64)
            .clamp(0, focusable_count as i64)
            as usize;

        let new_focus_id = self
            .meta
            .focusable
            .with(|focusable| focusable.get(new_focus_offset).copied());

        // Set new focus only in case there's a corresponding element by index. Otherwise it means buggy meta collection
        if let Some(new_focus_id) = new_focus_id {
            Some((new_focus_id, new_focus_offset))
        } else {
            None
        }
    }

    fn apply_focus(&mut self, focus: FocusedEl) {
        self.state.update(|state| state.focused = Some(focus))
    }

    // Dev tools //
    fn find_hovered_el(&self, point: Point) -> Option<DevHoveredEl> {
        self.layout.with(|layout| {
            layout
                .tree_root()
                .dev_hover(point)
                .map(|layout| DevHoveredEl { layout })
        })
    }

    pub fn handle_events(
        &mut self,
        events: impl Iterator<Item = W::Event>,
    ) -> Vec<UnhandledEvent<W>> {
        let unhandled = events
            .filter_map(|event| {
                // Global, page-level event handling //
                if self.dev_tools.with(|dt| dt.enabled) {
                    if let Some(point) = event.as_dev_el_hover() {
                        let hovered_el = self.find_hovered_el(point);
                        self.dev_tools.update(|dev_tools| {
                            dev_tools.hovered = hovered_el;
                        });
                        return None;
                    }
                }

                let next_focus = if let Some(next_focus) = event
                    .as_focus_move()
                    .map(|offset| self.find_focus(offset))
                    .flatten()
                {
                    self.state.update(|state| {
                        state.next_focus = Some(next_focus.0);
                    });

                    Some(next_focus)
                } else {
                    None
                };

                // Element event handling //
                let response = self.layout.with(|layout| {
                    let response = self.root.update_untracked(|root| {
                        root.on_event(&mut EventCtx {
                            event: &event,
                            // TODO: Maybe state should not be changeable in on_event, pass it by reference
                            page_state: self.state,
                            layout: &layout.tree_root(),
                        })
                    });

                    // TODO: notify root on event capture?
                    //  - No, root is not used reactively, it is a signal only to be usable in reactive contexts. Need `StoredValue`

                    response
                });

                match response {
                    EventResponse::Continue(propagate) => match propagate {
                        Propagate::Ignored => {
                            Some(UnhandledEvent::Event(event))
                        },
                    },
                    // TODO: Maybe better merge bubble and capture as they have similar logic?
                    EventResponse::Break(capture) => match capture {
                        // TODO: Captured data may be useful for debugging, for example we can point where on screen user clicked or something
                        Capture::Captured(capture) => {
                            if let Some(next_focus) = next_focus {
                                self.state.update(|state| {
                                    state.focused = Some(FocusedEl {
                                        id: next_focus.0,
                                        index: next_focus.1,
                                        absolute_position: capture
                                            .absolute_position,
                                    });
                                    state.next_focus = None;
                                });
                            }
                            None
                        },
                        Capture::Bubble(data) => match data {
                            crate::event::BubbledData::FocusOffset(offset) => {
                                self.find_focus(offset)
                                    .map(|focus| self.apply_focus(focus));
                                None
                            },
                            crate::event::BubbledData::Custom(custom) => {
                                // Users receive only their custom bubbled data
                                Some(UnhandledEvent::Bubbled(custom))
                            },
                        },
                    },
                }
            })
            .collect();

        unhandled
    }

    pub fn draw(
        &mut self,
        target: &mut impl DrawTarget<Color = <W::Renderer as Renderer>::Color>,
    ) -> bool {
        if self.drawing.get() {
            self.renderer.with(|renderer| renderer.finish_frame(target));
            true
        } else {
            false
        }
    }
}
