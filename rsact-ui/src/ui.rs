use crate::{
    el::{El, ElId},
    event::{Capture, Event, EventResponse, FocusEvent, Propagate},
    layout::{model_layout, size::Size, LayoutModel, Limits},
    render::{color::Color, draw_target::LayeringRenderer, Renderer},
    style::{
        theme::{Theme, ThemeStyler},
        NullStyler,
    },
    widget::{
        DrawCtx, DrawResult, EventCtx, PageState, PhantomWidgetCtx, Widget,
        WidgetCtx,
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

pub struct Page<C: WidgetCtx> {
    root: El<C>,
    ids: Memo<Vec<ElId>>,
    layout: Memo<LayoutModel>,
    state: PageState<C>,
    // TODO: Should be Memo?
    style: Signal<PageStyle<C::Color>>,
    renderer: C::Renderer,
}

impl<C: WidgetCtx> Page<C> {}

impl<C: WidgetCtx> Page<C> {
    fn new(root: impl Into<El<C>>, viewport: Size) -> Self {
        let root = root.into();
        let state = PageState::new();
        let limits = Limits::only_max(viewport);

        let layout_tree = root.build_layout_tree();
        let layout = use_memo(move |_| {
            // println!("Relayout");
            model_layout(layout_tree, limits)
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
            renderer: C::Renderer::new(viewport),
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
        events: impl Iterator<Item = C::Event>,
    ) -> Vec<C::Event> {
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
        target: &mut impl DrawTarget<Color = <C::Renderer as Renderer>::Color>,
        styler: &C::Styler,
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
                styler,
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
    S: Default + 'static,
{
    active_page: usize,
    pages: Vec<Page<PhantomWidgetCtx<R, E, S>>>,
    viewport: Size,
    on_exit: Option<Box<dyn Fn()>>,
    // TODO: Use `Option` instead of NullStyler to avoid useless allocation of
    // Default ThemeStyler. ThemeStyler should only be set when theme is set
    styler: Option<S>,
}

impl<C, E, S> UI<LayeringRenderer<C>, E, S>
where
    E: Event + 'static,
    C: Color + 'static,
    S: Default + 'static,
{
    pub fn draw(
        &mut self,
        target: &mut impl DrawTarget<Color = C>,
    ) -> DrawResult {
        self.pages[self.active_page].draw(target, &self.styler)
    }
}

impl<R, E> UI<R, E, ThemeStyler<R::Color>>
where
    R: Renderer + 'static,
    E: Event + 'static,
{
    pub fn theme(self, theme: Theme) -> Self {
        self.styler.set_theme(theme);
        self
    }
}

impl<R, E> UI<R, E, NullStyler>
where
    R: Renderer + 'static,
    E: Event + 'static,
{
    pub fn new(
        root: impl Into<El<PhantomWidgetCtx<R, E, NullStyler>>>,
        viewport: impl Into<Size> + Copy,
    ) -> Self {
        Self {
            active_page: 0,
            viewport: viewport.into(),
            pages: vec![Page::new(root, viewport.into())],
            on_exit: None,
            styler: Default::default(),
        }
    }
}

impl<R, E, S> UI<R, E, S>
where
    R: Renderer + 'static,
    E: Event + 'static,
    S: Default + 'static,
{
    /// Add ExitEvent handler that eats exit event
    pub fn on_exit(mut self, on_exit: impl Fn() + 'static) -> Self {
        self.on_exit = Some(Box::new(on_exit));
        self
    }

    pub fn current_page(&mut self) -> &mut Page<PhantomWidgetCtx<R, E, S>> {
        &mut self.pages[self.active_page]
    }

    pub fn add_page(&mut self, root: impl Into<El<PhantomWidgetCtx<R, E, S>>>) {
        self.pages.push(Page::new(root, self.viewport))
    }

    pub fn with_page(
        mut self,
        root: impl Into<El<PhantomWidgetCtx<R, E, S>>>,
    ) -> UI<R, E, S> {
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
