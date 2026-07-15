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
#[derive(Builder)]
#[builds(Flex<W>)]
pub struct FlexBuilder<W: WidgetCtx> {
    // TODO: Signal-vector type?
    // TODO: Can we do fixed size?
    #[children(reactive)]
    children: MaybeSignal<Vec<El<W>>>,
    #[widget]
    layout: LayoutBuilder<W>,
    // Moved 1:1 by the derive into the retained `Flex { layout, ctx }`:
    // the by-name build transform declares every field the widget has.
    #[widget]
    ctx: PhantomData<W>,
}

pub struct Flex<W: WidgetCtx> {
    layout: LayoutData,
    // `W` is otherwise unused on the retained widget (unlike `FlexBuilder`,
    // which threads it through `children: MaybeSignal<Vec<El<W>>>`) — kept
    // only to satisfy `Widget<W>`'s own `W` parameter, same as `space.rs`.
    ctx: PhantomData<W>,
}

impl<W: WidgetCtx + 'static> Flex<W> {
    #[track_caller]
    pub fn row(children: impl ViewSequence<W>) -> FlexBuilder<W> {
        FlexBuilder::new(children, Axis::X)
    }

    #[track_caller]
    pub fn col(children: impl ViewSequence<W>) -> FlexBuilder<W> {
        FlexBuilder::new(children, Axis::Y)
    }
}

impl<W: WidgetCtx + 'static> FlexBuilder<W> {
    #[track_caller]
    fn new(children: impl ViewSequence<W>, axis: Axis) -> Self {
        let children = children.into_children();

        // WS5.1: the flex's children come from the arena (via `set_children`);
        // the layout no longer collects child layout handles.
        Self {
            children,
            layout: LayoutBuilder::shrink(LayoutKind::Flex(FlexLayout::base(
                axis,
            ))),
            ctx: PhantomData,
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
        self.vertical_align(Align::Center)
            .horizontal_align(Align::Center)
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

impl<W: WidgetCtx + 'static> LayoutWidget<W> for FlexBuilder<W> {
    fn layout_mut(&mut self) -> &mut LayoutBuilder<W> {
        &mut self.layout
    }
}
impl<W: WidgetCtx + 'static> SizedWidget<W> for FlexBuilder<W> {}
impl<W: WidgetCtx + 'static> BlockModelWidget<W> for FlexBuilder<W> {}
impl<W: WidgetCtx + 'static> FontSettingWidget<W> for FlexBuilder<W> {}

impl<W: WidgetCtx + 'static> Widget<W> for Flex<W> {
    // NOTE: no `debug_name`/`flags` override on the retained widget — read once
    // pre-build from `Build` (seeding `ElState`); post-build consumption is via
    // `ElState`. `Build::debug_name` on `FlexBuilder` returns "Flex".
    fn render(&self, _ctx: RenderCtx<'_, W>) -> crate::widget::RenderResult {
        Ok(())
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.ignore()
    }
}

pub trait FlexExt<W: WidgetCtx> {
    #[allow(non_snake_case)]
    fn Col(self) -> FlexBuilder<W>;
    #[allow(non_snake_case)]
    fn Row(self) -> FlexBuilder<W>;
}

impl<W: WidgetCtx + 'static, T: ViewSequence<W>> FlexExt<W> for T {
    fn Col(self) -> FlexBuilder<W> {
        Flex::col(self)
    }

    fn Row(self) -> FlexBuilder<W> {
        Flex::row(self)
    }
}
