use crate::widget::prelude::*;
use alloc::boxed::Box;
use core::sync::atomic::AtomicUsize;
use rsact_reactive::maybe::IntoMaybeReactive;

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub enum ElId {
    Unique(usize),
    Custom(&'static str),
}

impl ElId {
    pub fn new(name: &'static str) -> Self {
        Self::Custom(name)
    }

    pub fn unique() -> Self {
        Self::Unique(
            NEXT_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
        )
    }
}

impl From<&'static str> for ElId {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

/// Value bound with [`ElId`], used for hashing purposes
#[derive(Debug, Clone, Copy, Hash)]
pub struct WithElId<T> {
    id: ElId,
    value: T,
}

impl<T> WithElId<T> {
    pub fn new(id: ElId, value: T) -> Self {
        Self { id, value }
    }
}

pub struct El<W>
where
    W: WidgetCtx,
{
    widget: Box<dyn Widget<W>>,
    mounted: bool,
    id: ElId,
}

impl<W> El<W>
where
    W: WidgetCtx,
{
    pub fn id(&self) -> ElId {
        self.id
    }
}

impl<W> El<W>
where
    W: WidgetCtx,
{
    pub(crate) fn new(widget: impl Widget<W> + 'static) -> Self {
        Self { widget: Box::new(widget), mounted: false, id: ElId::unique() }
    }
}

impl<W> Widget<W> for El<W>
where
    W: WidgetCtx + 'static,
{
    fn el(self) -> El<W>
    where
        Self: Sized + 'static,
    {
        self
    }

    // TODO: on_mount should not subscribe to ctx, but return a callback to call when MountCtx changes
    fn on_mount(&mut self, ctx: MountCtx<W>) {
        if !self.mounted {
            self.widget.on_mount(ctx);
            self.mounted = true;
        }
    }

    fn meta(&self, _parent_id: ElId) -> MetaTree {
        self.widget.meta(self.id)
    }

    fn layout(&self) -> Signal<Layout> {
        self.widget.layout()
    }

    #[track_caller]
    fn render(
        &self,
        ctx: &mut RenderCtx<'_, W>,
    ) -> crate::widget::RenderResult {
        self.widget.render(ctx)
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.id = self.id;
        self.widget.on_event(ctx)
    }
}
