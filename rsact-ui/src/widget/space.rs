use crate::layout::LayoutKind;
use crate::widget::{Meta, MetaTree, prelude::*};
use crate::{
    el::El,
    event::EventResponse,
    layout::{
        Layout,
        axis::{ColDir, Direction, RowDir},
        size::Length,
    },
    widget::{EventCtx, RenderCtx, RenderResult, Widget, WidgetCtx},
};
use core::marker::PhantomData;

pub struct Space<W: WidgetCtx, Dir: Direction> {
    layout: Signal<Layout>,
    ctx: PhantomData<W>,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx> Space<W, RowDir> {
    // pub fn row<L: Into<Length> + Clone + PartialEq + 'static>(
    //     length: impl AsMemo<L>,
    // ) -> Self {
    //     Self::new(length)
    // }

    pub fn row(length: impl Into<Length>) -> Self {
        Self::new(length)
    }
}

impl<W: WidgetCtx> Space<W, ColDir> {
    // pub fn col<L: Into<Length> + Clone + PartialEq + 'static>(
    //     length: impl AsMemo<L>,
    // ) -> Self {
    //     Self::new(length)
    // }

    pub fn col(length: impl Into<Length>) -> Self {
        Self::new(length)
    }
}

impl<W: WidgetCtx, Dir: Direction> Space<W, Dir> {
    // pub fn new<L: Into<Length> + Clone + PartialEq + 'static>(
    //     length: impl AsMemo<L>,
    // ) -> Self {
    //     let length = length.as_memo();
    //     let layout = Layout::shrink(LayoutKind::Edge).into_signal();

    //     layout.setter(length, move |length, layout| {
    //         layout.size =
    //             Dir::AXIS.canon(length.clone().into(), Length::fill());
    //     });

    //     Self { layout, ctx: PhantomData, dir: PhantomData }
    // }

    // TODO: Reactive length, MaybeReactive
    pub fn new(length: impl Into<Length>) -> Self {
        let layout = Layout::shrink(LayoutKind::Edge)
            .size(Dir::AXIS.canon(length.into(), Length::fill()))
            .signal();

        Self { layout, ctx: PhantomData, dir: PhantomData }
    }
}

impl<W: WidgetCtx, Dir: Direction> Widget<W> for Space<W, Dir> {
    fn meta(&self, _: ElId) -> MetaTree {
        MetaTree::childless(Meta::none)
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn on_mount(&mut self, _ctx: crate::widget::MountCtx<W>) {}

    fn render(&self, _ctx: &mut RenderCtx<'_, W>) -> RenderResult {
        Ok(())
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.ignore()
    }
}

impl<W, Dir> From<Space<W, Dir>> for El<W>
where
    W: WidgetCtx + 'static,
    Dir: Direction + 'static,
{
    fn from(value: Space<W, Dir>) -> Self {
        El::new(value)
    }
}
