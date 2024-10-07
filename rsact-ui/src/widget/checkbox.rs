use super::icon::{Icon, IconKind, IconStyle};
use crate::{
    declare_widget_style,
    el::ElId,
    event::EventResponse,
    layout::{ContentLayout, Layout, LayoutKind},
    render::{color::Color, Block, Renderer},
    style::{block::BlockStyle, Styler},
    widget::{Meta, MetaTree, Widget, WidgetCtx},
};
use rsact_reactive::{
    memo::{IntoMemo, MemoTree},
    memo_chain::IntoMemoChain,
    prelude::MemoChain,
    signal::{IntoSignal, ReadSignal, Signal, SignalMapper, WriteSignal},
};

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
// TODO: Add custom icons

pub struct Checkbox<W: WidgetCtx> {
    id: ElId,
    state: Signal<CheckboxState>,
    layout: Signal<Layout>,
    icon: Icon<W>,
    value: Signal<bool>,
    style: MemoChain<CheckboxStyle<W::Color>>,
}

impl<W: WidgetCtx> Checkbox<W>
where
    W::Styler: Styler<IconStyle<W::Color>, Class = ()>,
{
    pub fn new(value: impl IntoSignal<bool>) -> Self {
        let icon = Icon::new(IconKind::Check);
        let icon_layout = icon.layout();

        Self {
            id: ElId::unique(),
            state: CheckboxState::none().into_signal(),
            layout: Layout::shrink(LayoutKind::Content(ContentLayout {
                content_size: icon_layout
                    .mapped(move |layout| layout.content_size()),
            }))
            .into_signal(),
            icon,
            value: value.into_signal(),
            style: CheckboxStyle::base().into_memo_chain(),
        }
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
            ctx.layout.area,
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
