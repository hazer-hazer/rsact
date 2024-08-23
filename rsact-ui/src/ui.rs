use embedded_graphics::prelude::DrawTarget;

use crate::{
    el::El,
    event::Event,
    layout::{model_layout, LayoutModel, Limits, Viewport},
    render::Renderer,
    size::Size,
    widget::{Ctx, DrawResult, PhantomWidgetCtx, Widget, WidgetCtx},
};

pub struct UI<R: Renderer, E: Event> {
    root: El<PhantomWidgetCtx<R, E>>,
    ctx: Ctx<PhantomWidgetCtx<R, E>>,
    layout: LayoutModel,
}

impl<R: Renderer, E: Event> UI<R, E> {
    pub fn new(
        root: El<PhantomWidgetCtx<R, E>>,
        viewport: impl Into<Size>,
    ) -> Self {
        let ctx = Ctx::new();
        let layout =
            model_layout(&root, &ctx, &Limits::only_max(viewport.into()));

        Self { root, ctx, layout }
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
        self.root.draw(&self.ctx, target, &self.layout.tree_root())
    }
}
