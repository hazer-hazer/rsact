use crate::widget::{
    BlockModelWidget, Meta, MetaTree, SizedWidget, prelude::*,
};
use alloc::vec::Vec;
use core::marker::PhantomData;
use rsact_reactive::maybe::IntoMaybeReactive;

// pub type Row<C> = Flex<C, RowDir>;
// pub type Col<C> = Flex<C, ColDir>;

// TODO: Do we need flex style?
// declare_widget_style! {
//     FlexStyle () {
//         container: container,
//     }
// }

#[macro_export]
macro_rules! row {
    ($($el: expr),* $(,)?) => [
        Flex::row([
            $($el.el()),*
        ])
    ];
}
#[macro_export]
macro_rules! col {
    ($($el: expr),* $(,)?) => [
        Flex::col([
            $($el.el()),*
        ])
    ];
}
pub use col;
pub use row;

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
    // TODO: Can we do fixed size?
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

        let layout_children = children.map_reactive(|children| {
            children.iter().map(|child| child.layout().memo()).collect()
        });

        Self {
            children,
            layout: Layout::shrink(LayoutKind::Flex(FlexLayout::base(
                Dir::AXIS,
                layout_children,
            )))
            .signal(),
            dir: PhantomData,
        }
    }

    pub fn wrap(mut self, wrap: impl IntoMaybeReactive<bool>) -> Self {
        self.layout.setter(wrap.maybe_reactive(), |layout, &wrap| {
            layout.expect_flex_mut().wrap = wrap;
        });
        self
    }

    pub fn gap<G: Into<Size> + Copy + PartialEq + 'static>(
        mut self,
        gap: impl IntoMaybeReactive<G>,
    ) -> Self {
        self.layout.setter(gap.maybe_reactive(), |layout, &gap| {
            layout.expect_flex_mut().gap = gap.into();
        });
        self
    }

    pub fn vertical_align(
        mut self,
        vertical_align: impl IntoMaybeReactive<Align>,
    ) -> Self {
        self.layout.setter(
            vertical_align.maybe_reactive(),
            |layout, &vertical_align| {
                layout.expect_flex_mut().vertical_align = vertical_align;
            },
        );
        self
    }

    pub fn horizontal_align(
        mut self,
        horizontal_align: impl IntoMaybeReactive<Align>,
    ) -> Self {
        self.layout.setter(
            horizontal_align.maybe_reactive(),
            |layout, &horizontal_align| {
                layout.expect_flex_mut().horizontal_align = horizontal_align;
            },
        );
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
impl<W: WidgetCtx + 'static, Dir: Direction + 'static> FontSettingWidget<W>
    for Flex<W, Dir>
{
}

impl<W: WidgetCtx + 'static, Dir: Direction> Widget<W> for Flex<W, Dir> {
    fn meta(&self, id: ElId) -> MetaTree {
        MetaTree {
            data: Meta::none.memo(),
            children: self.children.map_reactive(move |children| {
                children.iter().map(|child| child.meta(id)).collect()
            }),
        }
    }

    fn on_mount(&mut self, ctx: MountCtx<W>) {
        ctx.pass_to_children(self.layout, &mut self.children);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn render(
        &self,
        ctx: &mut RenderCtx<'_, W>,
    ) -> crate::widget::RenderResult {
        ctx.render_children(&self.children)
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        self.children
            .update_untracked(move |children| ctx.pass_to_children(children))
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
