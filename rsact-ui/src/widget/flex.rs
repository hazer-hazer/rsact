use crate::widget::{BlockModelWidget, SizedWidget, prelude::*};
use alloc::vec::Vec;
use core::marker::PhantomData;
use rsact_reactive::prelude::*;

// pub type Row<C> = Flex<C, RowDir>;
// pub type Col<C> = Flex<C, ColDir>;

// TODO: Do we need flex style? Using Container as combinator to have box style
// in flex may not be handy declare_widget_style! {
//     FlexStyle () {
//         container: container,
//     }
// }

// TODO: Shouldn't Flex support changing direction so we need to store a field instead of using a const param.
#[derive(View)]
pub struct Flex<W: WidgetCtx, Dir: Direction> {
    // TODO: Signal-vector type?
    // TODO: Can we do fixed size?
    children: MaybeSignal<Vec<El<W>>>,
    layout: Layout,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx + 'static> Flex<W, RowDir> {
    #[track_caller]
    pub fn row(children: impl ViewSequence<W>) -> Self {
        Self::new(children)
    }
}

impl<W: WidgetCtx + 'static> Flex<W, ColDir> {
    #[track_caller]
    pub fn col(children: impl ViewSequence<W>) -> Self {
        Self::new(children)
    }
}

impl<W: WidgetCtx + 'static, Dir: Direction> Flex<W, Dir> {
    #[track_caller]
    fn new(children: impl ViewSequence<W>) -> Self {
        let children = children.into_children();

        let layout_children = children.map(|children| {
            children.iter().map(|child| child.layout()).collect()
        });

        Self {
            children,
            layout: Layout::shrink(LayoutKind::Flex(FlexLayout::base(
                Dir::AXIS,
                layout_children,
            ))),
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

impl<W: WidgetCtx + 'static, Dir: Direction + 'static> LayoutWidget<W>
    for Flex<W, Dir>
{
    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }
}

impl<W: WidgetCtx + 'static, Dir: Direction + 'static> SizedWidget<W>
    for Flex<W, Dir>
{
}
impl<W: WidgetCtx + 'static, Dir: Direction + 'static> BlockModelWidget<W>
    for Flex<W, Dir>
{
}
impl<W: WidgetCtx + 'static, Dir: Direction + 'static> FontSettingWidget<W>
    for Flex<W, Dir>
{
}

impl<W: WidgetCtx + 'static, Dir: Direction + 'static> Widget<W>
    for Flex<W, Dir>
{
    fn debug_name(&self) -> &'static str {
        "Flex"
    }

    fn build(&mut self, mut ctx: build::BuildCtx<W>) {
        self.children.maybe_effect(move |children, _| {
            // TODO: Reconcile children, this does not delete old children now.
            ctx.set_children(children);
        });
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    fn render(&self, _ctx: RenderCtx<'_, W>) -> crate::widget::RenderResult {
        Ok(())
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.ignore()
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

pub trait FlexExt<W: WidgetCtx> {
    fn col(self) -> Flex<W, ColDir>;
    fn row(self) -> Flex<W, RowDir>;
}

impl<W: WidgetCtx, T: ViewSequence<W>> FlexExt<W> for T {
    fn col(self) -> Flex<W, ColDir> {
        Flex::col(self)
    }

    fn row(self) -> Flex<W, RowDir> {
        Flex::row(self)
    }
}
