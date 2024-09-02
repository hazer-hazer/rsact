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
    #[track_caller]
    fn into_children(self) -> Signal<Vec<El<C>>> {
        use_signal(self.into_iter().collect())
    }
}

impl<C: WidgetCtx + 'static> IntoChildren<C> for Vec<El<C>> {
    #[track_caller]
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
    #[track_caller]
    pub fn row(children: impl IntoChildren<C>) -> Self {
        Self::new(children)
    }
}

impl<C: WidgetCtx + 'static> Flex<C, ColDir> {
    #[track_caller]
    pub fn col(children: impl IntoChildren<C>) -> Self {
        Self::new(children)
    }
}

impl<C: WidgetCtx + 'static, Dir: Direction> Flex<C, Dir> {
    #[track_caller]
    pub fn new(children: impl IntoChildren<C>) -> Self {
        let children = children.into_children();

        let content_size = children.mapped(|children| {
            children.iter().fold(Limits::unlimited(), |limits, child| {
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
        });

        Self {
            children,
            layout: Layout {
                kind: LayoutKind::Flex(FlexLayout::base(Dir::AXIS)),
                size: Size::shrink(),
                box_model: BoxModel::zero(),
                content_size,
            }
            .into_signal(),
            dir: PhantomData,
        }
    }

    pub fn wrap(self, wrap: impl EcoSignal<bool> + 'static) -> Self {
        self.layout.setter(wrap.eco_signal(), |&wrap, layout| {
            layout.expect_flex_mut().wrap = wrap
        });
        self
    }

    pub fn gap<G: Into<Size> + Copy + 'static>(
        self,
        gap: impl EcoSignal<G> + 'static,
    ) -> Self {
        self.layout.setter(gap.eco_signal(), |&gap, layout| {
            layout.expect_flex_mut().gap = gap.into();
        });

        self
    }

    pub fn vertical_align(
        self,
        vertical_align: impl EcoSignal<Align> + 'static,
    ) -> Self {
        self.layout.setter(
            vertical_align.eco_signal(),
            |&vertical_align, layout| {
                layout.expect_flex_mut().vertical_align = vertical_align
            },
        );
        self
    }

    pub fn horizontal_align(
        self,
        horizontal_align: impl EcoSignal<Align> + 'static,
    ) -> Self {
        self.layout.setter(
            horizontal_align.eco_signal(),
            |&horizontal_align, layout| {
                layout.expect_flex_mut().horizontal_align = horizontal_align
            },
        );
        self
    }
}

impl<C: WidgetCtx + 'static, Dir: Direction> Widget<C> for Flex<C, Dir> {
    fn children_ids(&self) -> Memo<Vec<ElId>> {
        let children = self.children;
        use_memo(move |_| {
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

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        let children = self.children;
        MemoTree {
            data: self.layout.into_memo(),
            children: use_memo(move |_| {
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

impl<C, Dir> From<Flex<C, Dir>> for El<C>
where
    C: WidgetCtx + 'static,
    Dir: Direction + 'static,
{
    fn from(value: Flex<C, Dir>) -> Self {
        El::new(value)
    }
}

// macro_rules! row {
//     ($($el: expr),* $(,)?) => [
//         Flex::row(vec![$($el.el()),*])
//     ];

//     (let ($var: ident, $flex: ident) = $($el: expr),* $(,)?) => {
//         let $var = use_signal(vec![$($el.el()),*]);
//         let flex =
//     };
// }
