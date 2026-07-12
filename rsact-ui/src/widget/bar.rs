use crate::{style::WidgetStyleFn, value::RangeValue, widget::prelude::*};
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

// WS13.4 (Task 5.5): `Dir: Direction` looked like the Space/Flex
// compile-time-only tag, but `render` reads `Dir::AXIS` too (bar length +
// resize direction), not just `new()`'s size computation — so per the 7.2
// slice (Dir type param -> runtime Axis, flex/space precedent) it becomes a
// runtime `axis: Axis` FIELD carried on both structs, not a ctor-only
// argument that gets dropped like `Space`'s did.
//
// `V: RangeValue` is deliberately left generic here, NOT de-genericized to a
// "canonical numeric" type: today's only live `RangeValue` impl (`RangeU8<MIN,
// MAX, STEP>`, see `value.rs`) is itself a const-generic family with distinct
// call-site instantiations, and the other candidate impls (plain ints, f32)
// are commented-out/TODO, not live. Picking one concrete type here would be
// an API-design guess, unlike `Dir` (exactly two concrete impls mapping
// bijectively onto `Axis`). Deferred to 5.8 (`slider.rs`), where the fleet
// checklist row explicitly identifies `V` as "really" living, and where the
// choice can be made once, holistically, against the fuller
// `RangeInclusive`/thumb-position math rather than piecemeal here.
#[derive(Builder)]
#[builds(Bar<W, V>)]
pub struct BarBuilder<W: WidgetCtx, V: RangeValue> {
    #[widget]
    value: MaybeReactive<V>,
    #[widget]
    layout: Layout,
    #[widget]
    style: WidgetStyleFn<BarStyle<W::Color>>,
    #[widget]
    axis: Axis,
}

pub struct Bar<W: WidgetCtx, V: RangeValue> {
    value: MaybeReactive<V>,
    layout: Layout,
    style: WidgetStyleFn<BarStyle<W::Color>>,
    axis: Axis,
}

impl<W: WidgetCtx, V: RangeValue + 'static> Bar<W, V> {
    pub fn vertical(value: impl IntoMaybeReactive<V>) -> BarBuilder<W, V> {
        Self::new(Axis::Y, value)
    }

    pub fn horizontal(value: impl IntoMaybeReactive<V>) -> BarBuilder<W, V> {
        Self::new(Axis::X, value)
    }

    pub fn new(
        axis: Axis,
        value: impl IntoMaybeReactive<V>,
    ) -> BarBuilder<W, V> {
        BarBuilder {
            value: value.maybe_reactive(),
            layout: Layout::edge(axis.canon(Length::fill(), Length::Fixed(10))),
            style: None,
            axis,
        }
    }
}

impl<W: WidgetCtx + 'static, V: RangeValue + 'static> Widget<W> for Bar<W, V> {
    // NOTE: no `flags`/`debug_name` override on the retained widget — both are
    // read exactly once, pre-build, from `Build` (seeding `ElState` at
    // `state.rs:72`); post-build all consumption is via `ElState`, so an
    // override here would be dead duplication of `BarBuilder`'s derived
    // `Build::debug_name` ("Bar" from `#[builds(Bar<W, V>)]`). `Bar` never
    // overrode `flags` either, so no `#[flags(...)]` attr is needed on
    // `BarBuilder`.
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

            let full_len = ctx.layout.inner.size.main(self.axis);
            let value_len = self.value.get().point(full_len);

            let bar_area = ctx.layout.inner.resized_axis(
                self.axis,
                value_len,
                Anchor::Start,
            );

            ctx.renderer.rounded_rect(
                bar_area,
                style
                    .container
                    .border
                    .radius
                    .into_corner_radii(bar_area.size),
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
