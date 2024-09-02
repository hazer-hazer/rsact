use crate::widget::prelude::*;
use crate::{
    el::El,
    event::{EventResponse, Propagate},
    layout::{
        axis::{ColDir, Direction, RowDir},
        box_model::BoxModel,
        size::{Length, Size},
        Layout, Limits,
    },
    widget::{DrawCtx, DrawResult, EventCtx, Widget, WidgetCtx},
};
use core::marker::PhantomData;

pub struct Space<C: WidgetCtx, Dir: Direction> {
    layout: Signal<Layout>,
    ctx: PhantomData<C>,
    dir: PhantomData<Dir>,
}

impl<C: WidgetCtx> Space<C, RowDir> {
    pub fn row(length: impl Into<Length>) -> Self {
        Self::new(length)
    }
}

impl<C: WidgetCtx> Space<C, ColDir> {
    pub fn col(length: impl Into<Length>) -> Self {
        Self::new(length)
    }
}

impl<C: WidgetCtx, Dir: Direction> Space<C, Dir> {
    pub fn new(length: impl Into<Length>) -> Self {
        Self {
            layout: Layout::new(
                crate::layout::LayoutKind::Edge,
                Limits::zero().into_memo(),
            )
            .size(Dir::AXIS.canon(length.into(), Length::fill()))
            .into_signal(),
            ctx: PhantomData,
            dir: PhantomData,
        }
    }
}

impl<C: WidgetCtx, Dir: Direction> Widget<C> for Space<C, Dir> {
    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        MemoTree::childless(self.layout.into_memo())
    }

    fn draw(&self, _ctx: &mut DrawCtx<'_, C>) -> DrawResult {
        Ok(())
    }

    fn on_event(
        &mut self,
        _ctx: &mut EventCtx<'_, C>,
    ) -> EventResponse<<C as WidgetCtx>::Event> {
        Propagate::Ignored.into()
    }
}

impl<C, Dir> From<Space<C, Dir>> for El<C>
where
    C: WidgetCtx + 'static,
    Dir: Direction + 'static,
{
    fn from(value: Space<C, Dir>) -> Self {
        El::new(value)
    }
}
