use super::prelude::*;

/// Reactive widget factory.
///
/// `factory` is executed inside a reactive effect, so any signals read inside
/// it are tracked and rebuild the active child element when changed.
pub fn dynamic<W, F, E>(mut factory: F) -> Dynamic<W>
where
    W: WidgetCtx + 'static,
    F: FnMut() -> E + 'static,
    E: View<W>,
{
    Dynamic::new(move || factory().into_el())
}

#[derive(View)]
pub struct Dynamic<W: WidgetCtx> {
    // Child that always some after construction, option is needed to be set on
    // initialization TODO: MaybeUninit can be used for optimization.
    current: Signal<Option<El<W>>>,
    // TODO: Track previous element on change to dispose it from arena.

    // WS5.1: no `layout` field — `Dynamic` is `transparent_layout`, so the arena
    // walk skips its slot (its `LayoutData` stays the zero default) and lays out
    // the wrapped child directly, which is what the note below asked for.
    // TODO: Need transparent layout node to nest dynamic child, otherwise
    // Layout needs to be stored separately to return it from
}

impl<W: WidgetCtx + 'static> Dynamic<W> {
    pub fn new<E: View<W>>(mut factory: impl FnMut() -> E + 'static) -> Self {
        let mut current = create_signal(None::<El<W>>);

        // WS5.1: the factory runs inside an effect so reactive reads rebuild the
        // child; the child's layout lives in the arena now (keyed by `ElId`), so
        // this no longer computes/stores a `Layout` — it only sets `current`.
        create_effect(move |_| {
            let el = factory().into_el();
            current.set(Some(el));
        });

        Self { current }
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

    #[track_caller]
    fn render(&self, _ctx: RenderCtx<'_, W>) -> RenderResult {
        Ok(())
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.ignore()
    }
}

// A factory closure is itself a `View`: it builds a reactive [`Dynamic`] child
// that re-runs the closure (and rebuilds) when any signal read inside changes.
//
// This is a blanket impl over `FnMut`, which coexists with the concrete leaf
// impls (`View for &str`, ...) and the per-widget derived impls because the
// compiler can do negative reasoning for the `Fn` traits (no user type can
// implement them on stable), so no overlap is possible.
impl<W, E, F> View<W> for F
where
    W: WidgetCtx + 'static,
    E: View<W>,
    F: FnMut() -> E + 'static,
{
    fn into_el(self) -> El<W> {
        dynamic(self).el()
    }
}
