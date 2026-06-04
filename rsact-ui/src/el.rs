use crate::widget::prelude::*;
use alloc::boxed::Box;
use core::sync::atomic::AtomicUsize;

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
    // TODO: If rsact-reactive would support ?Sized as a real smart-pointer we could do MaybeReactive<dyn Widget<W>>, so reactive elements creation would be possible in place. But the problem is that MaybeReactive is a readonly value, while MaybeSignal is owned stack value/Signal, so we either change the MaybeSignal to StoredValue/Signal or create a new MaybeSignal-like value with heap storage.
    // We can't, Rust does not allow unsized fields in structs, only through internal Box, Rc, etc. So we cannot make a custom arena-allocated smart pointer.
    widget: Box<dyn Widget<W>>,
    id: ElId,
}

impl<W> PartialEq for El<W>
where
    W: WidgetCtx,
{
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
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
        Self { widget: Box::new(widget), id: ElId::unique() }
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

    fn meta(&self, _parent_id: ElId) -> MetaTree {
        self.widget.meta(self.id)
    }

    fn layout(&self) -> Layout {
        self.widget.layout()
    }

    #[track_caller]
    fn render(&self, ctx: RenderCtx<'_, W>) -> RenderResult {
        self.widget.render(ctx)
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.id = self.id;
        self.widget.on_event(ctx)
    }
}
