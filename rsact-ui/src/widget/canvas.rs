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

// WS13.4 (Task 5.11): like `Label`/`Bar`/`Checkbox`/`Slider`/`Knob`, `Canvas`
// has no build-only field to drop — `draw`/`layout` are both read by
// `render`/`layout`, so `CanvasBuilder` moves both fields into the retained
// `Canvas` unchanged (a `size_of` `<` assertion would be false, not true).
// The split is purely mechanical: the immediate-mode draw closure is moved
// by name like any other `#[widget]` field, never invoked or inspected
// during the build-time move, so none of the WS1b.1 DrawQueue/draw-command
// semantics documented above are touched.
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
#[derive(Builder)]
#[builds(Canvas<W>)]
pub struct CanvasBuilder<W: WidgetCtx> {
    // A single boxed closure is the entire Canvas state — no per-frame
    // `VecDeque` of commands and no image storage, which is the memory win of
    // the immediate-mode model.
    #[widget]
    draw: Box<dyn Fn(&mut RenderCtx<'_, W, CtxReady>) -> RenderResult>,
    #[widget]
    layout: LayoutBuilder<W>,
}

pub struct Canvas<W: WidgetCtx> {
    draw: Box<dyn Fn(&mut RenderCtx<'_, W, CtxReady>) -> RenderResult>,
    layout: LayoutData,
}

impl<W: WidgetCtx> Canvas<W> {
    /// Create a Canvas from a draw closure. The closure receives a drawing
    /// context clipped to the Canvas's rect and is called on every (forced)
    /// render.
    ///
    /// WS6.4c(A): the argument is a [`RenderCtx`], not `&mut W::Renderer`. It
    /// still *is* a renderer — `RenderCtx` implements [`Renderer`], so a closure
    /// that only calls primitives compiles unchanged (the argument type is
    /// inferred from the expected `Fn` type; only a closure that *annotates*
    /// `&mut SomeRenderer` breaks). Two reasons for the change:
    ///
    /// - **Forced:** `RenderCtx::renderer` is private, because a widget that can
    ///   reach the raw renderer can bypass the render mode. Canvas's closure is
    ///   user code, so it needs a public path to something drawable.
    /// - **Necessary for correctness under 6.4c:** this is the only widget whose
    ///   reactive reads happen *behind* a `dyn Fn`. Muting by not calling the
    ///   closure would leave its probe with an empty source set, and since the
    ///   collect pass is the only tracked run, the canvas would never repaint
    ///   again. Muting at the draw call runs the closure — `x.get()` and all —
    ///   and no-ops only its primitives.
    ///
    /// Bonus: the closure can now read its own `ctx.layout`, styles and
    /// pseudo-class, which a bare renderer could never expose.
    ///
    /// TODO (WS6.4c): give user closures a purpose-built `CanvasRenderCtx` —
    /// the public subset (`area()`, `style::<S>()`, `pseudoclass()`, `font()`,
    /// `clip()`) with probes, part keys, the arena and the damage sink absent.
    /// `RenderCtx` is an internal pass context and should not be the public
    /// surface; passing it here is the step, not the destination. Design in
    /// `docs/plans/2026-07-05-rsact-evolution-roadmap.md` § 6.4c(C).
    pub fn new(
        draw: impl Fn(&mut RenderCtx<'_, W, CtxReady>) -> RenderResult + 'static,
    ) -> CanvasBuilder<W> {
        CanvasBuilder {
            draw: Box::new(draw),
            layout: LayoutBuilder::edge(LengthSize::new_equal(Length::fill())),
        }
    }
}

impl<W: WidgetCtx> LayoutWidget<W> for CanvasBuilder<W> {
    fn layout_mut(&mut self) -> &mut LayoutBuilder<W> {
        &mut self.layout
    }
}

impl<W: WidgetCtx> SizedWidget<W> for CanvasBuilder<W> {}

impl<W: WidgetCtx> Widget<W> for Canvas<W> {
    // NOTE: no `flags`/`debug_name` override on the retained widget — both
    // are read exactly once, pre-build, from `Build` (seeding `ElState` at
    // `state.rs:72`); post-build all consumption is via `ElState`, so an
    // override here would be dead duplication of `CanvasBuilder`'s derived
    // `Build::debug_name` ("Canvas" from `#[builds(Canvas<W>)]`). `Canvas`
    // never overrode `flags` either, so no `#[flags(...)]` attr is needed on
    // `CanvasBuilder`.
    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        // `render_self` gates the redraw (tracking whatever reactivity the
        // closure reads, plus `force_redraw`); `clip_inner` confines drawing to
        // the Canvas rect. The closure re-issues the whole scene each frame.
        ctx.render_self(|mut ctx| {
            ctx.clip_inner(|mut ctx| (self.draw)(&mut ctx))
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
        test_support::TestPage,
    };
    use alloc::rc::Rc;
    use core::cell::Cell;
    use rsact_reactive::scope::new_scope;
    use rsact_render::renderer::NullRenderer;

    type NullWtf = Wtf<NullRenderer, (), (), ()>;

    fn null_page(root: impl View<NullWtf>) -> TestPage<NullWtf> {
        let arena = create_signal(ElArena::new());
        let scope = new_scope();
        TestPage::new(
            Page::new(
                (),
                root,
                arena,
                Size::new_equal(64).maybe_reactive(),
                ().inert(),
                DevTools::default().signal(),
                FontCtx::new().signal(),
                scope,
            ),
            NullRenderer::default(),
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
            // WS6.4c(A): the argument type is inferred from the expected `Fn`
            // type, so an unannotated closure survived the switch from
            // `&mut W::Renderer` to `&mut RenderCtx` untouched. This one used to
            // annotate `&mut NullRenderer` and is the only site in the workspace
            // that had to change — which is the whole cost of that API move.
            Canvas::new(move |_ctx| {
                draws_in.set(draws_in.get() + 1);
                Ok(())
            })
            .into_el(),
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
