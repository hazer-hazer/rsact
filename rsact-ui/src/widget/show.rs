use super::prelude::*;

/// Widget dependent on boolean memo.
/// When memo is false widget:
/// - Has zero layout
/// - Isn't drawn
/// - Ignores events
pub struct Show<W: WidgetCtx> {
    // TODO: Do we need MaybeReactive overhead if user rarely needs element to always be hidden or shown?
    show: Memo<bool>,
    el: El<W>,
    // TODO: Cannot do fallback because layout returns Signal but I don't know how to make dynamic layouts and how they should be mutated.
    // fallback: Option<El<W>>,
}

impl<W: WidgetCtx> Show<W> {
    pub fn new(
        show: impl IntoMemo<bool>,
        el: El<W>,
        // fallback: Option<El<W>>,
    ) -> Self {
        let show = show.memo();
        el.layout().show(show);
        // TODO: This is a logic for `IfWidget` or so
        // fallback.layout().update(|layout| {
        //     layout.set_show(show.map(|show| !*show));
        // });
        Self { show, el }
    }
}

impl<W: WidgetCtx> Widget<W> for Show<W> {
    fn debug_name(&self) -> &'static str {
        "Show"
    }

    fn build(&mut self, mut ctx: BuildCtx<W>) {
        ctx.set_single_child(&mut self.el);
    }

    fn layout(&self) -> Layout {
        self.el.layout()
    }

    #[track_caller]
    fn render(&self, ctx: RenderCtx<'_, W>) -> RenderResult {
        // TODO: To render or not to should be controlled via ElData property "visible" and this widget should only control that property through arena. To do that, in build pass we should subscribe arena to show changes. Also, we need to figure out how to handle events, layout, etc., should invisible elements receive events or not (maybe only visibility-dependent events like mouse events, but not others?), should their layout occupy space (surely no)?
        // if self.show.get() { self.el.render(ctx) } else { Ok(()) }
        todo!()
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        todo!()
        // if self.show.get() { self.el.on_event(ctx) } else { ctx.ignore() }
    }
}
