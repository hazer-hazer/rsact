use crate::{
    render::{Renderable as _, primitives::rounded_rect::RoundedRect},
    value::RangeValue,
    widget::prelude::*,
};
use core::marker::PhantomData;
use embedded_graphics::{
    prelude::Primitive,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder},
};
use layout::size::RectangleExt;
use rsact_reactive::maybe::IntoMaybeReactive;

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
                .border(BorderStyle::base().radius(0.4)),
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

#[derive(Clone)]
pub struct Bar<W: WidgetCtx, V: RangeValue, Dir: Direction> {
    value: MaybeReactive<V>,
    layout: Layout,
    style: MemoChain<BarStyle<W::Color>>,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx, V: RangeValue + 'static> Bar<W, V, ColDir> {
    pub fn vertical(value: impl IntoMaybeReactive<V>) -> Self {
        Self::new(value)
    }
}

impl<W: WidgetCtx, V: RangeValue + 'static> Bar<W, V, RowDir> {
    pub fn horizontal(value: impl IntoMaybeReactive<V>) -> Self {
        Self::new(value)
    }
}

impl<W: WidgetCtx, V: RangeValue + 'static, Dir: Direction> Bar<W, V, Dir> {
    pub fn new(value: impl IntoMaybeReactive<V>) -> Self {
        Self {
            value: value.maybe_reactive(),
            layout: Layout::edge(
                Dir::AXIS.canon(Length::fill(), Length::Fixed(10)),
            ),
            style: BarStyle::base().memo_chain(),
            dir: PhantomData,
        }
    }
}

impl<W: WidgetCtx, V: RangeValue + 'static, Dir: Direction> Widget<W>
    for Bar<W, V, Dir>
where
    W::Styler: WidgetStylist<BarStyle<W::Color>>,
{
    fn meta(&self) -> super::MetaTree {
        MetaTree::childless(Meta::none)
    }

    fn on_mount(&mut self, ctx: super::MountCtx<W>) {
        ctx.accept_styles(self.style, ());
    }

    fn layout(&self) -> &Layout {
        &self.layout
    }

    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }

    fn render(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        let style = self.style.get();

        // let start = ctx.layout.area.anchor_point(
        //     Dir::AXIS
        //         .canon::<AxisAnchorPoint>(Anchor::Start, Anchor::Center)
        //         .into(),
        // );

        // let end = start + Dir::AXIS.canon::<Point>(value_len as i32, 0);

        // let bar_width = ctx.layout.area.size.cross(Dir::AXIS);

        let block_model = self.layout.block_model();
        Block::from_layout_style(
            ctx.layout.outer,
            block_model,
            style.container,
        )
        .render(ctx.renderer)?;

        let full_len = ctx.layout.inner.size.main(Dir::AXIS);
        let value_len = self.value.get().point(full_len);

        let bar_area =
            ctx.layout.inner.resized_axis(Dir::AXIS, value_len, Anchor::Start);

        RoundedRect::new(bar_area, style.container.border.radius)
            .into_styled(style.bar_style())
            .render(ctx.renderer)?;

        // ctx.renderer.line(
        //     Line::new(start, end).into_styled(style.line_style(bar_width)),
        // )?;

        Ok(())
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse {
        let _ = ctx;
        ctx.ignore()
    }
}
