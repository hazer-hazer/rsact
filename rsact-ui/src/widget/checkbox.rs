use super::ContainerLayout;
use crate::widget::prelude::*;
use rsact_reactive::prelude::*;

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
    }
}

impl<C: Color> CheckboxStyle<C> {
    pub fn base() -> Self {
        Self {
            container: BlockStyle::base().border(
                BorderStyle::base().color(C::default_foreground()).radius(2),
            ),
        }
    }
}

// TODO: Do we need `on_change` event while having signal value?

// TODO: Custom icon
pub struct Checkbox<W: WidgetCtx> {
    state: Signal<CheckboxState>,
    layout: Layout,
    size: MaybeSignal<u32>,
    value: MaybeSignal<bool>,
    style:
        Option<Box<dyn Fn(CheckboxStyle<W::Color>) -> CheckboxStyle<W::Color>>>,
}

impl<W: WidgetCtx> Checkbox<W> {
    pub fn new(value: impl Into<MaybeSignal<bool>>) -> Self {
        let value = value.into();

        Self {
            state: CheckboxState::none().signal(),
            // TODO: Maybe ContentLayout::Icon should be used as a single char-sized square layout?
            layout: Layout::shrink(LayoutKind::Edge),
            value,
            style: None,
        }
    }
}

impl<W: WidgetCtx> Widget<W> for Checkbox<W> {
    fn debug_name(&self) -> &'static str {
        "Checkbox"
    }

    fn build(&mut self, mut ctx: BuildCtx<W>) {
        ctx.set_single_child(&mut self.icon);
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    fn render(
        &self,
        mut ctx: crate::widget::RenderCtx<'_, W>,
    ) -> crate::widget::RenderResult {
        let style = ctx.get_style(|t| t.checkbox, self.style.as_deref());

        Block::from_layout_style(
            ctx.layout.outer,
            self.layout.with(|layout| layout.block_model()),
            style.container,
        )
        .render(ctx.renderer)?;

        ctx.render_focus_outline(ctx.id)?;

        Ok(())
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
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
