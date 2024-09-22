use crate::{
    el::{El, ElId},
    event::{Capture, Event, EventResponse, FocusEvent, Propagate},
    layout::{model_layout, size::Size, LayoutModel, Limits},
    render::{color::Color, draw_target::LayeringRenderer, Renderer},
    style::NullStyler,
    widget::{
        DrawCtx, DrawResult, EventCtx, MountCtx, PageState, PhantomWidgetCtx,
        Widget, WidgetCtx,
    },
};
use alloc::{boxed::Box, vec::Vec};
use embedded_graphics::prelude::DrawTarget;
use rsact_core::prelude::*;

pub struct PageStyle<C: Color> {
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

pub struct Page<W: WidgetCtx> {
    root: El<W>,
    ids: Memo<Vec<ElId>>,
    layout: Memo<LayoutModel>,
    state: PageState<W>,
    // TODO: Should be Memo?
    style: Signal<PageStyle<W::Color>>,
    renderer: W::Renderer,
}

impl<W: WidgetCtx> Page<W> {}

impl<W: WidgetCtx> Page<W> {
    fn new(
        root: impl Into<El<W>>,
        viewport: Signal<Size>,
        styler: Signal<W::Styler>,
    ) -> Self {
        let mut root = root.into();
        let state = PageState::new();

        root.on_mount(MountCtx {
            viewport: use_memo(move |_| viewport.get()),
            styler: use_memo(move |_| styler.get()),
        });

        let layout_tree = root.build_layout_tree();
        let layout = use_memo(move |_| {
            // println!("Relayout");
            model_layout(layout_tree, Limits::only_max(viewport.get()))
        });
        // TODO: Children ids should be paired with Behavior settings, child can
        // have an id but not be focusable for example
        let ids = root.children_ids();

        Self {
            root,
            layout,
            state,
            style: PageStyle::base().into_signal(),
            ids,
            // TODO: Signal viewport in Renderer
            renderer: W::Renderer::new(viewport.get()),
        }
    }

    // pub fn style(
    //     mut self,
    //     style: impl IntoSignal<PageStyle<C::Color>>,
    // ) -> Self {
    //     self.style = style.signal();
    //     self
    // }

    pub fn auto_focus(&mut self) {
        self.ids.with(|ids| {
            self.state.focused = ids.first().copied();
        })
    }

    pub fn handle_events(
        &mut self,
        events: impl Iterator<Item = W::Event>,
    ) -> Vec<W::Event> {
        events
            .map(|event| {
                let response = self.layout.with(|layout| {
                    self.root.on_event(&mut EventCtx {
                        event: &event,
                        page_state: &mut self.state,
                        layout: &layout.tree_root(),
                    })
                });

                match response {
                    EventResponse::Continue(propagate) => match propagate {
                        Propagate::Ignored => Some(event),
                        Propagate::BubbleUp(_, event) => Some(event),
                    },
                    EventResponse::Break(capture) => match capture {
                        Capture::Captured => None,
                        Capture::Bubbled(el_id, event) => {
                            if let Some(offset) = event.as_focus_move() {
                                self.ids.with(|ids| {
                                    let position =
                                        ids.iter().position(|&id| id == el_id);

                                    if let Some(new) =
                                        position.and_then(|pos| {
                                            ids.get(
                                                (pos as i64 + offset as i64)
                                                    as usize,
                                            )
                                            .copied()
                                        })
                                    {
                                        self.state.focused.replace(new);
                                    }
                                });

                                None
                            } else {
                                Some(event)
                            }
                        },
                    },
                }
            })
            .filter_map(|event| event)
            .collect()
    }

