use crate::{
    declare_widget_style,
    widget::{Meta, MetaTree, prelude::*},
};
use core::{marker::PhantomData, ops::RangeInclusive};
use rsact_reactive::prelude::*;

#[derive(Clone, Copy, Default, PartialEq)]
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
        track_width: u32,
        track_color: color,
        thumb_border_width: u32,
        thumb: container {
            thumb_color: background_color,
            thumb_border_color: border_color,
            thumb_border_radius: border_radius,
        },
        // TODO: Thumb size can be larger than Slider bounding box, I think it is better to use slider size for thumb size and if user wants padding then Container can be used, this will save us from thumb leaking outside of slider
        thumb_size: u32,
        thumb_shape: SliderThumbShape
    }
}

impl<C: Color> SliderStyle<C> {
    pub fn base() -> Self {
        Self {
            track_width: 2,
            track_color: ColorStyle::DefaultForeground,
            thumb_border_width: 0,
            thumb: BlockStyle::base()
                .background_color(C::default_foreground())
                .border(BorderStyle::base().radius(0.25)),
            thumb_size: 11,
            thumb_shape: SliderThumbShape::default(),
        }
    }

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
    pub pressed: bool,
    pub active: bool,
}

impl SliderState {
    pub fn none() -> Self {
        Self { pressed: false, active: false }
    }
}

// TODO: Floating label?
// TODO: Exponential
pub struct Slider<W: WidgetCtx, Dir: Direction> {
    value: Signal<f32>,
    range: MaybeReactive<RangeInclusive<f32>>,
    step: MaybeReactive<f32>,
    state: Signal<SliderState>,
    layout: Layout,
    style: Option<Box<dyn Fn(SliderStyle<W::Color>) -> SliderStyle<W::Color>>>,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx, Dir: Direction> Slider<W, Dir> {
    pub fn new(
        value: impl IntoSignal<f32>,
        range: impl IntoMaybeReactive<RangeInclusive<f32>>,
    ) -> Self {
        let range = range.maybe_reactive();
        let step = range.map(|range| Self::step_from_range(range));

        Self {
            state: SliderState::none().signal(),
            value: value.signal(),
            range,
            step,
            layout: Layout::edge(
                Dir::AXIS.canon(Length::fill(), Length::Fixed(13)),
            ),
            style: None,
            dir: PhantomData,
        }
    }

    // TODO: Custom speed
    fn step_from_range(range: &RangeInclusive<f32>) -> f32 {
        if range.is_empty() {
            0.0
        } else {
            (range.end() - range.start()) * 0.01
        }
    }

    pub fn step(mut self, step: impl IntoMaybeReactive<f32>) -> Self {
        self.step = step.maybe_reactive();
        self
    }
}

impl<W: WidgetCtx> Slider<W, ColDir> {
    pub fn vertical(
        value: impl IntoSignal<f32>,
        range: impl IntoMaybeReactive<RangeInclusive<f32>>,
    ) -> Self {
        Self::new(value, range)
    }
}

impl<W: WidgetCtx> Slider<W, RowDir> {
    pub fn horizontal(
        value: impl IntoSignal<f32>,
        range: impl IntoMaybeReactive<RangeInclusive<f32>>,
    ) -> Self {
        Self::new(value, range)
    }
}

impl<W: WidgetCtx, Dir: Direction> Widget<W> for Slider<W, Dir> {
    fn meta(&self, id: ElId) -> MetaTree {
        MetaTree::childless(Meta::focusable(id))
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, ctx: &mut RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self("Slider", |ctx| {
            ctx.render_focus_outline(ctx.id)?;

            let style = ctx.get_style(|t| t.slider, self.style.as_deref());

            let track_len = ctx
                .layout
                .inner
                .size
                .main(Dir::AXIS)
                .saturating_sub(style.thumb_size + 2);

            let half_thumb_size = style.thumb_size as i32 / 2;

            let start = ctx.layout.inner.top_left
                + Dir::AXIS.canon::<Point>(
                    half_thumb_size + 1,
                    ctx.layout.inner.size.cross(Dir::AXIS) as i32 / 2,
                );

            let end = start + Dir::AXIS.canon::<Point>(track_len as i32, 0);

            ctx.renderer().line(start, end, &style.track_draw_style())?;

            let range_len =
                self.range.with(|range| range.end() - range.start());

            let thumb_pos = start
                + Dir::AXIS.canon::<Point>(
                    ((self.value.get() / range_len) * track_len as f32) as i32
                        - half_thumb_size,
                    -half_thumb_size,
                );

            let thumb_draw_style = DrawStyle {
                fill: style.thumb.background_color.get(),
                stroke: style.thumb.border.color.get(),
                stroke_width: style.thumb_border_width,
                stroke_alignment: StrokeAlignment::Inside,
            };

            match style.thumb_shape {
                SliderThumbShape::Dash => ctx.renderer().line(
                    thumb_pos,
                    thumb_pos
                        + Dir::AXIS.canon::<Point>(0, style.thumb_size as i32),
                    &thumb_draw_style,
                ),
                SliderThumbShape::RoundedSquare => {
                    let rect =
                        Rect::new(thumb_pos, Size::new_equal(style.thumb_size));
                    ctx.renderer().rounded_rect(
                        rect,
                        style.thumb.border.radius.into_corner_radii(rect.size),
                        &thumb_draw_style,
                    )
                },
                SliderThumbShape::Circle => ctx.renderer().circle(
                    thumb_pos,
                    style.thumb_size,
                    &thumb_draw_style,
                ),
                SliderThumbShape::Square => ctx.renderer().rect(
                    Rect::new(thumb_pos, Size::new_equal(style.thumb_size)),
                    &thumb_draw_style,
                ),
            }
        })
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        let current_state = self.state.get();

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

        ctx.handle_focusable(|ctx, pressed| {
            if current_state.pressed != pressed {
                let toggle_active = if !current_state.pressed && pressed {
                    true
                } else {
                    false
                };

                self.state.update(|state| {
                    state.pressed = pressed;
                    if toggle_active {
                        state.active = !state.active;
                    }
                });

                ctx.capture()
            } else {
                ctx.ignore()
            }
        })
    }
}
