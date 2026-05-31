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
    current: Signal<Option<El<W>>>,
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
    fn meta(&self, id: ElId) -> MetaTree {
        // TODO: Is it okay to nest a child when Dynamic is a transparent widget? Maybe we need MaybeReactive meta tree data? We don't use meta tree depth lookup, so it is not a problem now, but maybe it will be in the future.
        MetaTree::new(
            Meta::none(),
            self.current.map(move |current| {
                current
                    .as_ref()
                    .map(|current| vec![current.meta(id)])
                    .unwrap_or_default()
            }),
        )
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, ctx: &mut RenderCtx<'_, W>) -> RenderResult {
        self.current.with(|current| current.as_ref().unwrap().render(ctx))
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        self.current.update(|current| current.as_mut().unwrap().on_event(ctx))
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
