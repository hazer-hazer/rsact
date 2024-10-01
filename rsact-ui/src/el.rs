use crate::widget::prelude::*;
use alloc::boxed::Box;
use core::sync::atomic::AtomicUsize;
use rsact_core::prelude::*;

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

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        if !self.mounted {
            self.widget.on_mount(ctx)
        }
    }

    fn children_ids(&self) -> Memo<Vec<ElId>> {
        self.widget.children_ids()
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

    fn on_event(
        &mut self,
        ctx: &mut EventCtx<'_, W>,
    ) -> EventResponse<<W as WidgetCtx>::Event> {
        self.widget.on_event(ctx)
        //     ctx.is_focused = Some(self.id) == ctx.page_state.focused;

        //     let behavior = self.behavior();
        //     if behavior.focusable {
        //         if let Some(common) = ctx.event.as_common() {
        //             match common {
        //                 crate::event::CommonEvent::FocusMove(_)
        //                     if ctx.is_focused =>
        //                 {
        //                     return Propagate::BubbleUp(self.id,
        // ctx.event.clone())                         .into()
        //                 },
        //                 _ => {},
        //             }
        //         }
        //     }

        //     self.widget.on_event(ctx)
    }
}

// impl<T, W: WidgetCtx> IntoSignal<El<W>> for T
// where
//     T: Widget<W>,
// {
//     fn into_signal(self) -> Signal<El<W>> {
//         todo!()
//     }
// }

// pub trait IntoElSignal<W: WidgetCtx> {
//     fn into_el(self) -> Signal<El<W>>;
// }

// impl<W: WidgetCtx, T> IntoElSignal<W> for T
// where
//     T: ReadSignal<El<W>>,
// {
//     fn into_el(self) -> Signal<El<W>> {

//     }
// }
