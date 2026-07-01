use crate::widget::prelude::*;
use rsact_reactive::prelude::*;

// TODO: Responsive for size
// 3 8 7 12 13 4
const CHECKBOX_ICON_POINTS: &[Point] =
    &[Point::new(3, 8), Point::new(7, 12), Point::new(13, 4)];

declare_widget_style! {
    CheckboxStyle () {
        container: container,
        icon_color: color = ColorStyle::DefaultForeground,
    }
}

// TODO: Do we need `on_change` event if we have signal value?

// TODO: Custom icon
#[derive(View)]
pub struct Checkbox<W: WidgetCtx> {
    layout: Layout,
    value: Signal<bool>,
    style: WidgetStyleFn<CheckboxStyle<W::Color>>,
}

impl<W: WidgetCtx> Checkbox<W> {
    pub fn new(value: impl IntoSignal<bool>) -> Self {
        Self {
            // TODO: Maybe ContentLayout::Icon should be used as a single
            // char-sized square layout?
            layout: Layout::edge(Size::new_equal(16).into()),
            // Promote to a real `Signal` so the checked state is tracked on
            // read in `render` and notified on write in `on_event`. A plain
            // value (`Checkbox::new(true)`) becomes an owned signal; a passed
            // `Signal` is reused, preserving two-way binding.
            value: value.signal(),
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
        ctx.handle()?; // hover + press claim + pointer capture (automatic)
        ctx.handle_click(|ctx| {
            self.value.update(|value| *value = !*value);
            ctx.capture()
        })
    }
}
