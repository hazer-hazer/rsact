use crate::{
    el::El,
    event::Event,
    layout::{model_layout, size::Size, LayoutModel, LayoutTree, Limits},
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
    layout: Signal<LayoutModel, ReadOnly>,
    state: PageState<C>,
}

impl<C: WidgetCtx + 'static> Page<C> {
    fn new(root: El<C>, viewport: Size) -> Self {
        let state = PageState::new();
        let limits = Limits::only_max(viewport);

        let layout_tree = LayoutTree::build(&root);
        let layout = use_computed(move || model_layout(&layout_tree, limits));

        Self { root, layout, state }
    }

    pub fn handle_events(&mut self, events: impl Iterator<Item = C::Event>) {
        events.for_each(|event| {
            self.root.on_event(&mut EventCtx {
                event: &event,
                page_state: &mut self.state,
            });
        });
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
        Self { active_page: 0, pages: vec![Page::new(root, viewport.into())] }
    }

    pub fn tick(&mut self, events: impl Iterator<Item = E>) {
        self.pages[self.active_page].handle_events(events)
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
