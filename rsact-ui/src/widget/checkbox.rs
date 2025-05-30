use super::{
    ContainerLayout,
    icon::{Icon, IconStyle},
};
use crate::{render::Renderable, widget::prelude::*};
use rsact_icons::{IconSet, system::SystemIcon};
use rsact_reactive::maybe::{IntoMaybeReactive, IsReactive};

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

type IconKind = SystemIcon;

pub struct Checkbox<W: WidgetCtx> {
    state: Signal<CheckboxState>,
    layout: Signal<Layout>,
    icon: El<W>,
    value: MaybeSignal<bool>,
    style: MemoChain<CheckboxStyle<W::Color>>,
}

impl<W: WidgetCtx> Checkbox<W>
where
    W::Styler: WidgetStylist<IconStyle<W::Color>>,
{
    pub fn new(value: impl Into<MaybeSignal<bool>>) -> Self {
        Self::new_with_icon(value, SystemIcon::Check.inert())
    }

    // TODO: Any IconSet?
    pub fn new_with_icon(
        value: impl Into<MaybeSignal<bool>>,
        icon: impl IntoMaybeReactive<SystemIcon>,
    ) -> Self {
        let value = value.into();
        let icon = Icon::new(icon).visible(value.map(|value| *value)).el();

        Self {
            state: CheckboxState::none().signal(),
            layout: Layout::shrink(LayoutKind::Container(
                ContainerLayout::base(icon.layout().memo())
                    .block_model(BlockModel::zero().border_width(1)),
            ))
            .signal(),
            icon,
            value,
            style: CheckboxStyle::base().memo_chain(),
        }
    }
}

impl<W: WidgetCtx> Widget<W> for Checkbox<W>
where
    W::Styler: WidgetStylist<CheckboxStyle<W::Color>>
        + WidgetStylist<IconStyle<W::Color>>,
{
    fn meta(&self, id: ElId) -> MetaTree {
        MetaTree::childless(move || Meta::focusable(id))
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);
        ctx.pass_to_child(self.layout, &mut self.icon);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn render(
        &self,
        ctx: &mut crate::widget::RenderCtx<'_, W>,
    ) -> crate::widget::RenderResult {
        ctx.render_self(|ctx| {
            let style = self.style.get();

            Block::from_layout_style(
                ctx.layout.outer,
                self.layout.with(|layout| layout.block_model()),
                style.container,
            )
            .render(ctx.renderer())?;

            ctx.render_focus_outline(ctx.id)
        })?;

        ctx.render_child(&self.icon)?;

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
