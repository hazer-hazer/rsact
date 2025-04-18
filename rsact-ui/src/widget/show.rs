use super::prelude::*;

/// Widget dependent on boolean memo.
/// When memo is false widget:
/// - Has zero layout
/// - Isn't drawn
/// - Ignores events
pub struct Show<W: WidgetCtx> {
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
        el.layout().update(|layout| {
            layout.set_show(show);
        });
        // fallback.layout().update(|layout| {
        //     layout.set_show(show.map(|show| !*show));
        // });
        Self { show, el }
    }
}

impl<W: WidgetCtx> Widget<W> for Show<W> {
    fn meta(&self) -> MetaTree {
        self.el.meta()
    }

    fn on_mount(&mut self, ctx: MountCtx<W>) {
        self.el.on_mount(ctx);
    }

    fn layout(&self) -> Signal<Layout> {
        self.el.layout()
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        if self.show.get() { self.el.draw(ctx) } else { Ok(()) }
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse {
        if self.show.get() { self.el.on_event(ctx) } else { ctx.ignore() }
    }
}
