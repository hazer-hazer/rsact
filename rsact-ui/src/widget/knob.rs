use super::prelude::*;
use crate::{
    layout::length::LengthSize, render::geometry::*, value::RangeValue,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnobState {
    pub active: bool,
}

impl KnobState {
    pub fn none() -> Self {
        Self { active: false }
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

// WS13.4 (Task 5.9): "Like slider" per the fleet checklist, but the `V`
// question resolves the other way here — `Knob<W, V: RangeValue>` DOES
// genuinely carry a `V` generic (unlike `Slider`, which never had one; see
// slider.rs's WS13.4 comment). Applying Bar's decision rule (bar.rs:24-40)
// to the same evidence Bar found: today's only live `RangeValue` impl is the
// const-generic `RangeU8<MIN, MAX, STEP>` family (itself a family of
// distinct instantiations, not one canonical type), and the other candidate
// impls (plain ints, f32) are still commented-out/TODO in value.rs, not
// live. So `V` is deliberately left generic here too, NOT de-genericized to
// a guessed "canonical numeric" type — same call as Bar, deferred to the
// same WS7 remainder. `Knob` has no `Dir`/`Axis` generic at all (it renders
// a full sector, not an oriented track), so there is no Dir-side decision to
// make on this widget.
//
// Every field here is read by `render` (`value`/`style`) or by `on_event`
// (`value`/`state`), so — like `Bar`/`Checkbox`/`Slider` — there is no
// build-only field to drop; `KnobBuilder` moves all four fields into the
// retained `Knob` unchanged (a `size_of` `<` assertion would be false, not
// true). `value: Signal<V>` and `state: KnobState` are the widget's job
// (WS4.5: the live value + local value-adjust mode ARE what a Knob is), so
// both stay retained fields.
#[derive(Builder)]
#[builds(Knob<W, V>)]
#[flags(focusable)]
pub struct KnobBuilder<W: WidgetCtx, V: RangeValue> {
    #[widget]
    layout: LayoutBuilder<W>,
    #[widget]
    value: Signal<V>,
    // WS4.5: plain field, not a Signal — read/written only in
    // `on_event(&mut self)`, never in render/layout, so it needs no node.
    #[widget]
    state: KnobState,
    #[widget]
    style: WidgetStyleFn<KnobStyle<W::Color>>,
}

pub struct Knob<W: WidgetCtx, V: RangeValue> {
    layout: LayoutData,
    value: Signal<V>,
    state: KnobState,
    style: WidgetStyleFn<KnobStyle<W::Color>>,
}

impl<W: WidgetCtx, V: RangeValue + 'static> Knob<W, V> {
    pub fn new(value: Signal<V>) -> KnobBuilder<W, V> {
        KnobBuilder {
            layout: LayoutBuilder::edge(LengthSize::new_equal(Length::Fixed(
                25,
            ))),
            value,
            state: KnobState::none(),
            style: None,
        }
    }
}

impl<W: WidgetCtx, V: RangeValue + 'static> KnobBuilder<W, V> {
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
    // NOTE: no `flags`/`debug_name` override on the retained widget — both are
    // read exactly once, pre-build, from `Build` (seeding `ElState` at
    // `state.rs:72`); post-build all consumption is via `ElState`, so an
    // override here would be dead duplication of `KnobBuilder`'s derived
    // `Build::flags`/`Build::debug_name` ("Knob" from `#[builds(Knob<W, V>)]`).
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
        let current_state = self.state;

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

        ctx.handle()?; // focus press claim (encoder), automatic
        ctx.handle_click(|ctx| {
            self.state.active = !self.state.active;
            ctx.capture()
        })
    }
}
