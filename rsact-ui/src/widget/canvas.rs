use crate::widget::prelude::*;

// An immediate-mode drawing surface (WS1b b.1).
//
// A `Canvas` holds a single draw closure invoked on every render pass, so
// there is no retained command buffer to drain: the scene is whatever the
// closure draws *this* frame. This replaced the earlier `DrawQueue` /
// `DrawCommand` / `CanvasImage` command-buffer model, which drained itself on
// the first render and therefore blanked on the next `force_redraw` (fired on
// any relayout / navigation / devtools toggle). The maintainer's b.1 decision
// (2026-07-07) is recorded in docs/plans/2026-07-05-rsact-evolution-roadmap.md;
// a retained `Memo<Vec<DrawCommand>>` layer, if ever wanted, is WS16.1's call.
//
// TODO (WS16.1): an optional retained/diffed command layer on top of this
// immediate-mode primitive, once rsact-render's `Image` `PartialEq` is fixed
// (it returns false for `Owned == Owned`, defeating memo-diffing of command
// lists).

/// An immediate-mode drawing surface.
///
/// A `Canvas` is built from a single draw closure. The closure is invoked on
/// every render pass and receives the renderer already clipped to the Canvas's
/// own rect, so drawing cannot escape its bounds. Because the closure runs
/// inside [`RenderCtx::render_self`](crate::el::render::RenderCtx::render_self)'s
/// reactive observer, it re-runs automatically whenever a reactive value it
/// reads changes, and — like every other widget — it redraws on `force_redraw`
/// / relayout. There is no retained command buffer.
///
/// ```ignore
/// // `x` is a `Memo<i32>` / `Signal<i32>`; reading it here subscribes the
/// // Canvas's render observer, so the circle follows `x` reactively.
/// Canvas::new(move |renderer| {
///     renderer.circle(Point::new(x.get(), 15), 50, &style)?;
///     Ok(())
/// })
/// ```
#[derive(View)]
pub struct Canvas<W: WidgetCtx> {
    // A single boxed closure is the entire Canvas state — no per-frame
    // `VecDeque` of commands and no image storage, which is the memory win of
    // the immediate-mode model.
    draw: Box<dyn Fn(&mut W::Renderer) -> RenderResult>,
    layout: Layout,
}

impl<W: WidgetCtx> Canvas<W> {
    /// Create a Canvas from a draw closure. The closure receives the renderer
    /// clipped to the Canvas's rect and is called on every (forced) render.
    pub fn new(
        draw: impl Fn(&mut W::Renderer) -> RenderResult + 'static,
    ) -> Self {
        Self {
            draw: Box::new(draw),
            layout: Layout::edge(LengthSize::new_equal(Length::fill())),
        }
    }
}

impl<W: WidgetCtx> LayoutWidget<W> for Canvas<W> {
    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }
}

impl<W: WidgetCtx> SizedWidget<W> for Canvas<W> {}

impl<W: WidgetCtx> Widget<W> for Canvas<W> {
    fn debug_name(&self) -> &'static str {
        "Canvas"
    }

    fn build(&mut self, ctx: BuildCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        // `render_self` gates the redraw (tracking whatever reactivity the
        // closure reads, plus `force_redraw`); `clip_inner` confines drawing to
        // the Canvas rect. The closure re-issues the whole scene each frame.
        ctx.render_self(|mut ctx| {
            ctx.clip_inner(|ctx| (self.draw)(ctx.renderer))
        })
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.ignore()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        el::{arena::ElArena, ctx::Wtf, view::View},
        font::FontCtx,
        page::{Page, dev::DevTools},
    };
    use alloc::rc::Rc;
    use core::cell::Cell;
    use rsact_render::renderer::NullRenderer;

    type NullWtf = Wtf<NullRenderer, (), (), ()>;

    fn null_page(root: impl View<NullWtf>) -> Page<NullWtf> {
        let arena = create_signal(ElArena::new());
        Page::new(
            (),
            root,
            arena,
            Size::new_equal(64).maybe_reactive(),
            ().inert(),
            DevTools::default().signal(),
            NullRenderer::default().signal(),
            FontCtx::new().signal(),
        )
    }

    // b.1: an immediate-mode Canvas must re-run its draw closure on every
    // forced redraw (relayout / navigation / devtools), not blank after the
    // first frame the way the old drain-on-render `DrawQueue` did. The closure
    // counts its own invocations through a non-reactive `Cell` (so it doesn't
    // subscribe the render observer to itself).
    #[test]
    fn canvas_redraws_on_force_and_not_when_idle() {
        let draws = Rc::new(Cell::new(0u32));
        let draws_in = Rc::clone(&draws);

        let mut page = null_page(
            Canvas::new(move |_renderer: &mut NullRenderer| {
                draws_in.set(draws_in.get() + 1);
                Ok(())
            })
            .el(),
        );

        page.use_renderer(|_| {});
        assert_eq!(draws.get(), 1, "closure must draw on the first frame");

        // Nothing changed and no force: the render observer must gate the
        // redraw away (immediate-mode still respects reactive gating).
        page.use_renderer(|_| {});
        assert_eq!(draws.get(), 1, "must not redraw when nothing changed");

        // A forced redraw (as after relayout / navigation) must re-issue the
        // whole scene — the old DrawQueue blanked here.
        page.force_redraw();
        page.use_renderer(|_| {});
        assert_eq!(draws.get(), 2, "Canvas must redraw on force, not blank");
    }
}
