use super::prelude::*;
use core::marker::PhantomData;

/// Widget dependent on boolean memo.
/// When memo is false widget:
/// - Has zero layout
/// - Isn't drawn
/// - Ignores events
// M2's consumer (WS13.4/5.1): `#[layout(delegate = "el")]` — same handle as
// `layout`; exercises the delegate path (M2). `layout` and `el`'s layout are
// the same `ValueId` (see `Show::new`), so the default `self.layout` would
// also work here; the delegate is used deliberately as its end-to-end
// consumer.
#[derive(Builder)]
#[builds(Show<W>)]
#[layout(delegate = "el")]
pub struct ShowBuilder<W: WidgetCtx> {
    // TODO: Do we need MaybeReactive overhead if user rarely needs element to
    // always be hidden or shown?
    #[child(single)]
    el: El<W>,
    #[widget]
    layout: Layout,
    // Moved 1:1 by the derive into the retained `Show { layout, ctx }` (a ZST):
    // `layout: Layout` alone doesn't use `W`, so `ctx: PhantomData<W>` carries
    // it (flex.rs precedent).
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
        el.layout().show(show);
        // TODO: This is a logic for `IfWidget` or so
        // fallback.layout().update(|layout| {
        //     layout.set_show(show.map(|show| !*show));
        // });
        // WS13.4: `show` is dead past this point — its only use is wiring
        // `el`'s layout above — so it is a `new()` local rather than a stored
        // builder field (deviation from the row's literal wording, which
        // listed `show` as a builder field; see the task report).
        let layout = el.layout();
        ShowBuilder { el, layout, ctx: PhantomData }
    }
}

pub struct Show<W: WidgetCtx> {
    layout: Layout,
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
    fn layout(&self) -> Layout {
        self.layout
    }

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
        // by `el.layout().show(show)` (see `Show::new`), which resolves a hidden
        // element to a zero layout. So a no-op render here is correct-enough and
        // must not `todo!()` (that would abort the device on every frame).
        Ok(())
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        // See the render note: events reach the child through the arena. Ignore
        // at this wrapper rather than panicking on the event path.
        // if self.show.get() { self.el.on_event(ctx) } else { ctx.ignore() }
        ctx.ignore()
    }
}
