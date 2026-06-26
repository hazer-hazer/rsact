use crate::widget::prelude::*;
use rsact_reactive::prelude::*;

// TODO: Responsive for size
// 3 8 7 12 13 4
const CHECKBOX_ICON_POINTS: &[Point] =
    &[Point::new(3, 8), Point::new(7, 12), Point::new(13, 4)];

#[derive(Clone, Copy)]
pub struct CheckboxState {
    pub pressed: bool,
}

impl CheckboxState {
    pub fn none() -> Self {
        Self { pressed: false }
    }
}

declare_widget_style! {
    CheckboxStyle (CheckboxState) {
        container: container,
        icon_color: color = ColorStyle::DefaultForeground,
    }
}

// TODO: Do we need `on_change` event if we have signal value?

// TODO: Custom icon
pub struct Checkbox<W: WidgetCtx> {
    state: Signal<CheckboxState>,
    layout: Layout,
    value: MaybeSignal<bool>,
    style: WidgetStyleFn<CheckboxStyle<W::Color>>,
}

impl<W: WidgetCtx> Checkbox<W> {
    pub fn new(value: impl Into<MaybeSignal<bool>>) -> Self {
        let value = value.into();

        Self {
            state: CheckboxState::none().signal(),
            // TODO: Maybe ContentLayout::Icon should be used as a single
            // char-sized square layout?
            layout: Layout::edge(Size::new_equal(16).into()),
            value,
            style: None,
        }
    }
}

impl<W: WidgetCtx> LayoutWidget<W> for Checkbox<W> {
    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }
}

impl<W: WidgetCtx> Widget<W> for Checkbox<W> {
    fn debug_name(&self) -> &'static str {
        "Checkbox"
    }

    fn flags(&self) -> WidgetFlags {
        WidgetFlags::default().hoverable().clickable().focusable()
    }

    fn build(&mut self, _ctx: BuildCtx<W>) {}

    fn layout(&self) -> Layout {
        self.layout
    }

    fn render(
        &self,
        mut ctx: crate::widget::RenderCtx<'_, W>,
    ) -> crate::widget::RenderResult {
        ctx.render_self(|mut ctx| {
            let style = ctx.get_style(self.style.as_deref());

            Block::from_layout_style(
                ctx.layout.outer,
                self.layout.with(|layout| layout.block_model()),
                style.container,
            )
            .render(ctx.renderer)?;

            ctx.render_focus_outline(ctx.id)?;

            if self.value.get()
                && let Some(icon_color) = style.icon_color.get()
            {
                let icon_style =
                    DrawStyle::default().stroke_width(2).stroke(icon_color);

                ctx.renderer.path(
                    &PathBuilder::new()
                        .with_lines(
                            CHECKBOX_ICON_POINTS
                                .iter()
                                .copied()
                                .map(|point| point + ctx.layout.inner.top_left),
                        )
                        .build(),
                    &icon_style,
                )?;
            }

            Ok(())
        })
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.handle()?;

        ctx.handle_focusable(|ctx, pressed| {
            let current_state = self.state.get();

            if current_state.pressed != pressed {
                if !current_state.pressed && pressed {
                    self.value.update(|value| *value = !*value);
                }

                self.state.update(|state| state.pressed = pressed);

                ctx.capture()
            } else {
                ctx.ignore()
            }
        })
    }
}
