use crate::{
    el::{El, ElId},
    event::{
        Capture, Event, EventResponse, FocusEvent, MouseEvent, Propagate,
        UnhandledEvent,
    },
    font::{Font, FontCtx, FontProps},
    layout::{LayoutCtx, LayoutModel, Limits, model_layout, size::Size},
    render::{
        Renderer,
        color::{Color, MapColor},
        framebuf::PackedColor,
    },
    style::TreeStyle,
    widget::{
        Behavior, DrawCtx, EventCtx, MountCtx, PageState, Widget, WidgetCtx,
    },
};
use alloc::{boxed::Box, vec::Vec};
use dev::{DevHoveredEl, DevTools};
use embedded_graphics::{
    Drawable as _,
    prelude::{DrawTarget, Point},
};
use num::traits::WrappingAdd as _;
use rsact_reactive::{
    maybe::IntoMaybeReactive, prelude::*, scope::new_deny_new_scope,
};

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
struct PageMeta {
    // /// Page elements meta tree
    // meta: MetaTree,
    /// Count of focusable elements
    focusable: Memo<Vec<ElId>>,
}

pub struct Page<W: WidgetCtx> {
    // TODO: root is not used as a Signal but as boxed value, better add StoredValue to rsact_reactive for static storage
    // TODO: Same is about other not-really-reactive states in Page
    root: Signal<El<W>>,
    meta: PageMeta,
    layout: Memo<LayoutModel>,
    state: Signal<PageState<W>>,
    style: Signal<PageStyle<W::Color>>,
    renderer: Signal<W::Renderer>,
    viewport: Memo<Size>,
    dev_tools: Signal<DevTools>,
    force_redraw: Signal<bool>,
    drawing: Memo<(bool, usize)>,
    draw_calls: Signal<usize>,
}

