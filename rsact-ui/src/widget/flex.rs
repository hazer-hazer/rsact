use crate::widget::{
    prelude::*, BlockModelWidget, Meta, MetaTree, SizedWidget,
};
use alloc::vec::Vec;
use core::marker::PhantomData;
use layout::flex::flex_content_size;

// pub type Row<C> = Flex<C, RowDir>;
// pub type Col<C> = Flex<C, ColDir>;

pub trait IntoChildren<W: WidgetCtx> {
    fn into_children(self) -> MaybeSignal<Vec<El<W>>>;
}

impl<W: WidgetCtx + 'static, const SIZE: usize> IntoChildren<W>
    for [El<W>; SIZE]
{
    #[track_caller]
    fn into_children(self) -> MaybeSignal<Vec<El<W>>> {
        create_signal(self.into_iter().collect()).into()
    }
}

impl<W: WidgetCtx + 'static> IntoChildren<W> for Vec<El<W>> {
    #[track_caller]
    fn into_children(self) -> MaybeSignal<Vec<El<W>>> {
        create_signal(self).into()
    }
}

impl<W: WidgetCtx + 'static> IntoChildren<W> for Signal<Vec<El<W>>> {
    fn into_children(self) -> MaybeSignal<Vec<El<W>>> {
        self.into()
    }
}

pub struct Flex<W: WidgetCtx, Dir: Direction> {
    // TODO: Signal vector?
    children: MaybeSignal<Vec<El<W>>>,
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

        let content_size = children
            .map(|children| flex_content_size(Dir::AXIS, children.iter()));

        Self {
            children,
            layout: Layout::shrink(LayoutKind::Flex(FlexLayout::base(
                Dir::AXIS,
                content_size,
            )))
            .signal(),
            dir: PhantomData,
        }
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.layout.update_untracked(|layout| {
            layout.expect_flex_mut().wrap = wrap;
        });
        self
    }

    pub fn gap(mut self, gap: impl Into<Size>) -> Self {
        self.layout.update_untracked(|layout| {
            layout.expect_flex_mut().gap = gap.into();
        });
        self
    }

    pub fn vertical_align(mut self, vertical_align: impl Into<Align>) -> Self {
        self.layout.update_untracked(|layout| {
            layout.expect_flex_mut().vertical_align = vertical_align.into();
        });
        self
    }

    pub fn horizontal_align(
        mut self,
        horizontal_align: impl Into<Align>,
    ) -> Self {
        self.layout.update_untracked(|layout| {
            layout.expect_flex_mut().horizontal_align = horizontal_align.into();
        });
        self
    }

    pub fn center(self) -> Self {
        self.vertical_align(Align::Center).horizontal_align(Align::Center)
    }

    // pub fn wrap(self, wrap: impl MaybeSignal<bool> + 'static) -> Self {
    //     self.layout.setter(wrap.maybe_signal(), |&wrap, layout| {
    //         layout.expect_flex_mut().wrap = wrap
    //     });
    //     self
    // }

    // pub fn gap<G: Into<Size> + Copy + 'static>(
    //     self,
    //     gap: impl MaybeSignal<G> + 'static,
    // ) -> Self {
    //     self.layout.setter(gap.maybe_signal(), |&gap, layout| {
    //         layout.expect_flex_mut().gap = gap.into();
    //     });
    //     self
    // }

    // pub fn vertical_align(
    //     self,
    //     vertical_align: impl MaybeSignal<Align> + 'static,
    // ) -> Self {
    //     self.layout.setter(
    //         vertical_align.maybe_signal(),
    //         |&vertical_align, layout| {
    //             layout.expect_flex_mut().vertical_align = vertical_align
    //         },
    //     );
    //     self
    // }

    // pub fn center(self) -> Self {
    //     self.vertical_align(Align::Center).horizontal_align(Align::Center)
    // }

    // pub fn horizontal_align(
    //     self,
    //     horizontal_align: impl MaybeSignal<Align> + 'static,
    // ) -> Self {
    //     self.layout.setter(
    //         horizontal_align.maybe_signal(),
    //         |&horizontal_align, layout| {
    //             layout.expect_flex_mut().horizontal_align = horizontal_align
    //         },
    //     );
    //     self
    // }
}

impl<W: WidgetCtx + 'static, Dir: Direction> SizedWidget<W> for Flex<W, Dir> {}
impl<W: WidgetCtx + 'static, Dir: Direction> BlockModelWidget<W>
    for Flex<W, Dir>
{
}

impl<W: WidgetCtx + 'static, Dir: Direction> Widget<W> for Flex<W, Dir> {
    fn meta(&self) -> MetaTree {
        MetaTree {
            data: Meta::none.memo(),
            children: self.children.map_reactive(|children| {
                children.iter().map(Widget::meta).collect()
            }),
        }
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        // ctx.pass_to_children(self.children);
        // TODO: Use writable lens/computed
        self.children.update_untracked(|children| {
            children.iter_mut().for_each(|child| child.on_mount(ctx));
        })
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        MemoTree {
            data: self.layout.memo(),
            children: self.children.map_reactive(|children| {
                children.iter().map(Widget::build_layout_tree).collect()
            }),
        }
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, W>,
    ) -> crate::widget::DrawResult {
        self.children.with(|children| ctx.draw_children(children.iter()))
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse<W> {
        self.children
            .update_untracked(|children| ctx.pass_to_children(children))
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
