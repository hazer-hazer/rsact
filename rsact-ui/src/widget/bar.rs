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

    fn render(&self, ctx: RenderCtx<W>) -> Computed<()> {
        let style = self.style;
        let value = self.value;
        let block_model = self.layout.block_model();

        ctx.render(move |renderer, layout| {
            let style = style.get();

            // let start = ctx.layout.area.anchor_point(
            //     Dir::AXIS
            //         .canon::<AxisAnchorPoint>(Anchor::Start, Anchor::Center)
            //         .into(),
            // );

            // let end = start + Dir::AXIS.canon::<Point>(value_len as i32, 0);

            // let bar_width = ctx.layout.area.size.cross(Dir::AXIS);

            Block::from_layout_style(
                layout.outer,
                block_model.get(),
                style.container,
            )
            .render(renderer);

            let full_len = layout.inner.size.main(Dir::AXIS);
            let value_len = value.get().point(full_len);

            let bar_area =
                layout.inner.resized_axis(Dir::AXIS, value_len, Anchor::Start);

            RoundedRect::new(bar_area, style.container.border.radius)
                .into_styled(style.bar_style())
                .render(renderer);

            // ctx.renderer.line(
            //     Line::new(start, end).into_styled(style.line_style(bar_width)),
            // )?;
        })
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse {
        let _ = ctx;
        ctx.ignore()
    }
}
