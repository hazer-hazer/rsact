use crate::{declare_widget_style, widget::prelude::*};
use core::ops::RangeInclusive;
use rsact_reactive::prelude::*;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum SliderThumbShape {
    Dash,
    Square,
    RoundedSquare,
    #[default]
    Circle,
}

// TODO: Support any slider value or use f32 with user conversions?
// pub trait SliderValue {
// }

// TODO: Sizes depended on viewport
declare_widget_style! {
    SliderStyle (SliderState) {
        track_width: u32 = 10,
        track_color: color,
        thumb_border_width: u32 = 10,
        thumb: container {
            thumb_color: background_color,
            thumb_border_color: border_color,
            thumb_border_radius: border_radius,
        },
        // TODO: Thumb size can be larger than Slider bounding box, I think it is better to use slider size for thumb size and if user wants padding then Container can be used, this will save us from thumb leaking outside of slider
        thumb_size: u32 = 10,
        thumb_shape: SliderThumbShape = SliderThumbShape::Circle,
    }
}

impl<C: Color> SliderStyle<C> {
    fn track_draw_style(&self) -> DrawStyle<C> {
        DrawStyle {
            fill: None,
            stroke: self.track_color.get(),
            stroke_width: self.track_width,
            stroke_alignment: StrokeAlignment::Center,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct SliderState {
    // Press state is global now (see `PageState`/`PointerState`); only the
    // widget-specific `active` (value-adjust) mode is stored locally.
    pub active: bool,
}

impl SliderState {
    pub fn none() -> Self {
        Self { active: false }
    }
}

// WS13.4 (Task 5.8): unlike Bar's still-open `V: RangeValue` question, Slider
// never had a `V` generic to begin with — its value/range/step are already
// concrete on `f32` (see the commented-out `SliderValue`/generic-value TODO
// above), so there is no "canonical numeric boundary" decision to make here;
// the fleet checklist row's "`V` + `Dir` de-generic" applies only to `Dir`
// for this widget. `Dir: Direction` DID read like the Space/Flex
// compile-time-only tag (only `new()` picked an `Axis` from it), but `render`
// reads `Dir::AXIS` repeatedly too (track length/orientation, thumb
// position) — so per the 7.2 slice (Bar precedent: `Dir` type param ->
// runtime `Axis` FIELD when render/on_event read it, not just a ctor-only
// argument) it becomes a runtime `axis: Axis` field carried on both structs.
//
// Every field here is read by `render` (`value`/`range`/`axis`/`style`) or
// by `on_event` (`value`/`range`/`step`/`state`), so — like `Bar`/
// `Checkbox` — there is no build-only field to drop; `SliderBuilder` moves
// all seven fields into the retained `Slider` unchanged (a `size_of` `<`
// assertion would be false, not true). `value: Signal<f32>` and `state:
// SliderState` are the widget's job (WS4.5: the live value + local
// value-adjust mode ARE what a Slider is), so both stay retained fields.
#[derive(Builder)]
#[builds(Slider<W>)]
#[flags(focusable)]
pub struct SliderBuilder<W: WidgetCtx> {
    #[widget]
    value: Signal<f32>,
    #[widget]
    range: MaybeReactive<RangeInclusive<f32>>,
    #[widget]
    step: MaybeReactive<f32>,
    // WS4.5: plain field, not a Signal — `state` is read/written only in
    // `on_event(&mut self)` (never in render/layout), so it needs no runtime
    // node. Global press/focus state lives in PageState; this is only the
    // widget-local value-adjust mode.
    #[widget]
    state: SliderState,
    #[widget]
    layout: LayoutBuilder<W>,
    #[widget]
    style: WidgetStyleFn<SliderStyle<W::Color>>,
    #[widget]
    axis: Axis,
}

// TODO: Floating label?
// TODO: Exponential
pub struct Slider<W: WidgetCtx> {
    value: Signal<f32>,
    range: MaybeReactive<RangeInclusive<f32>>,
    step: MaybeReactive<f32>,
    state: SliderState,
    layout: LayoutData,
    style: WidgetStyleFn<SliderStyle<W::Color>>,
    axis: Axis,
}

impl<W: WidgetCtx> Slider<W> {
    pub fn new(
        axis: Axis,
        value: impl IntoSignal<f32>,
        range: impl IntoMaybeReactive<RangeInclusive<f32>>,
    ) -> SliderBuilder<W> {
        let range = range.maybe_reactive();
        let step = range.map(|range| Self::step_from_range(range));

        SliderBuilder {
            state: SliderState::none(),
            value: value.signal(),
            range,
            step,
            layout: LayoutBuilder::edge(
                axis.canon(Length::fill(), Length::Fixed(13)),
            ),
            style: None,
            axis,
        }
    }

    pub fn vertical(
        value: impl IntoSignal<f32>,
        range: impl IntoMaybeReactive<RangeInclusive<f32>>,
    ) -> SliderBuilder<W> {
        Self::new(Axis::Y, value, range)
    }

    pub fn horizontal(
        value: impl IntoSignal<f32>,
        range: impl IntoMaybeReactive<RangeInclusive<f32>>,
    ) -> SliderBuilder<W> {
        Self::new(Axis::X, value, range)
    }

    // TODO: Custom speed
    fn step_from_range(range: &RangeInclusive<f32>) -> f32 {
        if range.is_empty() {
            0.0
        } else {
            (range.end() - range.start()) * 0.01
        }
    }
}

impl<W: WidgetCtx> SliderBuilder<W> {
    pub fn step(mut self, step: impl IntoMaybeReactive<f32>) -> Self {
        self.step = step.maybe_reactive();
        self
    }
}

impl<W: WidgetCtx> Widget<W> for Slider<W> {
    // NOTE: no `flags`/`debug_name` override on the retained widget — both are
    // read exactly once, pre-build, from `Build` (seeding `ElState` at
    // `state.rs:72`); post-build all consumption is via `ElState`, so an
    // override here would be dead duplication of `SliderBuilder`'s derived
    // `Build::flags`/`Build::debug_name` ("Slider" from `#[builds(Slider<W>)]`).
    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self(|mut ctx| {
            ctx.render_focus_outline(ctx.id)?;

            let style = ctx.get_style(self.style.as_deref());

            let track_len = ctx
                .layout
                .inner
                .size
                .main(self.axis)
                .saturating_sub(style.thumb_size + 2);

            let half_thumb_size = style.thumb_size as i32 / 2;

            let start = ctx.layout.inner.top_left
                + self.axis.canon::<Point>(
                    half_thumb_size + 1,
                    ctx.layout.inner.size.cross(self.axis) as i32 / 2,
                );

            let end = start + self.axis.canon::<Point>(track_len as i32, 0);

            ctx.renderer.line(start, end, &style.track_draw_style())?;

            let (range_start, range_len) = self
                .range
                .with(|range| (*range.start(), range.end() - range.start()));

            // Normalize the value into 0..=1 *relative to the range start* —
            // not value/len, which mispositions any non-zero-based range (and
            // guard a degenerate zero-width range against NaN/inf).
            let frac = if range_len != 0.0 {
                ((self.value.get() - range_start) / range_len).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let thumb_pos = start
                + self.axis.canon::<Point>(
                    (frac * track_len as f32) as i32 - half_thumb_size,
                    -half_thumb_size,
                );

            let thumb_draw_style = DrawStyle {
                fill: style.thumb.background_color.get(),
                stroke: style.thumb.border.color.get(),
                stroke_width: style.thumb_border_width,
                stroke_alignment: StrokeAlignment::Inside,
            };

            match style.thumb_shape {
                SliderThumbShape::Dash => ctx.renderer.line(
                    thumb_pos,
                    thumb_pos
                        + self.axis.canon::<Point>(0, style.thumb_size as i32),
                    &thumb_draw_style,
                ),
                SliderThumbShape::RoundedSquare => {
                    let rect =
                        Rect::new(thumb_pos, Size::new_equal(style.thumb_size));
                    ctx.renderer.rounded_rect(
                        rect,
                        style.thumb.border.radius.into_corner_radii(rect.size),
                        &thumb_draw_style,
                    )
                },
                SliderThumbShape::Circle => ctx.renderer.circle(
                    thumb_pos,
                    style.thumb_size,
                    &thumb_draw_style,
                ),
                SliderThumbShape::Square => ctx.renderer.rect(
                    Rect::new(thumb_pos, Size::new_equal(style.thumb_size)),
                    &thumb_draw_style,
                ),
            }
        })
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        let current_state = self.state;

        if current_state.active && ctx.is_focused() {
            // TODO: Right slider event interpretation
            if let Some(offset) = ctx.event.interpret_as_rotation() {
                let current = self.value.get();
                let range = self.range.get_cloned();
                let new = (current + offset as f32 * self.step.get())
                    .clamp(*range.start(), *range.end());

                if current != new {
                    self.value.set(new);
                }

                return ctx.capture();
            }
        }

        ctx.handle()?; // focus press claim (encoder), automatic
        ctx.handle_click(|ctx| {
            self.state.active = !self.state.active;
            ctx.capture()
        })
    }
}
