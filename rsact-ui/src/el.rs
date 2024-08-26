use core::sync::atomic::AtomicUsize;

use alloc::boxed::Box;
use rsact_core::signal::ReadSignal;

use crate::{
    layout::LayoutTree,
    widget::{Widget, WidgetCtx},
};

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

pub struct El<C>
where
    C: WidgetCtx,
{
    id: ElId,
    widget: Box<dyn Widget<C>>,
}

impl<C> El<C>
where
    C: WidgetCtx,
{
    pub(crate) fn new(widget: Box<dyn Widget<C>>) -> Self {
        Self { id: ElId::unique(), widget }
    }
}

impl<C> Widget<C> for El<C>
where
    C: WidgetCtx,
{
    fn children(&self) -> &[El<C>] {
        self.widget.children()
    }

    fn size(&self) -> crate::size::Size<crate::size::Length> {
        self.widget.size()
    }

    fn content_size(&self) -> crate::layout::Limits {
        self.widget.content_size()
    }

    fn layout(
        &self,
        ctx: &crate::widget::LayoutCtx<'_, C>,
    ) -> crate::layout::Layout {
        self.widget.layout(ctx)
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, C>,
    ) -> crate::widget::DrawResult {
        self.widget.draw(ctx)
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, C>,
    ) -> crate::event::EventResponse<<C as WidgetCtx>::Event> {
        self.widget.on_event(ctx)
    }
}
