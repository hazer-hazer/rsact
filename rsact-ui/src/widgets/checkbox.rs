use super::icon::{Icon, IconKind};
use crate::{
    el::ElId,
    event::{Capture, Propagate},
    font::FontSize,
    layout::{
        size::{Length, Size},
        Layout, Limits,
    },
    render::{color::Color, Block, Renderer},
    style::block::BoxStyle,
    widget::{prelude::BoxModel, Widget, WidgetCtx},
};
use rsact_core::{
    mapped,
    memo::{IntoMemo, MemoTree},
    memo_chain::IntoMemoChain,
    prelude::{use_memo, use_memo_chain, use_signal, MemoChain},
    signal::{
        IntoSignal, ReadSignal, Signal, SignalMapper, SignalSetter, WriteSignal,
    },
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

#[derive(Clone, Copy, PartialEq)]
pub struct CheckboxStyle<C: Color> {
    block: BoxStyle<C>,
}

impl<C: Color> CheckboxStyle<C> {
    pub fn base() -> Self {
        Self { block: BoxStyle::base() }
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

impl<W: WidgetCtx> Checkbox<W> {
    pub fn new(value: impl IntoSignal<bool>) -> Self {
        let icon = Icon::new(IconKind::Check);
        let icon_layout = icon.layout();

        Self {
            id: ElId::unique(),
            state: CheckboxState::none().into_signal(),
            layout: Layout {
                kind: crate::layout::LayoutKind::Edge,
                size: Size::shrink(),
                content_size: icon_layout
                    .mapped(move |layout| layout.content_size.get()),
            }
            .into_signal(),
            icon,
            value: value.into_signal(),
            style: CheckboxStyle::base().into_memo_chain(),
        }
    }
}

impl<W: WidgetCtx> Widget<W> for Checkbox<W> {
    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        self.icon.on_mount(ctx)
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> rsact_core::prelude::MemoTree<Layout> {
        MemoTree::childless(self.layout.into_memo())
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, W>,
    ) -> crate::widget::DrawResult {
        let style = self.style.get();

        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.area,
            self.layout.get().box_model(),
            style.block,
        ));

        if self.value.get() {
            ctx.draw_child(&self.icon)?;
        }

        Ok(())
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> crate::event::EventResponse<<W as WidgetCtx>::Event> {
        ctx.handle_focusable(self.id, |pressed| {
            let current_state = self.state.get();

            if current_state.pressed != pressed {
                if !current_state.pressed && pressed {
                    self.value.update(|value| *value = !*value);
                }

                self.state.update(|state| state.pressed = pressed);

                Capture::Captured.into()
            } else {
                Propagate::Ignored.into()
            }
        })
    }
}
