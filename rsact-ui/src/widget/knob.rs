use super::prelude::*;
use crate::render::Renderable;
use crate::render::primitives::sector::Sector;
use crate::value::RangeValue;
use embedded_graphics::prelude::{Angle, Primitive};
use embedded_graphics::primitives::{PrimitiveStyle, PrimitiveStyleBuilder};
use layout::size::SizeExt;

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
        angle_start: Angle,
        angle: Angle,
    }
}

impl<C: Color> KnobStyle<C> {
    pub fn base() -> Self {
        Self {
            container: BlockStyle::base(),
            color: ColorStyle::DefaultForeground,
            // thickness: 5,
            angle_start: Angle::from_degrees(0.0),
            angle: Angle::from_degrees(360.0),
        }
    }

    fn sector_style(&self) -> PrimitiveStyle<C> {
        let base = PrimitiveStyleBuilder::new()
            // .stroke_width(self.thickness)
            .stroke_alignment(
                embedded_graphics::primitives::StrokeAlignment::Outside,
            );

        let base = self
            .color
            .get()
            .map(|color| base.fill_color(color))
            .unwrap_or(base);

        base.build()
    }
}

pub struct Knob<W: WidgetCtx, V: RangeValue> {
    id: ElId,
    layout: Layout,
    value: Signal<V>,
    state: Signal<KnobState>,
    style: MemoChain<KnobStyle<W::Color>>,
}

impl<W: WidgetCtx, V: RangeValue + 'static> Knob<W, V> {
    pub fn new(value: Signal<V>) -> Self {
        Self {
            id: ElId::unique(),
            layout: Layout::edge(Size::new_equal(Length::Fixed(25))),
            value,
            state: KnobState::none().signal(),
            style: KnobStyle::base().memo_chain(),
        }
    }

    // pub fn size(self, size: impl AsMemo<u32>) -> Self {
    //     self.layout.setter(size.as_memo(), |size, layout| {
    //         layout.size = Size::new_equal(Length::Fixed(*size));
    //     });
    //     self
    // }

    pub fn size(mut self, size: impl Into<u32>) -> Self {
        self.layout
            .size
            .set_untracked(Size::new_equal(Length::Fixed(size.into())));
        self
    }
}

impl<W: WidgetCtx, V: RangeValue + 'static> Widget<W> for Knob<W, V>
where
    W::Styler: WidgetStylist<KnobStyle<W::Color>>,
{
    fn meta(&self) -> MetaTree {
        let id = self.id;
        MetaTree::childless(create_memo(move || Meta::focusable(id)))
    }

    fn on_mount(&mut self, ctx: super::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);
    }

    fn layout(&self) -> &Layout {
        &self.layout
    }

    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }

    fn render(&self, ctx: RenderCtx<W>) -> Computed<()> {
        let style = self.style.get();

        let value_real = self.value.get().real_point();
        let range_degrees = style.angle;
        let value_angle = Angle::from_degrees(
            (value_real * range_degrees.to_degrees()).min(360.0),
        );

        Sector::new(
            ctx.layout.inner.top_left,
            ctx.layout.inner.size.max_square().width,
            style.angle_start,
            value_angle,
        )
        .into_styled(style.sector_style())
        .render(ctx.renderer)?;

        // TODO: Round focus outline
        ctx.render_focus_outline(self.id)
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse {
        let current_state = self.state.get();

        if current_state.active && ctx.is_focused(self.id) {
            if let Some(offset) = ctx.event.interpret_as_rotation() {
                let current = self.value.get();

                let new = current.offset(offset);

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
