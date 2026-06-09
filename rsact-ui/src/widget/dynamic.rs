use super::prelude::*;

/// Reactive widget factory.
///
/// `factory` is executed inside a reactive effect, so any signals read inside
/// it are tracked and rebuild the active child element when changed.
pub fn dynamic<W, F, E>(mut factory: F) -> Dynamic<W>
where
    W: WidgetCtx + 'static,
    F: FnMut() -> E + 'static,
    E: Into<El<W>>,
{
    Dynamic::new(move || factory().into())
}

pub struct Dynamic<W: WidgetCtx> {
    // Child that always some after construction, option is needed to be set on initialization
    // TODO: MaybeUninit can be used for optimization.
    current: Signal<Option<El<W>>>,

    // TODO: Track previous element on change to dispose it from arena.

    // TODO: Need transparent layout node to nest dynamic child, otherwise
    // Layout needs to be stored separately to return it from
    layout: Layout,
}

impl<W: WidgetCtx + 'static> Dynamic<W> {
    pub fn new<E: Into<El<W>>>(
        mut factory: impl FnMut() -> E + 'static,
    ) -> Self {
        let mut current = create_signal(None::<El<W>>);

        let layout = create_effect(move |_| {
            let el = factory().into();
            let layout = el.layout();
            current.set(Some(el));
            layout
        })
        .with_last_value(|layout| *layout);

        Self { current, layout }
    }
}

impl<W: WidgetCtx + 'static> Widget<W> for Dynamic<W> {
    fn flags(&self) -> WidgetFlags {
        WidgetFlags::default().transparent_layout()
    }

    fn debug_name(&self) -> &'static str {
        "[Dynamic]"
    }

    // TODO: Can transparent widget have children?
    fn build(&mut self, mut ctx: BuildCtx<W>) {
        let mut current = self.current;
        create_effect(move |_| {
            current.update(|current| {
                ctx.set_single_child(
                    current.as_mut().expect("Dynamic element cannot be unset"),
                );
            })
        });
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, _ctx: RenderCtx<'_, W>) -> RenderResult {
        Ok(())
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.ignore()
    }
}

impl<W, E, F> From<F> for El<W>
where
    W: WidgetCtx,
    E: Into<El<W>>,
    F: FnMut() -> E + 'static,
{
    fn from(factory: F) -> Self {
        dynamic(factory).el()
    }
}
