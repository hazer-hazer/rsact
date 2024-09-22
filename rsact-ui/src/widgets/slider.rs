use core::marker::PhantomData;

use embedded_graphics::{
    prelude::{Point, Primitive},
    primitives::{
        Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle,
        RoundedRectangle, Styled,
    },
};
use rsact_core::memo_chain::IntoMemoChain;

use crate::widget::prelude::*;

pub trait SliderEvent {
    fn as_slider_move(&self, axis: Axis) -> Option<i32>;
}

// TODO: Sizes depended on viewport
#[derive(Clone, Copy, PartialEq)]
pub struct SliderStyle<C: Color> {
    track_width: u32,
    track_color: Option<C>,
    thumb_border_width: u32,
    thumb_border: BorderStyle<C>,
    thumb_color: Option<C>,
    thumb_size: u32,
}

impl<C: Color> SliderStyle<C> {
    pub fn base() -> Self {
        Self {
            track_width: 2,
            track_color: Some(C::default_foreground()),
            thumb_border_width: 100,
            thumb_border: BorderStyle::base()
                .radius(BorderRadius::new_equal(Radius::circle()))
                .color(C::default_foreground()),
            thumb_color: Some(C::default_background()),
            thumb_size: 20,
        }
    }

    fn track_line_style(&self) -> PrimitiveStyle<C> {
        let style = PrimitiveStyleBuilder::new();

        let style = if let Some(track_color) = self.track_color {
            style.stroke_color(track_color)
        } else {
            style
        };

        style.stroke_width(self.track_width).build()
    }

    fn thumb(
        &self,
        rect: Rectangle,
    ) -> Styled<RoundedRectangle, PrimitiveStyle<C>> {
        let style = PrimitiveStyleBuilder::new()
            .stroke_width(self.thumb_border_width)
            .stroke_alignment(
                embedded_graphics::primitives::StrokeAlignment::Inside,
            );

        let style = if let Some(thumb_color) = self.thumb_color {
            style.fill_color(thumb_color)
        } else {
            style
        };

        let style = if let Some(border_color) = self.thumb_border.color {
            style.stroke_color(border_color)
        } else {
            style
        };

        RoundedRectangle::new(
            rect.resized(
                embedded_graphics::prelude::Size::new_equal(self.thumb_size),
                embedded_graphics::geometry::AnchorPoint::Center,
            ),
            self.thumb_border.radius.into_corner_radii(rect.size.into()),
        )
        .into_styled(style.build())
    }
}

#[derive(Clone, Copy)]
pub struct SliderState {
    pressed: bool,
    active: bool,
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
                box_model: BoxModel::zero().padding(Padding::new_equal(5)),
                content_size: Limits::zero().into_memo(),
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
{
    fn children_ids(&self) -> Memo<Vec<ElId>> {
        let id = self.id;
        use_memo(move |_| vec![id])
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        let _ = ctx;
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

        let start = ctx.layout.area.top_left
            + Dir::AXIS.canon::<Point>(
                0,
                ctx.layout.area.size.cross(Dir::AXIS) as i32 / 2,
            );

        let end = start
            + Dir::AXIS
                .canon::<Point>(ctx.layout.area.size.main(Dir::AXIS) as i32, 0);

        ctx.renderer.line(
            Line::new(start, end).into_styled(style.track_line_style()),
        )?;

        ctx.renderer.rect(style.thumb(Rectangle::new(
            ctx.layout.area.top_left,
            Into::<Size>::into(ctx.layout.area.size).min_square().into(),
        )))?;

        Ok(())
    }

    fn on_event(
        &mut self,
        ctx: &mut EventCtx<'_, W>,
    ) -> EventResponse<<W as WidgetCtx>::Event> {
        let current_state = self.state.get();
        if current_state.active && ctx.is_focused(self.id) {
            if let Some(offset) = ctx.event.as_slider_move(Dir::AXIS) {
                let current = self.value.get();
                let new =
                    (current as i32 + offset).clamp(0, u8::MAX as i32) as u8;

                if current != new {
                    self.value.set(new);
                }

                return Capture::Captured.into();
            }
        }

        ctx.handle_focusable(self.id, |pressed| {
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

                Capture::Captured.into()
            } else {
                Propagate::Ignored.into()
            }
        })
    }
}