    pub fn draw(
        &mut self,
        target: &mut impl DrawTarget<Color = <W::Renderer as Renderer>::Color>,
    ) -> DrawResult {
        self.style.with(|style| {
            if let Some(background_color) = style.background_color {
                Renderer::clear(&mut self.renderer, background_color)
            } else {
                Ok(())
            }
        })?;

        let result = self.layout.with(|layout| {
            self.root.draw(&mut DrawCtx {
                state: &self.state,
                renderer: &mut self.renderer,
                layout: &layout.tree_root(),
            })
        })?;

        self.renderer.finish(target);

        Ok(result)

        // self.style.with(|style| {
        //     if let Some(focused) = self.state.focused {
        //         renderer.block(Block {
        //             border:
        // Border::zero().color(style.focus_outline.color).radius(style.
        // focus_outline.radius).width(1),             rect: ,
        //             background: todo!(),
        //         })
        //     }
        // });
    }
}

pub struct UI<R, E, S>
where
    R: Renderer + 'static,
    E: Event + 'static,
    S: PartialEq + Copy + 'static,
{
    active_page: usize,
    pages: Vec<Page<PhantomWidgetCtx<R, E, S>>>,
    viewport: Signal<Size>,
    on_exit: Option<Box<dyn Fn()>>,
    // TODO: Use `Option` instead of NullStyler to avoid useless allocation of
    // Default ThemeStyler. ThemeStyler should only be set when theme is set
    styler: Signal<S>,
}

impl<C, E, S> UI<LayeringRenderer<C>, E, S>
where
    E: Event + 'static,
    C: Color + 'static,
    S: PartialEq + Copy + 'static,
{
    pub fn draw(
        &mut self,
        target: &mut impl DrawTarget<Color = C>,
    ) -> DrawResult {
        self.pages[self.active_page].draw(target)
    }
}

// impl<R, E> UI<R, E, ThemeStyler<R::Color>>
// where
//     R: Renderer + 'static,
//     E: Event + 'static,
//     R::Color: ThemeColor,
// {
//     pub fn theme(mut self, theme: Theme<R::Color>) -> Self {
//         if let Some(styler) = self.styler.as_mut() {
//             styler.set_theme(theme);
//         } else {
//             self.styler.replace(ThemeStyler::new(theme));
//         }
//         self
//     }
// }

// impl<R, E> UI<R, E, NullStyler>
// where
//     R: Renderer + 'static,
//     E: Event + 'static,
// {
// }

impl<R, E, S> UI<R, E, S>
where
    R: Renderer + 'static,
    E: Event + 'static,
    S: PartialEq + Copy + 'static,
{
    pub fn new(
        root: impl Into<El<PhantomWidgetCtx<R, E, S>>>,
        viewport: impl Into<Size> + Copy,
        styler: S,
    ) -> Self {
        let viewport = use_signal(viewport.into());
        let styler = use_signal(styler);

        Self {
            active_page: 0,
            viewport,
            pages: vec![Page::new(root, viewport, styler)],
            on_exit: None,
            styler,
        }
    }

    /// Add ExitEvent handler that eats exit event
    pub fn on_exit(mut self, on_exit: impl Fn() + 'static) -> Self {
        self.on_exit = Some(Box::new(on_exit));
        self
    }

    pub fn current_page(&mut self) -> &mut Page<PhantomWidgetCtx<R, E, S>> {
        &mut self.pages[self.active_page]
    }

    pub fn add_page(&mut self, root: impl Into<El<PhantomWidgetCtx<R, E, S>>>) {
        self.pages.push(Page::new(root, self.viewport, self.styler))
    }

    pub fn with_page(
        mut self,
        root: impl Into<El<PhantomWidgetCtx<R, E, S>>>,
    ) -> Self {
        self.add_page(root);
        self
    }

    pub fn tick(&mut self, events: impl Iterator<Item = E>) -> Vec<E> {
        self.pages[self.active_page]
            .handle_events(events)
            .iter()
            .cloned()
            .filter_map(|e| {
                if let (Some(on_exit), true) =
                    (self.on_exit.as_ref(), e.as_exit())
                {
                    on_exit();
                    None
                } else {
                    Some(e)
                }
            })
            .collect()
    }
}
