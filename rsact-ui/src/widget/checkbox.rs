use super::{
    icon::{Icon, IconStyle},
    ContainerLayout,
};
use crate::widget::prelude::*;
use rsact_icons::system::SystemIcon;

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
        Self { container: BlockStyle::base() }
    }
}

// TODO: Do we need `on_change` event with signal value?

type IconType = SystemIcon;

pub struct Checkbox<W: WidgetCtx> {
    id: ElId,
    state: Signal<CheckboxState>,
    layout: Signal<Layout>,
    icon: Icon<W, IconType>,
    value: Signal<bool>,
    style: MemoChain<CheckboxStyle<W::Color>>,
}

impl<W: WidgetCtx> Checkbox<W>
where
    W::Styler: Styler<IconStyle<W::Color>, Class = ()>,
{
    pub fn new(value: impl IntoSignal<bool>) -> Self {
        let icon = Icon::new(SystemIcon::Check);

        Self {
            id: ElId::unique(),
            state: CheckboxState::none().into_signal(),
            layout: Layout::shrink(LayoutKind::Container(
                ContainerLayout::base(
                    icon.layout()
                        .mapped(|icon_layout| icon_layout.content_size()),
                ),
            ))
            .into_signal(),
            icon,
            value: value.into_signal(),
            style: CheckboxStyle::base().into_memo_chain(),
        }
    }

    pub fn check_icon(self, icon: IconType) -> Self {
        self.icon.icon.set(icon);
        self
    }
}

impl<W: WidgetCtx> Widget<W> for Checkbox<W>
where
    W::Styler: Styler<CheckboxStyle<W::Color>, Class = ()>
        + Styler<IconStyle<W::Color>, Class = ()>,
{
    fn meta(&self) -> MetaTree {
        MetaTree::childless(Meta::focusable(self.id))
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);
        self.icon.on_mount(ctx)
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> rsact_reactive::prelude::MemoTree<Layout> {
        MemoTree::childless(self.layout.into_memo())
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, W>,
    ) -> crate::widget::DrawResult {
        let style = self.style.get();

        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.outer,
            self.layout.get().block_model(),
            style.container,
        ))?;

        if self.value.get() {
            ctx.draw_child(&self.icon)?;
        }

        Ok(())
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse<W> {
        ctx.handle_focusable(self.id, |pressed| {
            let current_state = self.state.get();

            if current_state.pressed != pressed {
                if !current_state.pressed && pressed {
                    self.value.update(|value| *value = !*value);
                }

                self.state.update(|state| state.pressed = pressed);

                W::capture()
            } else {
                W::ignore()
            }
        })
    }
}