impl<W: WidgetCtx> Page<W> {
    pub(crate) fn new(
        root: impl Into<El<W>>,
        viewport: Memo<Size>,
        styler: Memo<W::Styler>,
        dev_tools: Signal<DevTools>,
        mut renderer: Signal<W::Renderer>,
        fonts: Signal<FontCtx>,
    ) -> Self {
        let mut root: El<W> = root.into();
        let state = PageState::new().signal();

        // Raw root initialization //
        root.on_mount(MountCtx {
            viewport,
            styler,
            inherit_font_props: FontProps {
                font: Some(Font::Auto.maybe_reactive()),
                font_size: None,
                font_style: None,
            },
        });

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
        let layout_tree = root.layout();
        let layout_model = map!(move |viewport, fonts| {
            let viewport = *viewport;
            // println!("Relayout");
            // TODO: Possible optimization is to use previous memo result, pass
            // it to model_layout as tree and don't relayout parents if layouts
            // inside Fixed-sized container changed, returning previous result
            let layout = model_layout(
                &LayoutCtx { fonts, viewport },
                layout_tree.memo(),
                Limits::only_max(viewport),
                viewport.into(),
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

        let drawing = create_memo(move |prev| {
            // TODO: force_redraw must be placed into ui context and be available in widgets so some widget can request redraw
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
                                renderer.clear(background_color)
                            } else {
                                Ok(())
                            }
                        })
                        .ok()
                        .unwrap();

                    // TODO: How to handle results?
                    let _result = with!(|layout_model, fonts| {
                        // FIXME: This might be wrong. User possibly want to create new reactive values. Better make it a debug feature.
                        let _deny_new = new_deny_new_scope();
                        root.update_untracked(|root| {
                            root.draw(&mut DrawCtx {
                                state,
                                renderer,
                                layout: &layout_model.tree_root(),
                                tree_style: TreeStyle::base(),
                                viewport,
                                fonts,
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

            // Draw tag is just count of draw calls
            // TODO: Review if in case of `take_draw_calls` usage, the tag could overlap with previous one. But we only check for equality of current and previous `draw_calls` so it seem to never be equal as we place 0 into `draw_calls` when take it.
            let tag = draw_calls.update(|draw_calls| {
                *draw_calls = draw_calls.wrapping_add(&1);
                *draw_calls
            });

            (prev.map(|(_, prev_tag)| tag != *prev_tag).unwrap_or(true), tag)
        });

        Self {
            root,
            layout: layout_model,
            state,
            style,
            meta: PageMeta { focusable },
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
        // Useless check, focus_el already checks
        // if self.meta.focusable.with(|focusable| !focusable.is_empty()) {
        self.focus_el(0);
        // }
    }

    /// Focus first focusable element in page if no element focused
    pub fn apply_auto_focus(&mut self) {
        if self.state.with(|state| state.focused.is_none()) {
            self.focus_first();
        }
    }

    /// Find element to focus by offset from currently focused element index.
    fn find_focus(&mut self, offset: i32) -> Option<(ElId, usize)> {
        let focusable_count = self.meta.focusable.with(Vec::len);

        let current_offset = self.state.with(|state| {
            state.focused.as_ref().map(|focused| focused.1).unwrap_or(0)
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

    fn apply_focus(&mut self, new_focus: (ElId, usize)) {
        self.state.update(|state| state.focused = Some(new_focus))
    }

    /// Send focus event to tree. Sets new focus if event was captured
    fn focus_el(&mut self, offset: i32) -> Option<UnhandledEvent<W>> {
        if let Some(new_focus) = self.find_focus(offset) {
            let response =
                self.send_event(Event::Focus(FocusEvent::Focus(new_focus.0)));

            if response.is_none() {
                self.apply_focus(new_focus);
            }

            response
        } else {
            None
        }
    }

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
        if let Some(focus_offset) = unhandled.interpret_as_focus_move() {
            // Note: Focus event is eaten here, even if no element focused. This might be incorrect
            return self.focus_el(focus_offset);
        }

        Some(UnhandledEvent::Event(unhandled))
    }

    #[must_use]
    fn send_specific_event(
        &mut self,
        event: &Event<W::CustomEvent>,
    ) -> EventResponse {
        self.layout.with(|layout| {
            let response = self.root.update_untracked(|root| {
                root.on_event(&mut EventCtx {
                    event,
                    // TODO: Maybe state should not be changeable in on_event, pass it by reference
                    page_state: self.state,
                    layout: &layout.tree_root(),
                })
            });

            // TODO: notify root on event capture?
            //  - No, root is not used reactively, it is a signal only to be usable in reactive contexts. Need `StoredValue`

            response
        })
    }

    #[must_use]
    fn send_event(
        &mut self,
        event: Event<W::CustomEvent>,
    ) -> Option<UnhandledEvent<W>> {
        // Global, page-level event handling //
        if self.dev_tools.with(|dt| dt.enabled) {
            if let Event::Mouse(MouseEvent::MouseMove(point)) = event {
                let hovered_el = self.find_hovered_el(point);
                self.dev_tools.update(|dev_tools| {
                    dev_tools.hovered = hovered_el;
                });
                return None;
            }
        }

        // Element event handling //
        let response = self.send_specific_event(&event);

        match response {
            EventResponse::Continue(propagate) => match propagate {
                Propagate::Ignored => self.on_unhandled_event(event),
            },
            EventResponse::Break(capture) => match capture {
                // TODO: Captured data may be useful for debugging, for example we can point where on screen user clicked or something
                Capture::Captured(_capture) => None,
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

    pub fn draw(
        &mut self,
        target: &mut impl DrawTarget<Color = W::Color>,
    ) -> bool {
        if self.drawing.get().0 {
            self.renderer.with(|renderer| renderer.draw(target)).ok().unwrap();
            true
        } else {
            false
        }
    }

    pub fn draw_with_renderer(&self, f: impl FnOnce(&W::Renderer)) -> bool {
        if self.drawing.get().0 {
            self.renderer.with(|renderer| {
                f(renderer);
            });

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
        el::El,
        font::FontCtx,
        prelude::{Edge, Size, Text},
        render::{NullDrawTarget, NullRenderer},
        style::NullStyler,
        widget::{Widget, WidgetCtx, Wtf},
    };
    use alloc::string::String;
    use embedded_graphics::pixelcolor::BinaryColor;
    use rsact_reactive::{
        maybe::IntoInert,
        memo::{IntoMemo, create_memo},
        signal::IntoSignal,
        write::WriteSignal,
    };

    type NullWtf = Wtf<NullRenderer, NullStyler, (), ()>;

    fn create_null_page(root: impl Into<El<NullWtf>>) -> Page<NullWtf> {
        Page::new(
            root,
            create_memo(|_| Size::new_equal(1)),
            NullStyler::default().inert().memo(),
            DevTools::default().signal(),
            NullRenderer::default().signal(),
            FontCtx::new().signal(),
        )
    }

    #[test]
    fn draw_on_demand() {
        let mut redraw_signal_data = String::new().signal();

        let mut page = create_null_page(Text::new(redraw_signal_data).el());

        assert_eq!(page.take_draw_calls(), 0);

        // First draw request without changes subscribes to reactive values inside drawing context.
        page.draw(&mut NullDrawTarget::default());
        assert_eq!(page.take_draw_calls(), 1);

        // Nothing changed inside drawing context
        page.draw(&mut NullDrawTarget::default());
        assert_eq!(page.take_draw_calls(), 0);

        // Something's changed
        redraw_signal_data.update(|string| string.push_str("kek"));
        page.draw(&mut NullDrawTarget::default());
        assert_eq!(page.take_draw_calls(), 1);
    }
}
