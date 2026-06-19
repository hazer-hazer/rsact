use super::prelude::*;
use crate::{
    layout::length::LengthSize, render::geometry::*, value::RangeValue,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnobState {
    pub pressed: bool,
    pub active: bool,
}

impl KnobState {
    pub fn none() -> Self {
        Self { pressed: false, active: false }
    }
}

declare_widget_style! {
    KnobStyle (KnobState) {
        container: container,
        color: color,
        // thickness: u32,
        angle_start: Angle = Angle::from_degrees(0.0),
        angle: Angle = Angle::from_degrees(360.0),
    }
}

impl<C: Color> KnobStyle<C> {
    fn sector_draw_style(&self) -> DrawStyle<C> {
        DrawStyle {
            fill: self.color.get(),
            stroke: None,
            stroke_width: 0,
            stroke_alignment: StrokeAlignment::Outside,
        }
    }
}

pub struct Knob<W: WidgetCtx, V: RangeValue> {
    layout: Layout,
    value: Signal<V>,
    state: Signal<KnobState>,
    style: WidgetStyleFn<KnobStyle<W::Color>>,
}

impl<W: WidgetCtx, V: RangeValue + 'static> Knob<W, V> {
    pub fn new(value: Signal<V>) -> Self {
        Self {
            layout: Layout::edge(LengthSize::new_equal(Length::Fixed(25))),
            value,
            state: KnobState::none().signal(),
            style: None,
        }
    }

    // pub fn size(self, size: impl AsMemo<u32>) -> Self {
    //     self.layout.setter(size.as_memo(), |size, layout| {
    //         layout.size = Size::new_equal(Length::Fixed(*size));
    //     });
    //     self
    // }

    pub fn size(mut self, size: impl Into<u32>) -> Self {
        self.layout.update_untracked(|layout| {
            layout.size = LengthSize::new_equal(Length::Fixed(size.into()));
        });
        self
    }
}

impl<W: WidgetCtx, V: RangeValue + 'static> Widget<W> for Knob<W, V> {
    fn debug_name(&self) -> &'static str {
        "Knob"
    }

    fn build(&mut self, ctx: BuildCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self(|mut ctx| {
            let style = ctx.get_style(self.style.as_deref());

            let value_real = self.value.get().real_point();
            let range_degrees = style.angle;
            let value_angle = Angle::from_degrees(
                (value_real * range_degrees.to_degrees()).min(360.0),
            );

            let top_left = ctx.layout.inner.top_left;
            let diameter = ctx.layout.inner.size.max_square().width;
            ctx.renderer.sector(
                top_left,
                diameter,
                style.angle_start,
                value_angle,
                &style.sector_draw_style(),
            )?;

            // TODO: Round focus outline
            ctx.render_focus_outline(ctx.id)
        })
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        let current_state = self.state.get();

        if current_state.active && ctx.is_focused() {
            if let Some(offset) = ctx.event.interpret_as_rotation() {
                let current = self.value.get();

                let new = current.offset(offset);

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
