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

pub struct El<W>
where
    W: WidgetCtx,
{
    widget: Box<dyn Widget<W>>,
    mounted: bool,
}

impl<W> El<W>
where
    W: WidgetCtx,
{
    pub(crate) fn new(widget: impl Widget<W> + 'static) -> Self {
        Self { widget: Box::new(widget), mounted: false }
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
    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        // TODO: Wrong, mount can be called but should not call widget on_mount
        // debug_assert!(!self.mounted, "Widgets must not be remounted");
        if !self.mounted {
            self.widget.on_mount(ctx);
            self.mounted = true;
        }
    }

    fn meta(&self) -> crate::widget::MetaTree {
        self.widget.meta()
    }

    fn layout(&self) -> Signal<Layout> {
        self.widget.layout()
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        self.widget.build_layout_tree()
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> crate::widget::DrawResult {
        self.widget.draw(ctx)
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse<W> {
        self.widget.on_event(ctx)
    }
}
