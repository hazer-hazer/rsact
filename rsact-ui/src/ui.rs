use embedded_graphics::prelude::DrawTarget;

use crate::{
    el::El,
    event::Event,
    layout::{model_layout, LayoutModel, Limits, Viewport},
    render::Renderer,
    size::Size,
    widget::{
        AppState, DrawCtx, DrawResult, LayoutCtx, PhantomWidgetCtx, Widget,
        WidgetCtx,
    },
};

pub struct UI<R: Renderer, E: Event> {
    root: El<PhantomWidgetCtx<R, E>>,
    state: AppState<PhantomWidgetCtx<R, E>>,
    layout: LayoutModel,
}

impl<R: Renderer, E: Event> UI<R, E> {
    pub fn new(
        root: El<PhantomWidgetCtx<R, E>>,
        viewport: impl Into<Size>,
    ) -> Self {
        let ctx = AppState::new();
        let layout = model_layout(
            &root,
            &LayoutCtx { state: &ctx },
            &Limits::only_max(viewport.into()),
        );

        Self { root, state: ctx, layout }
    }

    pub fn tick(&mut self, events: impl Iterator<Item = E>) {
        events.for_each(|event| {
            // TODO
        });
    }
}

impl<R: Renderer, E: Event> UI<R, E>
where
    R: DrawTarget,
{
    pub fn draw(&self, target: &mut R) -> DrawResult {
        self.root.draw(&mut DrawCtx {
            state: &self.state,
            renderer: target,
            layout: &self.layout.tree_root(),
        })
    }
}
