use crate::{
    style::{Styler, WidgetStyle},
    widget::{prelude::*, BoxModelWidget, SizedWidget},
};

pub trait ButtonEvent {
    fn as_button_press(&self) -> bool;
    fn as_button_release(&self) -> bool;
}

#[derive(Clone, Copy)]
pub struct ButtonState {
    pub pressed: bool,
    // pub focused: bool,
}

impl ButtonState {
    pub fn none() -> Self {
        Self { pressed: false }
    }
}

// pub trait ButtonStyler<C: Color> {
//     type Class;

//     fn default() -> Self::Class;

//     fn style(
//         self,
//         class: Self::Class,
//     ) -> impl Fn(ButtonStyle<C>, ButtonState) -> ButtonStyle<C> + 'static;
// }

// impl<C: Color + 'static> ButtonStyler<C> for ButtonStyle<C> {
//     type Class = ();

//     fn default() -> Self::Class {
//         ()
//     }

//     fn style(
//         self,
//         _class: Self::Class,
//     ) -> impl Fn(ButtonStyle<C>, ButtonState) -> ButtonStyle<C> + 'static {
//         move |_, _| self
//     }
// }

// impl<C, F> ButtonStyler<C> for F
// where
//     C: Color,
//     F: Fn(ButtonStyle<C>, ButtonState) -> ButtonStyle<C> + 'static,
// {
//     type Class = ();

//     fn default() -> Self::Class {
//         ()
//     }

//     fn style(
//         self,
//         _class: Self::Class,
//     ) -> impl Fn(ButtonStyle<C>, ButtonState) -> ButtonStyle<C> + 'static {
//         self
//     }
// }

// pub trait ButtonStyler {
//     type Color: Color;
//     type Class;

//     fn default() -> Self::Class;
//     fn style(
//         &self,
//         class: Self::Class,
//         state: ButtonState,
//     ) -> ButtonStyle<Self::Color>;
// }

// impl<F, C: Color> ButtonStyler for F
// where
//     F: Fn(ButtonState) -> ButtonStyle<C>,
// {
//     type Color = C;
//     type Class = ButtonClass;

//     fn default() -> Self::Class {
//         ButtonClass::Normal
//     }

//     fn style(
//         &self,
//         class: Self::Class,
//         state: ButtonState,
//     ) -> ButtonStyle<Self::Color> {
//         match class {
//             ButtonClass::Normal => todo!(),
//         }
//     }
// }

// impl<C: Color, F: Fn(ButtonState) -> ButtonStyle<C>> ButtonStyler<C> for F {
//     type Class = Self;

//     fn default(&self) -> Self::Class {
//         *self
//     }

//     fn style(&self, class: &Self::Class, state: ButtonState) ->
// ButtonStyle<C> {         class(state)
//     }
// }

#[derive(Clone, Copy, PartialEq)]
pub struct ButtonStyle<C: Color> {
    pub container: BoxStyle<C>,
}

impl<C: Color> WidgetStyle for ButtonStyle<C> {
    type Color = C;
    type Inputs = ButtonState;
}

impl<C: Color> ButtonStyle<C> {
    pub fn base() -> Self {
        Self {
            container: BoxStyle::base()
                .border(BorderStyle::base().color(C::default_foreground())),
        }
    }

    pub fn container(mut self, container: BoxStyle<C>) -> Self {
        self.container = container;
        self
    }
}

// fn default_styler<C: Color>(status: ButtonStatus) -> ButtonStyle<C> {
//     let base = ButtonStyle::base();
//     match status {
//         ButtonStatus { pressed: true } => base,
//         ButtonStatus { pressed: false } => base,
//     }
// }

pub struct Button<W: WidgetCtx> {
    id: ElId,
    layout: Signal<Layout>,
    content: Signal<El<W>>,
    state: Signal<ButtonState>,
    // style: Signal<ButtonStyle<C::Color>>,
    // style: Option<ButtonStyle<C>>,
    style: MemoChain<ButtonStyle<W::Color>>,
    on_click: Option<Box<dyn Fn()>>,
}

