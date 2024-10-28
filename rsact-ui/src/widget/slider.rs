use crate::{
    declare_widget_style,
    render::{primitives::line::Line, Renderable},
    style::{ColorStyle, Styler},
    widget::{prelude::*, Meta, MetaTree},
};
use core::marker::PhantomData;
use embedded_graphics::{
    prelude::{Point, Primitive},
    primitives::{
        PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, RoundedRectangle,
        Styled,
    },
};
use rsact_reactive::memo_chain::IntoMemoChain;

pub trait SliderEvent {
    fn as_slider_move(&self, axis: Axis) -> Option<i32>;
}

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
        thumb_size: u32,
    }
}

impl<C: Color> SliderStyle<C> {
    pub fn base() -> Self {
        Self {
            track_width: 2,
            track_color: ColorStyle::DefaultForeground,
            thumb_border_width: 1,
            thumb: BlockStyle::base().border(BorderStyle::base().radius(0.25)),
            thumb_size: 20,
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

    fn thumb_style(
        &self,
        rect: Rectangle,
    ) -> Styled<RoundedRectangle, PrimitiveStyle<C>> {
        let style = PrimitiveStyleBuilder::new()
            .stroke_width(self.thumb_border_width)
            .stroke_alignment(
                embedded_graphics::primitives::StrokeAlignment::Inside,
            );

        let style = if let Some(thumb_color) = self.thumb.background_color.get()
        {
            style.fill_color(thumb_color)
        } else {
            style
        };

        let style = if let Some(border_color) = self.thumb.border.color.get() {
            style.stroke_color(border_color)
        } else {
            style
        };

        RoundedRectangle::new(
            rect.resized(
                embedded_graphics::prelude::Size::new_equal(self.thumb_size),
                embedded_graphics::geometry::AnchorPoint::Center,
            ),
            self.thumb.border.radius.into_corner_radii(rect.size),
        )
        .into_styled(style.build())
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

pub struct Slider<W: WidgetCtx, Dir: Direction> {
    id: ElId,
    value: Signal<u8>,
    state: Signal<SliderState>,
    layout: Signal<Layout>,
    style: MemoChain<SliderStyle<W::Color>>,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx, Dir: Direction> Slider<W, Dir> {
    pub fn new(value: impl IntoSignal<u8>) -> Self {
        Self {
            id: ElId::unique(),
            state: SliderState::none().into_signal(),
            value: value.into_signal(),
            layout: Layout {
                kind: LayoutKind::Edge,
                size: Dir::AXIS.canon(Length::fill(), Length::Fixed(25)),
            }
            .into_signal(),
            style: SliderStyle::base().into_memo_chain(),
            dir: PhantomData,
        }
    }
}

impl<W: WidgetCtx> Slider<W, ColDir> {
    pub fn vertical(value: impl IntoSignal<u8>) -> Self {
        Self::new(value)
    }
}

impl<W: WidgetCtx> Slider<W, RowDir> {
    pub fn horizontal(value: impl IntoSignal<u8>) -> Self {
        Self::new(value)
    }
}

impl<W: WidgetCtx, Dir: Direction> Widget<W> for Slider<W, Dir>
where
    W::Event: SliderEvent,
    W::Styler: Styler<SliderStyle<W::Color>, Class = ()>,
{
    fn meta(&self) -> MetaTree {
        MetaTree::childless(Meta::focusable(self.id))
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        MemoTree::childless(self.layout.into_memo())
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        let style = self.style.get();

        ctx.draw_focus_outline(self.id)?;

        let track_len =
            ctx.layout.inner.size.main(Dir::AXIS) - style.thumb_size - 1;

        let start = ctx.layout.inner.top_left
            + Dir::AXIS.canon::<Point>(
                style.thumb_size as i32 / 2,
                ctx.layout.inner.size.cross(Dir::AXIS) as i32 / 2,
            );

        let end = start + Dir::AXIS.canon::<Point>(track_len as i32, 0);

        Line::new(start, end)
            .into_styled(style.track_line_style())
            .render(ctx.renderer)?;

        let thumb_offset = (self.value.get() as f32 / 256.0) * track_len as f32;

        style
            .thumb_style(Rectangle::new(
                ctx.layout.inner.top_left
                    + Dir::AXIS.canon::<Point>(thumb_offset as i32, 0),
                Into::<Size>::into(ctx.layout.inner.size).min_square().into(),
            ))
            .render(ctx.renderer)?;

        Ok(())
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse<W> {
        let current_state = self.state.get();

        if current_state.active && ctx.is_focused(self.id) {
            if let Some(offset) = ctx.event.as_slider_move(Dir::AXIS) {
                let current = self.value.get();
                let new =
                    (current as i32 + offset).clamp(0, u8::MAX as i32) as u8;

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
