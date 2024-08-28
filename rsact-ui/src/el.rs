use crate::{event::Event, widget::prelude::*};
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
    C: WidgetCtx + 'static,
{
    fn children_ids(&self) -> Signal<Vec<ElId>> {
        let children_ids = self.widget.children_ids();
        children_ids.update(|children_ids| {
            children_ids.push(self.id);
        });
        children_ids
    }

    fn layout(&self) -> Signal<Layout> {
        self.widget.layout()
    }

    fn build_layout_tree(&self) -> SignalTree<Layout> {
        self.widget.build_layout_tree()
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> crate::widget::DrawResult {
        self.widget.draw(ctx)
    }

    fn behavior(&self) -> Behavior {
        self.widget.behavior()
    }

    fn on_event(
        &mut self,
        ctx: &mut EventCtx<'_, C>,
    ) -> EventResponse<<C as WidgetCtx>::Event> {
        ctx.is_focused = Some(self.id) == ctx.page_state.focused;

        let behavior = self.behavior();
        if behavior.focusable {
            if let Some(common) = ctx.event.as_common() {
                match common {
                    crate::event::CommonEvent::FocusMove(_)
                        if ctx.is_focused =>
                    {
                        return Propagate::BubbleUp(self.id, ctx.event.clone())
                            .into()
                    },
                    _ => {},
                }
            }
        }

        self.widget.on_event(ctx)
    }
}