impl<W: WidgetCtx + 'static> Button<W> {
    pub fn new(content: impl IntoSignal<El<W>>) -> Self {
        let content = content.into_signal();
        let state = use_signal(ButtonState::none());

        let layout = Layout::new(
            LayoutKind::Container(ContainerLayout {
                box_model: BoxModel::zero().border_width(1).padding(5),
                horizontal_align: Align::Center,
                vertical_align: Align::Center,
            }),
            content.mapped(|content| {
                content.layout().with(|layout| layout.content_size.get())
            }),
        )
        .into_signal();

        Self {
            id: ElId::unique(),
            layout,
            content,
            state,
            style: use_memo_chain(|_| ButtonStyle::base()),
            on_click: None,
        }
    }

    pub fn on_click<F: 'static>(mut self, on_click: F) -> Self
    where
        F: Fn(),
    {
        self.on_click = Some(Box::new(on_click));
        self
    }

    pub fn use_state(
        self,
        state: impl WriteSignal<ButtonState> + 'static,
    ) -> Self {
        state.set_from(self.state);
        self
    }

    // pub fn style(
    //     self,
    //     style: impl Fn(ButtonState) -> ButtonStyle<C::Color> + 'static,
    // ) -> Self {
    //     self.style.set_from(self.state.mapped_clone(style));
    //     self
    // }

    // pub fn style(
    //     self,
    //     style: impl ReadSignal<ButtonStyle<C::Color>> + 'static,
    // ) -> Self {
    //     self.style.set_from(style);
    //     self
    // }

    pub fn style(
        self,
        styler: impl Fn(ButtonStyle<W::Color>, ButtonState) -> ButtonStyle<W::Color>
            + 'static,
    ) -> Self {
        let state = self.state;
        self.style.last(move |base| styler(*base, state.get()));
        self
    }

    // pub fn style(mut self, style: ButtonStyler<C::Color>) -> Self {
    //     self.style = Some(style);
    //     self
    // }
}

impl<W: WidgetCtx + 'static> SizedWidget<W> for Button<W>
where
    W::Event: ButtonEvent,
    W::Styler: Styler<ButtonStyle<W::Color>, Class = ()>,
{
}

impl<W: WidgetCtx + 'static> BoxModelWidget<W> for Button<W>
where
    W::Event: ButtonEvent,
    W::Styler: Styler<ButtonStyle<W::Color>, Class = ()>,
{
}

impl<W: WidgetCtx + 'static> Widget<W> for Button<W>
where
    W::Event: ButtonEvent,
    W::Styler: Styler<ButtonStyle<W::Color>, Class = ()>,
{
    fn children_ids(&self) -> Memo<Vec<ElId>> {
        let id = self.id;
        use_memo(move |_| vec![id])
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        // let state = self.state;
        // let styler = ctx.styler.get().style(());
        // self.style.then(move |base| styler(*base, state.get()));

        self.content.update_untracked(|content| {
            ctx.pass_to_children(core::slice::from_mut(content))
        });

        ctx.accept_styles(self.style, self.state);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        MemoTree {
            data: self.layout.into_memo(),
            children: self
                .content
                .mapped(|content| vec![content.build_layout_tree()]),
        }
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        ctx.draw_focus_outline(self.id)?;

        let style = self.style.get();

        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.area,
            self.layout.get().box_model(),
            style.container,
        ))?;

        self.content.with(|content| ctx.draw_child(content))
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse<<W as WidgetCtx>::Event> {
        ctx.handle_focusable(self.id, |pressed| {
            let current_state = self.state.get();

            // if current_state.focused != is_focused {
            //     self.state.update(|state| state.focused = is_focused);
            // }

            // if !is_focused {
            //     return Propagate::Ignored.into();
            // }

            if current_state.pressed != pressed {
                if let Some(on_click) = self.on_click.as_ref() {
                    if !current_state.pressed && pressed {
                        on_click()
                    }
                }

                self.state.update(|state| state.pressed = pressed);

                Capture::Captured.into()
            } else {
                Propagate::Ignored.into()
            }
        })
    }
}

// impl<C> From<Button<C>> for El<C>
// where
//     C::Event: ButtonEvent,
//     C: WidgetCtx + 'static,
// {
//     fn from(value: Button<C>) -> Self {
//         El::new(value)
//     }
// }
