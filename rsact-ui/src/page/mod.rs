use crate::{
    el::{El, ElId},
    event::{
        Capture, Event, EventResponse, FocusEvent, MouseEvent, Propagate,
        UnhandledEvent,
    },
    font::{Font, FontCtx, FontProps},
    layout::{
        LayoutCtx, Limits,
        model::{LayoutModel, model_layout},
    },
    render::prelude::*,
    style::{TreeStyle, theme::Theme},
    widget::{Behavior, Widget, ctx::*},
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
    id: W::PageId,
    root: El<W>,
    meta: PageMeta,
    // TODO: MaybeReactive
    layout: Memo<LayoutModel>,
    state: Signal<PageState<W>>,
    style: Signal<PageStyle<W::Color>>,
    // TODO: Just use Rc<RefCell<R>> because we don't need to track renderer.
    renderer: Signal<W::Renderer>,
    viewport: MaybeReactive<Size>,
    theme: Inert<Theme<W::Color>>,
    dev_tools: Signal<DevTools>,
    force_redraw: Trigger,
    render_calls: usize,
    fonts: Signal<FontCtx>,
}

impl<W: WidgetCtx> Page<W> {
    pub(crate) fn new(
        id: W::PageId,
        root: impl Into<El<W>>,
        viewport: MaybeReactive<Size>,
        theme: Inert<Theme<W::Color>>,
        dev_tools: Signal<DevTools>,
        renderer: Signal<W::Renderer>,
        fonts: Signal<FontCtx>,
    ) -> Self {
        let root: El<W> = root.into();
        let state = PageState::new().signal().name("Page state");

        let force_redraw = create_trigger().name("Force redraw");

        let meta = root.meta(root.id());

        // TODO: Multiple behaviors will lead to multiple memos, I am not sure what is more efficient, to recollect all behaviors when any changed or to store multiple memos.
        let focusable = create_memo(move || {
            info!("Collect page {:?} meta", id);

            meta.flat_collect()
                .iter()
                .filter_map(|el_meta| {
                    if let Some(id) = el_meta.id {
                        if !(el_meta.behavior & Behavior::FOCUSABLE).is_empty()
                        {
                            return Some(id);
                        }
                    }
                    None
                })
                .collect()
        })
        .name("Focusable");

        let layout_tree = root.layout().name("Layout tree");
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
            force_redraw.notify();

            layout
        })
        .name("Layout model");

        let style = PageStyle::base().signal().name("Page style");

        Self {
            id,
            root,
            layout: layout_model,
            state,
            style,
            meta: PageMeta { focusable },
            // TODO: Signal viewport in Renderer
            renderer,
            viewport: viewport.name("Viewport"),
            theme,
            dev_tools,
            force_redraw,
            render_calls: 0,
            fonts,
        }
    }

    pub(crate) fn force_redraw(&mut self) -> &mut Self {
        info!("Force redraw page {:?}", self.id);
        self.force_redraw.notify();
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

    /// Focus first focusable element in page
    pub fn focus_first(&mut self) {
        self.focus_el(0);
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
        // Note: Need to have special deferred reactive updates zone. Because if some child node depends on value it's children set, then there will be a BorrowRefMut error because children are borrowed mutably for update on events. This happens for example if flex layout contains a checkbox toggling this flex layout wrap.

        let defer_effects = defer_effects();

        let res = self.layout.with(|layout| {
            let response = self.root.on_event(EventCtx {
                id: self.root.id(),
                event,
                // TODO: Maybe state should not be changeable in on_event, pass it by reference
                page_state: self.state,
                layout: &layout.tree_root(),
            });

            // TODO: notify root on event capture?
            //  - No, root is not used reactively, it is a signal only to be usable in reactive contexts. Need `StoredValue`

            response
        });

        defer_effects.run();

        res
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
                // TODO: Get rid of force redrawing the whole page when using dev_tools but use dirty rectangles
                self.force_redraw();
                return None;
            }
        }

        // Element event handling //
        let response = self.send_specific_event(&event);

        match response {
            EventResponse::Continue(propagate) => match propagate {
                Propagate::Ignored => {
                    info!(
                        "Event {:?} was ignored, applying global handling",
                        event
                    );
                    self.on_unhandled_event(event)
                },
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

                    let layout = self.layout;
                    with!(|layout| {
                        self.root.render(&mut RenderCtx::new(
                            self.root.id(),
                            self.state.read_only(),
                            renderer,
                            &layout.tree_root(),
                            TreeStyle::base(),
                            self.style.read_only(),
                            self.viewport,
                            self.fonts.read_only(),
                            self.theme,
                            FontProps {
                                font: Some(Font::Auto.maybe_reactive()),
                                font_size: None,
                                font_style: None,
                            },
                            self.force_redraw,
                        ))
                    })?;

                    self.dev_tools.with(|dev_tools| {
                        if dev_tools.enabled {
                            if let Some(hovered) = &dev_tools.hovered {
                                return hovered
                                    .draw(renderer, self.viewport.get());
                            }
                        }
                        Ok(())
                    })
                })
                .ok()
                // TODO: What do we do with errors, huh?
                .unwrap();
        });

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
        el::El,
        font::FontCtx,
        prelude::*,
        style::theme::Theme,
        widget::{Widget, ctx::*},
    };
    use alloc::string::String;
    use rsact_reactive::prelude::*;

    type NullWtf = Wtf<NullRenderer, (), ()>;

    fn create_null_page(root: impl Into<El<NullWtf>>) -> Page<NullWtf> {
        Page::new(
            (),
            root,
            Size::new_equal(1).maybe_reactive(),
            Theme::default().inert(),
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
