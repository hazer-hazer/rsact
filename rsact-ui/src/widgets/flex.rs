use crate::{
    el::El,
    event::Propagate,
    layout::{
        axis::{Axial as _, Axis},
        box_model::BoxModel,
        size::{Length, Size},
        Align, FlexLayout, Layout, LayoutKind, Limits,
    },
    widget::{Widget, WidgetCtx},
};
use alloc::vec::Vec;
use core::marker::PhantomData;
use num::traits::SaturatingAdd;
use rsact_core::{
    prelude::*,
    signal::{EcoSignal, IntoSignal, ReadSignal, SignalTree},
};

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
    children: Signal<Vec<Signal<El<C>>>>,
    layout: Signal<Layout>,
    dir: PhantomData<Dir>,
}

impl<C: WidgetCtx + 'static> Flex<C, RowDir> {
    pub fn row<I: IntoSignal<El<C>> + 'static>(
        children: impl IntoIterator<Item = I>,
    ) -> Self {
        Self::new(children)
    }
}

impl<C: WidgetCtx + 'static> Flex<C, ColDir> {
    pub fn col<I: IntoSignal<El<C>> + 'static>(
        children: impl IntoIterator<Item = I>,
    ) -> Self {
        Self::new(children)
    }
}

impl<C: WidgetCtx + 'static, Dir: FlexDir> Flex<C, Dir> {
    pub fn new<I: IntoSignal<El<C>> + 'static>(
        children: impl IntoIterator<Item = I>,
    ) -> Self {
        let children = use_signal(
            children.into_iter().map(IntoSignal::signal).collect::<Vec<_>>(),
        );

        let content_size = use_computed(move || {
            children.with(|children| {
                children.iter().fold(Limits::unknown(), |limits, child| {
                    let child_limits = child.with(|child| {
                        child.layout().with(|child| child.content_size.get())
                    });
                    // let child = child.get();
                    Limits::new(
                        limits.min().min(child_limits.min()),
                        Dir::AXIS.infix(
                            limits.max(),
                            child_limits.max(),
                            |arg0: u32, v: u32| {
                                SaturatingAdd::saturating_add(&arg0, &v)
                            },
                            core::cmp::max,
                        ),
                    )
                })
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

    pub fn wrap(self, wrap: impl EcoSignal<bool> + 'static) -> Self {
        let wrap = wrap.eco_signal();
        use_memo(move || {
            let wrap = wrap.get();
            self.layout.update(move |layout| {
                layout.expect_flex_mut().wrap = wrap;
            });
            wrap
        });
        self
    }

    pub fn gap<G: Into<Size> + Copy + 'static>(
        self,
        gap: impl EcoSignal<G> + 'static,
    ) -> Self {
        let gap = gap.eco_signal();
        use_memo(move || {
            let gap = gap.get().into();
            self.layout.update(move |layout| {
                layout.expect_flex_mut().gap = gap;
            });
            gap
        });
        self
    }

    pub fn vertical_align(
        self,
        vertical_align: impl EcoSignal<Align> + 'static,
    ) -> Self {
        let vertical_align = vertical_align.eco_signal();
        use_memo(move || {
            let vertical_align = vertical_align.get();
            self.layout.update(move |layout| {
                layout.expect_flex_mut().vertical_align = vertical_align
            });
            vertical_align
        });
        self
    }

    pub fn horizontal_align(
        self,
        horizontal_align: impl EcoSignal<Align> + 'static,
    ) -> Self {
        let horizontal_align = horizontal_align.eco_signal();
        use_memo(move || {
            let horizontal_align = horizontal_align.get();
            self.layout.update(move |layout| {
                layout.expect_flex_mut().horizontal_align = horizontal_align
            });
            horizontal_align
        });
        self
    }
}

impl<C: WidgetCtx + 'static, Dir: FlexDir> Widget<C> for Flex<C, Dir> {
    // fn children(&self) -> &[El<C>] {
    //     &self.children
    // }

    // fn children_mut(&mut self) -> &mut [El<C>] {
    //     &mut self.children
    // }

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

    fn build_layout_tree(&self) -> SignalTree<Layout> {
        SignalTree {
            data: self.layout,
            children: self.children.with(|children| {
                children
                    .iter()
                    .map(|child| child.with(Widget::build_layout_tree))
                    .collect()
            }),
        }
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, C>,
    ) -> crate::widget::DrawResult {
        self.children.with(|children| ctx.draw_children(children))
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, C>,
    ) -> crate::event::EventResponse<<C as WidgetCtx>::Event> {
        self.children.maybe_update(|children| ctx.pass_to_children(children))
    }
}
