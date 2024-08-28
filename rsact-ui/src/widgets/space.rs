use core::marker::PhantomData;

use rsact_core::{
    prelude::{use_computed, use_signal},
    signal::{Signal, SignalTree},
};

use crate::{
    event::{EventResponse, Propagate},
    layout::{
        axis::{ColDir, Direction, RowDir},
        box_model::BoxModel,
        size::{Length, Size},
        Layout, Limits,
    },
    widget::{DrawCtx, DrawResult, EventCtx, Widget, WidgetCtx},
};

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
            layout: use_signal(Layout {
                kind: crate::layout::LayoutKind::Edge(
                    crate::layout::EdgeLayout {},
                ),
                size: Dir::AXIS.canon(length.into(), Length::fill()),
                box_model: BoxModel::zero(),
                content_size: use_computed(Limits::unknown),
            }),
            ctx: PhantomData,
            dir: PhantomData,
        }
    }
}

impl<C: WidgetCtx, Dir: Direction> Widget<C> for Space<C, Dir> {
    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> rsact_core::signal::SignalTree<Layout> {
        SignalTree::childless(self.layout)
    }

    fn draw(&self, _ctx: &mut DrawCtx<'_, C>) -> DrawResult {
        Ok(())
    }

    fn on_event(
        &mut self,
        ctx: &mut EventCtx<'_, C>,
    ) -> EventResponse<<C as WidgetCtx>::Event> {
        Propagate::Ignored.into()
    }
}
