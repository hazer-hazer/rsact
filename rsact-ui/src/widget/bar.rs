use crate::{style::WidgetStyleFn, value::RangeValue, widget::prelude::*};
use core::marker::PhantomData;
use rsact_reactive::prelude::*;

// TODO: Padding for inner bar

declare_widget_style! {
    BarStyle () {
        container: container,
        color: color,
    }
}

impl<C: Color> BarStyle<C> {
    fn bar_draw_style(&self) -> DrawStyle<C> {
        DrawStyle {
            fill: self.color.get(),
            stroke: None,
            stroke_width: 0,
            stroke_alignment: StrokeAlignment::Inside,
        }
    }
}

pub struct Bar<W: WidgetCtx, V: RangeValue, Dir: Direction> {
    value: MaybeReactive<V>,
    layout: Layout,
    style: WidgetStyleFn<BarStyle<W::Color>>,
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
            style: None,
            dir: PhantomData,
        }
    }
}

impl<W: WidgetCtx, V: RangeValue + 'static, Dir: Direction + 'static> Widget<W>
    for Bar<W, V, Dir>
{
    fn debug_name(&self) -> &'static str {
        "Bar"
    }

    fn build(&mut self, ctx: build::BuildCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self(|ctx| {
            let style = ctx.get_style(self.style.as_deref());

            // let start = ctx.layout.area.anchor_point(
            //     Dir::AXIS
            //         .canon::<AxisAnchorPoint>(Anchor::Start, Anchor::Center)
            //         .into(),
            // );

            // let end = start + Dir::AXIS.canon::<Point>(value_len as i32, 0);

            // let bar_width = ctx.layout.area.size.cross(Dir::AXIS);

            let block_model = self.layout.with(|layout| layout.block_model());
            Block::from_layout_style(
                ctx.layout.outer,
                block_model,
                style.container,
            )
            .render(ctx.renderer)?;

            let full_len = ctx.layout.inner.size.main(Dir::AXIS);
            let value_len = self.value.get().point(full_len);

            let bar_area = ctx.layout.inner.resized_axis(
                Dir::AXIS,
                value_len,
                Anchor::Start,
            );

            ctx.renderer.rounded_rect(
                bar_area,
                style.container.border.radius.into_corner_radii(bar_area.size),
                &style.bar_draw_style(),
            )?;

            // ctx.renderer.line(
            //     Line::new(start,
            // end).into_styled(style.line_style(bar_width)), )?;

            Ok(())
        })
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        let _ = ctx;
        ctx.ignore()
    }
}
