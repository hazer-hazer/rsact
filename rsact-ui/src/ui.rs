use crate::{
    el::{El, ElId},
    event::{self, CommonEvent, Event, EventResponse, Propagate},
    layout::{model_layout, size::Size, Layout, LayoutModel, Limits},
    render::Renderer,
    widget::{
        DrawCtx, DrawResult, EventCtx, LayoutCtx, PageState, PhantomWidgetCtx,
        Widget, WidgetCtx,
    },
};
use alloc::vec::Vec;
use embedded_graphics::prelude::DrawTarget;
use rsact_core::{
    prelude::use_computed,
    signal::{marker::ReadOnly, ReadSignal, Signal},
};

struct Page<C: WidgetCtx> {
    root: El<C>,
    ids: Signal<Vec<ElId>>,
    layout: Signal<LayoutModel>,
    state: PageState<C>,
}

impl<C: WidgetCtx + 'static> Page<C> {
    fn new(root: El<C>, viewport: Size) -> Self {
        let state = PageState::new();
        let limits = Limits::only_max(viewport);

        let layout_tree = root.build_layout_tree();
        let layout = use_computed(move || {
            // println!("Relayout");
            model_layout(layout_tree, limits)
        });
        let ids = root.children_ids();

        Self { root, layout, state, ids }
    }

    pub fn handle_events(
        &mut self,
        events: impl Iterator<Item = C::Event>,
    ) -> Vec<CommonEvent> {
        events
            .map(|event| {
                let response = self.root.on_event(&mut EventCtx {
                    event: &event,
                    page_state: &mut self.state,
                });

                if let EventResponse::Continue(Propagate::BubbleUp(
                    el_id,
                    event,
                )) = response
                {
                    if let Some(common) = event.as_common() {
                        match common {
                            event::CommonEvent::FocusMove(offset) => {
                                self.ids.with(|ids| {
                                    let position =
                                        ids.iter().position(|&id| id == el_id);
                                    // TODO: Warn if id not found
                                    self.state.focused =
                                        position.and_then(|pos| {
                                            ids.get(
                                                (pos as i64 + offset as i64)
                                                    as usize,
                                            )
                                            .copied()
                                        });
                                });
                                None
                            },
                            event::CommonEvent::Exit => Some(common),
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .filter_map(|event| event)
            .collect()
    }

    pub fn draw(&self, renderer: &mut C::Renderer) -> DrawResult {
        self.layout.with(|layout| {
            self.root.draw(&mut DrawCtx {
                state: &self.state,
                renderer,
                layout: &layout.tree_root(),
            })
        })
    }
}

pub struct UI<R: Renderer, E: Event> {
    active_page: usize,
    pages: Vec<Page<PhantomWidgetCtx<R, E>>>,
    on_exit: Option<Box<dyn Fn()>>,
}

impl<R, E> UI<R, E>
where
    R: Renderer + 'static,
    E: Event + 'static,
{
    pub fn new(
        root: El<PhantomWidgetCtx<R, E>>,
        viewport: impl Into<Size>,
    ) -> Self {
        Self {
            active_page: 0,
            pages: vec![Page::new(root, viewport.into())],
            on_exit: None,
        }
    }

    pub fn on_exit(mut self, on_exit: impl Fn() + 'static) -> Self {
        self.on_exit = Some(Box::new(on_exit));
        self
    }

    pub fn tick(&mut self, events: impl Iterator<Item = E>) {
        self.pages[self.active_page]
            .handle_events(events)
            .into_iter()
            .for_each(|event| match event {
                CommonEvent::FocusMove(_) => {},
                CommonEvent::Exit => {
                    if let Some(on_exit) = self.on_exit.as_ref() {
                        on_exit()
                    }
                },
            });
    }
}

impl<R, E> UI<R, E>
where
    R: DrawTarget + Renderer + 'static,
    E: Event + 'static,
{
    pub fn draw(&self, target: &mut R) -> DrawResult {
        self.pages[self.active_page].draw(target)
    }
}
