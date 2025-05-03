use crate::{
    declare_widget_style,
    render::{
        Renderable,
        primitives::{circle::Circle, line::Line, rounded_rect::RoundedRect},
    },
    style::{ColorStyle, WidgetStylist},
    widget::{Meta, MetaTree, prelude::*},
};
use core::{marker::PhantomData, ops::RangeInclusive};
use embedded_graphics::{
    prelude::{Point, Primitive},
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
};
use rsact_reactive::{maybe::IntoMaybeReactive, memo_chain::IntoMemoChain};

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

    fn track_line_style(&self) -> PrimitiveStyle<C> {
        let style = PrimitiveStyleBuilder::new();

        let style = if let Some(track_color) = self.track_color.get() {
            style.stroke_color(track_color)
        } else {
            style
        };

        style.stroke_width(self.track_width).build()
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
    id: ElId,
    value: Signal<f32>,
    range: MaybeReactive<RangeInclusive<f32>>,
    step: Memo<f32>,
    state: Signal<SliderState>,
    layout: Layout,
    style: MemoChain<SliderStyle<W::Color>>,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx, Dir: Direction> Slider<W, Dir> {
    pub fn new(
        value: impl IntoSignal<f32>,
        range: impl IntoMaybeReactive<RangeInclusive<f32>>,
    ) -> Self {
        let range = range.maybe_reactive();
        let step = range.map_reactive(|range| Self::step_from_range(range));

        Self {
            id: ElId::unique(),
            state: SliderState::none().signal(),
            value: value.signal(),
            range,
            step,
            layout: Layout::edge(
                Dir::AXIS.canon(Length::fill(), Length::Fixed(13)),
            ),
            style: SliderStyle::base().memo_chain(),
            dir: PhantomData,
        }
    }

    fn step_from_range(range: &RangeInclusive<f32>) -> f32 {
        if range.is_empty() {
            0.0
        } else {
            (range.end() - range.start()) * 0.01
        }
    }

    pub fn step(mut self, step: impl IntoMemo<f32>) -> Self {
        self.step = step.memo();
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

impl<W: WidgetCtx, Dir: Direction> Widget<W> for Slider<W, Dir>
where
    W::Styler: WidgetStylist<SliderStyle<W::Color>>,
{
    fn meta(&self) -> MetaTree {
        let id = self.id;

        MetaTree::childless(create_memo(move || Meta::focusable(id)))
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);
    }

    fn layout(&self) -> &Layout {
        &self.layout
    }

    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }

    fn render(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        ctx.render_focus_outline(self.id)?;

        let style = self.style.get();

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

        Line::new(start, end)
            .into_styled(style.track_line_style())
            .render(ctx.renderer)?;

        let range_len = self.range.with(|range| range.end() - range.start());

        let thumb_pos = start
            + Dir::AXIS.canon::<Point>(
                ((self.value.get() / range_len) * track_len as f32) as i32
                    - half_thumb_size,
                -half_thumb_size,
            );

        let thumb_style = PrimitiveStyleBuilder::new()
            .stroke_width(style.thumb_border_width)
            .stroke_alignment(
                embedded_graphics::primitives::StrokeAlignment::Inside,
            );

        let thumb_style =
            if let Some(thumb_color) = style.thumb.background_color.get() {
                thumb_style.fill_color(thumb_color)
            } else {
                thumb_style
            };

        let thumb_style =
            if let Some(border_color) = style.thumb.border.color.get() {
                thumb_style.stroke_color(border_color)
            } else {
                thumb_style
            };

        match style.thumb_shape {
            SliderThumbShape::Dash => Line::new(
                thumb_pos,
                thumb_pos
                    + Dir::AXIS.canon::<Point>(0, style.thumb_size as i32),
            )
            .into_styled(thumb_style.build())
            .render(ctx.renderer),
            SliderThumbShape::RoundedSquare => RoundedRect::new(
                Rectangle::new(
                    thumb_pos,
                    embedded_graphics::prelude::Size::new_equal(
                        style.thumb_size,
                    ),
                ),
                style.thumb.border.radius,
            )
            .into_styled(thumb_style.build())
            .render(ctx.renderer),
            SliderThumbShape::Circle => {
                Circle::new(thumb_pos, style.thumb_size)
                    .into_styled(thumb_style.build())
                    .render(ctx.renderer)
            },
            SliderThumbShape::Square => Rectangle::new(
                thumb_pos,
                embedded_graphics::prelude::Size::new_equal(style.thumb_size),
            )
            .into_styled(thumb_style.build())
            .render(ctx.renderer),
        }
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse {
        let current_state = self.state.get();

        if current_state.active && ctx.is_focused(self.id) {
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

        ctx.handle_focusable(self.id, |ctx, pressed| {
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
