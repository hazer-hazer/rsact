use crate::{
    el::El,
    layout::{
        axis::{Axial as _, Axis},
        box_model::BoxModel,
        size::{Length, Size},
        FlexLayout, Layout, LayoutKind, Limits,
    },
    widget::{Widget, WidgetCtx},
};
use alloc::vec::Vec;
use core::marker::PhantomData;
use num::traits::SaturatingAdd;
use rsact_core::{prelude::*, signal::ReadSignal};

pub trait FlexDir {
    const AXIS: Axis;
}

pub struct RowDir;
impl FlexDir for RowDir {
    const AXIS: Axis = Axis::X;
}

pub struct ColDir;
impl FlexDir for ColDir {
    const AXIS: Axis = Axis::Y;
}

// pub type Row<C> = Flex<C, RowDir>;
// pub type Col<C> = Flex<C, ColDir>;

pub struct Flex<C: WidgetCtx, Dir: FlexDir> {
    // TODO: Signal vector
    children: Vec<El<C>>,
    layout: Signal<Layout>,
    dir: PhantomData<Dir>,
}

impl<C: WidgetCtx> Flex<C, RowDir> {
    pub fn row(children: impl IntoIterator<Item = El<C>>) -> Self {
        Self::new(children)
    }
}

impl<C: WidgetCtx> Flex<C, ColDir> {
    pub fn col(children: impl IntoIterator<Item = El<C>>) -> Self {
        Self::new(children)
    }
}

impl<C: WidgetCtx, Dir: FlexDir> Flex<C, Dir> {
    pub fn new(children: impl IntoIterator<Item = El<C>>) -> Self {
        let children: Vec<_> = children.into_iter().collect();

        let children_limits = children
            .iter()
            .map(|child| child.layout().with(|child| child.content_size))
            .collect::<Vec<_>>();

        let content_size = use_computed(move || {
            children_limits.iter().fold(Limits::unknown(), |limits, child| {
                let child = child.get();
                Limits::new(
                    limits.min().min(child.min()),
                    Dir::AXIS.infix(
                        limits.max(),
                        child.max(),
                        |arg0: u32, v: u32| {
                            SaturatingAdd::saturating_add(&arg0, &v)
                        },
                        core::cmp::max,
                    ),
                )
            })
        });

        Self {
            children,
            layout: use_signal(Layout {
                kind: LayoutKind::Flex(FlexLayout::base(Dir::AXIS)),
                size: Size::shrink(),
                box_model: BoxModel::zero(),
                content_size,
            }),
            dir: PhantomData,
        }
    }
}

impl<C: WidgetCtx, Dir: FlexDir> Widget<C> for Flex<C, Dir> {
    fn children(&self) -> &[El<C>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut [El<C>] {
        &mut self.children
    }

    // fn size(&self) -> Size<Length> {
    //     self.layout.size.get()
    // }

    // fn content_size(&self) -> Limits {
    //     // TODO: Cache
    // }

    // fn layout(&self, _ctx: &crate::widget::LayoutCtx<'_, C>) -> LayoutKind {
    //     LayoutKind::Flex(self.layout.kind.get())
    // }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, C>,
    ) -> crate::widget::DrawResult {
        ctx.draw_children(&self.children)
    }
}
