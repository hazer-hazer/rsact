use super::prelude::*;
use core::marker::PhantomData;

/// Widget dependent on boolean memo.
/// When memo is false widget:
/// - Has zero layout
/// - Isn't drawn
/// - Ignores events
// WS5.1: `Show` owns its own `LayoutBuilder` now (there is no shared `Layout`
// `ValueId` to delegate to off the graph). At `new` it clones the wrapped
// child's initial `LayoutData` and stamps the `show` memo onto it, so when
// `show` is false `Show`'s arena layout resolves to zero and the child subtree
// is not laid out or drawn. (The macro `#[layout(delegate)]` path stays covered
// by the derive's unit tests; `Show` no longer uses it.)
#[derive(Builder)]
#[builds(Show<W>)]
pub struct ShowBuilder<W: WidgetCtx> {
    // TODO: Do we need MaybeReactive overhead if user rarely needs element to
    // always be hidden or shown?
    #[child(single)]
    el: El<W>,
    #[widget]
    layout: LayoutBuilder<W>,
    // Moved 1:1 by the derive into the retained `Show { layout, ctx }`:
    // `layout: LayoutData` alone doesn't use `W`, so `ctx: PhantomData<W>`
    // carries it (flex.rs precedent).
    #[widget]
    ctx: PhantomData<W>,
    // TODO: Cannot do fallback because layout returns Signal but I don't know
    // how to make dynamic layouts and how they should be mutated.
    // fallback: Option<El<W>>,
}

impl<W: WidgetCtx + 'static> Show<W> {
    pub fn new(
        show: impl IntoMemo<bool>,
        el: El<W>,
        // fallback: Option<El<W>>,
    ) -> ShowBuilder<W> {
        let show = show.memo();
        // WS5.1: clone the wrapped child's initial layout and gate it by `show`.
        // The child stays a real arena node; `Show`'s own layout mirrors it and
        // carries the visibility memo (off the graph, no shared `ValueId`).
        let mut layout = LayoutBuilder::new(el.layout_data());
        layout.show(show);
        ShowBuilder { el, layout, ctx: PhantomData }
    }
}

pub struct Show<W: WidgetCtx> {
    layout: LayoutData,
    // `W` is otherwise unused on the retained widget (unlike `ShowBuilder`,
    // which threads it through `el: El<W>`) — kept only to satisfy
    // `Widget<W>`'s own `W` parameter, same as `space.rs`/`flex.rs`.
    ctx: PhantomData<W>,
}

impl<W: WidgetCtx + 'static> Widget<W> for Show<W> {
    // NOTE: no `debug_name`/`flags` override on the retained widget — both are
    // read exactly once, pre-build, from `Build` (seeding `ElState`);
    // post-build all consumption is via `ElState`, so an override here would
    // be dead duplication of `ShowBuilder`'s derived `Build::debug_name`
    // ("Show" from `#[builds(Show<W>)]`). `Show` never overrode `flags`
    // either, so no `#[flags(...)]` attr is needed on `ShowBuilder`.
    #[track_caller]
    fn render(&self, _ctx: RenderCtx<'_, W>) -> RenderResult {
        // TODO: To render or not should be controlled via ElData property
        // "visible" and this widget should only control that property through
        // arena. To do that, in build pass we should subscribe arena to show
        // changes. Also, we need to figure out how to handle events, layout,
        // etc., should invisible elements receive events or not (maybe only
        // visibility-dependent events like mouse events, but not others?),
        // should their layout occupy space (surely no)? if self.show.
        // get() { self.el.render(ctx) } else { Ok(()) }
        //
        // Until then: `Show` owns no visual of its own — the child `el` is a
        // real arena node rendered by the tree walker, and visibility is driven
        // by `Show`'s own layout carrying the `show` memo (see `Show::new`),
        // which resolves a hidden element to a zero layout. So a no-op render
        // here is correct-enough and must not `todo!()` (that would abort the
        // device on every frame).
        Ok(())
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        // See the render note: events reach the child through the arena. Ignore
        // at this wrapper rather than panicking on the event path.
        // if self.show.get() { self.el.on_event(ctx) } else { ctx.ignore() }
        ctx.ignore()
    }
}
