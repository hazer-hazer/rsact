use super::{
    icon::{Icon, IconStyle},
    ContainerLayout,
};
use crate::{render::Renderable, widget::prelude::*};
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
        Self {
            container: BlockStyle::base().border(
                BorderStyle::base().color(C::default_foreground()).radius(5),
            ),
        }
    }
}

// TODO: Do we need `on_change` event with signal value?

type IconKind = SystemIcon;

pub struct Checkbox<W: WidgetCtx> {
    id: ElId,
    state: Signal<CheckboxState>,
    layout: Signal<Layout>,
    // TODO: Reactive icon?
    icon: Icon<W, IconKind>,
    value: MaybeSignal<bool>,
    style: MemoChain<CheckboxStyle<W::Color>>,
}

impl<W: WidgetCtx> Checkbox<W>
where
    W::Styler: WidgetStylist<IconStyle<W::Color>>,
{
    pub fn new(value: impl Into<MaybeSignal<bool>>) -> Self {
        let icon = Icon::new(SystemIcon::Check);

        Self {
            id: ElId::unique(),
            state: CheckboxState::none().signal(),
            layout: Layout::shrink(LayoutKind::Container(
                ContainerLayout::base(
                    icon.layout().map(|icon_layout| icon_layout.content_size()),
                )
                .block_model(BlockModel::zero().border_width(1)),
            ))
            .signal(),
            icon,
            value: value.into(),
            style: CheckboxStyle::base().memo_chain(),
        }
    }

    pub fn check_icon(mut self, icon: IconKind) -> Self {
        self.icon.kind.set(icon);
        self
    }
}

impl<W: WidgetCtx> Widget<W> for Checkbox<W>
where
    W::Styler: WidgetStylist<CheckboxStyle<W::Color>>
        + WidgetStylist<IconStyle<W::Color>>,
{
    fn meta(&self) -> MetaTree {
        let id = self.id;
        MetaTree::childless(move || Meta::focusable(id))
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);
        self.icon.on_mount(ctx)
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> rsact_reactive::prelude::MemoTree<Layout> {
        let icon_layout = self.icon.build_layout_tree();

        MemoTree {
            data: self.layout.memo(),
            children: create_memo(move |_| vec![icon_layout]),
        }
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, W>,
    ) -> crate::widget::DrawResult {
        let style = self.style.get();

        Block::from_layout_style(
            ctx.layout.outer,
            self.layout.with(|layout| layout.block_model()),
            style.container,
        )
        .render(ctx.renderer)?;

        ctx.draw_focus_outline(self.id)?;

        if self.value.get() {
            ctx.draw_child(&self.icon)?;
        }

        Ok(())
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse {
        ctx.handle_focusable(self.id, |ctx, pressed| {
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
