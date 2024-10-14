use crate::{value::RangeValue, widget::prelude::*};
use core::marker::PhantomData;
use embedded_graphics::{
    prelude::Primitive,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, RoundedRectangle},
};
use layout::size::{RectangleExt, SizeExt};

// TODO: Padding for inner bar

declare_widget_style! {
    BarStyle () {
        container: container,
        color: color,
    }
}

impl<C: Color> BarStyle<C> {
    pub fn base() -> Self {
        Self {
            container: BlockStyle::base()
                .border(BorderStyle::base().radius(0.5)),
            color: ColorStyle::DefaultForeground,
        }
    }

    fn bar_style(&self) -> PrimitiveStyle<C> {
        let base = PrimitiveStyleBuilder::new();

        if let Some(color) = self.color.get() {
            base.fill_color(color)
        } else {
            base
        }
        .build()
    }
}

pub struct Bar<W: WidgetCtx, V: RangeValue, Dir: Direction> {
    value: Signal<V>,
    layout: Signal<Layout>,
    style: MemoChain<BarStyle<W::Color>>,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx, V: RangeValue + 'static> Bar<W, V, ColDir> {
    pub fn vertical(value: impl IntoSignal<V> + 'static) -> Self {
        Self::new(value)
    }
}

impl<W: WidgetCtx, V: RangeValue + 'static> Bar<W, V, RowDir> {
    pub fn horizontal(value: impl IntoSignal<V> + 'static) -> Self {
        Self::new(value)
    }
}

impl<W: WidgetCtx, V: RangeValue + 'static, Dir: Direction> Bar<W, V, Dir> {
    pub fn new(value: impl IntoSignal<V> + 'static) -> Self {
        Self {
            value: value.into_signal(),
            layout: Layout {
                kind: LayoutKind::Edge,
                size: Dir::AXIS.canon(Length::fill(), Length::Fixed(10)),
            }
            .into_signal(),
            style: BarStyle::base().into_memo_chain(),
            dir: PhantomData,
        }
    }
}

impl<W: WidgetCtx, V: RangeValue + 'static, Dir: Direction> Widget<W>
    for Bar<W, V, Dir>
where
    W::Styler: Styler<BarStyle<W::Color>, Class = ()>,
{
    fn meta(&self) -> super::MetaTree {
        MetaTree::childless(Meta::none())
    }

    fn on_mount(&mut self, ctx: super::MountCtx<W>) {
        ctx.accept_styles(self.style, ());
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        MemoTree::childless(self.layout)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        let style = self.style.get();

        // let start = ctx.layout.area.anchor_point(
        //     Dir::AXIS
        //         .canon::<AxisAnchorPoint>(Anchor::Start, Anchor::Center)
        //         .into(),
        // );

        // let end = start + Dir::AXIS.canon::<Point>(value_len as i32, 0);

        // let bar_width = ctx.layout.area.size.cross(Dir::AXIS);

        let block_model = self.layout.with(|layout| layout.block_model());
        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.outer,
            block_model,
            style.container,
        ))?;

        let full_len = ctx.layout.inner.size.main(Dir::AXIS);
        let value_len = self.value.get().point(full_len);

        let bar_area =
            ctx.layout.inner.resized_axis(Dir::AXIS, value_len, Anchor::Start);

        // Note: I use `max_square` for corner radius to make it so "round" as
        // user needs more likely, but this is not really the right way. Better
        // add `BorderRadius::MaxSquare` variant to make it look like a sausage
        // instead of UFO
        ctx.renderer.rect(
            RoundedRectangle::new(
                bar_area,
                style
                    .container
                    .border
                    .radius
                    .into_corner_radii(bar_area.size.max_square()),
            )
            .into_styled(style.bar_style()),
        )?;

        // ctx.renderer.line(
        //     Line::new(start, end).into_styled(style.line_style(bar_width)),
        // )?;

        Ok(())
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse<W> {
        let _ = ctx;
        W::ignore()
    }
}
