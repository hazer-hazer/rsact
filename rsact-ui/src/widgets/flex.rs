use crate::widget::prelude::*;
use alloc::vec::Vec;
use core::marker::PhantomData;
use num::traits::SaturatingAdd;

// pub type Row<C> = Flex<C, RowDir>;
// pub type Col<C> = Flex<C, ColDir>;

pub trait IntoChildren<C: WidgetCtx> {
    fn into_children(self) -> Signal<Vec<El<C>>>;
}

impl<C: WidgetCtx + 'static, const SIZE: usize> IntoChildren<C>
    for [El<C>; SIZE]
{
    fn into_children(self) -> Signal<Vec<El<C>>> {
        use_signal(self.into_iter().collect())
    }
}

impl<C: WidgetCtx + 'static> IntoChildren<C> for Vec<El<C>> {
    fn into_children(self) -> Signal<Vec<El<C>>> {
        use_signal(self)
    }
}

impl<C: WidgetCtx + 'static> IntoChildren<C> for Signal<Vec<El<C>>> {
    fn into_children(self) -> Signal<Vec<El<C>>> {
        self
    }
}

pub struct Flex<C: WidgetCtx, Dir: Direction> {
    // TODO: Signal vector
    children: Signal<Vec<El<C>>>,
    layout: Signal<Layout>,
    dir: PhantomData<Dir>,
}

impl<C: WidgetCtx + 'static> Flex<C, RowDir> {
    pub fn row(children: impl IntoChildren<C>) -> Self {
        Self::new(children)
    }
}

impl<C: WidgetCtx + 'static> Flex<C, ColDir> {
    pub fn col(children: impl IntoChildren<C>) -> Self {
        Self::new(children)
    }
}

impl<C: WidgetCtx + 'static, Dir: Direction> Flex<C, Dir> {
    pub fn new(children: impl IntoChildren<C>) -> Self {
        let children = children.into_children();

        let content_size = use_computed(move || {
            // println!("Recompute content size");
            children.with(|children| {
                children.iter().fold(Limits::unknown(), |limits, child| {
                    let child_limits =
                        child.layout().with(|child| child.content_size.get());
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
            layout: use_computed(move || Layout {
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

impl<C: WidgetCtx + 'static, Dir: Direction> Widget<C> for Flex<C, Dir> {
    fn children_ids(&self) -> Signal<Vec<ElId>> {
        let children = self.children;
        use_computed(move || {
            children.with(|children| {
                children
                    .iter()
                    .map(|child| {
                        child
                            .children_ids()
                            .with(|ids| ids.iter().copied().collect::<Vec<_>>())
                    })
                    .flatten()
                    .collect()
            })
        })
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> SignalTree<Layout> {
        let children = self.children;
        SignalTree {
            data: self.layout,
            children: use_computed(move || {
                children.with(|children| {
                    children.iter().map(Widget::build_layout_tree).collect()
                })
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
        self.children.control_flow(|children| ctx.pass_to_children(children))
    }
}
