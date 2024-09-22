use crate::widget::{prelude::*, BoxModelWidget, SizedWidget};
use alloc::vec::Vec;
use core::marker::PhantomData;
use num::traits::SaturatingAdd;

// pub type Row<C> = Flex<C, RowDir>;
// pub type Col<C> = Flex<C, ColDir>;

pub trait IntoChildren<W: WidgetCtx> {
    fn into_children(self) -> Signal<Vec<El<W>>>;
}

impl<W: WidgetCtx + 'static, const SIZE: usize> IntoChildren<W>
    for [El<W>; SIZE]
{
    #[track_caller]
    fn into_children(self) -> Signal<Vec<El<W>>> {
        use_signal(self.into_iter().collect())
    }
}

impl<W: WidgetCtx + 'static> IntoChildren<W> for Vec<El<W>> {
    #[track_caller]
    fn into_children(self) -> Signal<Vec<El<W>>> {
        use_signal(self)
    }
}

impl<W: WidgetCtx + 'static> IntoChildren<W> for Signal<Vec<El<W>>> {
    fn into_children(self) -> Signal<Vec<El<W>>> {
        self
    }
}

pub struct Flex<W: WidgetCtx, Dir: Direction> {
    // TODO: Signal vector
    children: Signal<Vec<El<W>>>,
    layout: Signal<Layout>,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx + 'static> Flex<W, RowDir> {
    #[track_caller]
    pub fn row(children: impl IntoChildren<W>) -> Self {
        Self::new(children)
    }
}

impl<W: WidgetCtx + 'static> Flex<W, ColDir> {
    #[track_caller]
    pub fn col(children: impl IntoChildren<W>) -> Self {
        Self::new(children)
    }
}

impl<W: WidgetCtx + 'static, Dir: Direction> Flex<W, Dir> {
    #[track_caller]
    pub fn new(children: impl IntoChildren<W>) -> Self {
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

    pub fn wrap(self, wrap: impl MaybeSignal<bool> + 'static) -> Self {
        self.layout.setter(wrap.maybe_signal(), |&wrap, layout| {
            layout.expect_flex_mut().wrap = wrap
        });
        self
    }

    pub fn gap<G: Into<Size> + Copy + 'static>(
        self,
        gap: impl MaybeSignal<G> + 'static,
    ) -> Self {
        self.layout.setter(gap.maybe_signal(), |&gap, layout| {
            layout.expect_flex_mut().gap = gap.into();
        });

        self
    }

    pub fn vertical_align(
        self,
        vertical_align: impl MaybeSignal<Align> + 'static,
    ) -> Self {
        self.layout.setter(
            vertical_align.maybe_signal(),
            |&vertical_align, layout| {
                layout.expect_flex_mut().vertical_align = vertical_align
            },
        );
        self
    }

    pub fn horizontal_align(
        self,
        horizontal_align: impl MaybeSignal<Align> + 'static,
    ) -> Self {
        self.layout.setter(
            horizontal_align.maybe_signal(),
            |&horizontal_align, layout| {
                layout.expect_flex_mut().horizontal_align = horizontal_align
            },
        );
        self
    }
}

impl<W: WidgetCtx + 'static, Dir: Direction> SizedWidget<W> for Flex<W, Dir> {}
impl<W: WidgetCtx + 'static, Dir: Direction> BoxModelWidget<W>
    for Flex<W, Dir>
{
}

impl<W: WidgetCtx + 'static, Dir: Direction> Widget<W> for Flex<W, Dir> {
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

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        self.children
            .update_untracked(|children| ctx.pass_to_children(children))
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
        ctx: &mut crate::widget::DrawCtx<'_, W>,
    ) -> crate::widget::DrawResult {
        self.children.with(|children| ctx.draw_children(children))
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> crate::event::EventResponse<<W as WidgetCtx>::Event> {
        self.children.control_flow(|children| ctx.pass_to_children(children))
    }
}

impl<W, Dir> From<Flex<W, Dir>> for El<W>
where
    W: WidgetCtx + 'static,
    Dir: Direction + 'static,
{
    fn from(value: Flex<W, Dir>) -> Self {
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
