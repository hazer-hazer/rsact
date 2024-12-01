use core::marker::PhantomData;
use rsact_ui::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectKeyboardState {
    pressed: bool,
    active: bool,
    // TODO: Merge focused with active into Option<usize>
    focused: usize,
}

impl SelectKeyboardState {
    fn none() -> Self {
        Self { pressed: false, active: false, focused: 0 }
    }
}

// declare_widget_style! {
//     SelectKeyboardStyle (SelectKeyboardState) {
//         container: container,
//     }
// }

// pub enum SelectKeyboardBtn {
//     Push(char),
//     Pop,
//     Enter,
// }

// pub trait Alphabet {
//     type Key: PartialEq;

//     fn to_string(keys: Vec<Self::Key>) -> String;

//     // fn buttons<W: WidgetCtx>() -> impl Iterator<Item = (char, Button<W>)>;
//     fn buttons() -> impl Iterator<Item = &'static str>;
// }

/**
 * Keyboard for use with encoder to input text.
 */
pub struct EncoderKeyboard<W: WidgetCtx, Dir: Direction> {
    id: ElId,
    buttons_count: usize,
    content: Scrollable<W, Dir>,
    state: Signal<SelectKeyboardState>,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx> EncoderKeyboard<W, RowDir>
where
    W::Styler: WidgetStylist<ButtonStyle<W::Color>>
        + WidgetStylist<MonoTextStyle<W::Color>>
        + WidgetStylist<ScrollableStyle<W::Color>>,
    W::Event: ButtonEvent + ScrollEvent,
{
    pub fn row(value: Signal<String>) -> Self {
        Self::new(value)
    }
}

impl<W: WidgetCtx> EncoderKeyboard<W, ColDir>
where
    W::Styler: WidgetStylist<ButtonStyle<W::Color>>
        + WidgetStylist<MonoTextStyle<W::Color>>
        + WidgetStylist<ScrollableStyle<W::Color>>,
    W::Event: ButtonEvent + ScrollEvent,
{
    pub fn col(value: Signal<String>) -> Self {
        Self::new(value)
    }
}

impl<W: WidgetCtx, Dir: Direction + 'static> EncoderKeyboard<W, Dir>
where
    W::Styler: WidgetStylist<ButtonStyle<W::Color>>
        + WidgetStylist<MonoTextStyle<W::Color>>
        + WidgetStylist<ScrollableStyle<W::Color>>,
    W::Event: ButtonEvent + ScrollEvent,
{
    pub fn new(mut value: Signal<String>) -> Self {
        let mut state = SelectKeyboardState::none().signal();
        let mut buttons = ('a'..='z')
            .map(|char| {
                Button::new(char)
                    .on_click(move || {
                        println!("Add char {char}");
                        value.update(|value| value.push(char));
                    })
                    .el()
            })
            .collect::<Vec<_>>();

        buttons.push(
            Button::new("DEL")
                .on_click(move || {
                    value.update(|value| value.pop());
                })
                .el(),
        );
        buttons.push(
            Button::new("ENTER")
                .on_click(move || {
                    state.update(|state| state.active = false);
                })
                .el(),
        );

        let buttons_count = buttons.len();

        // TODO: API for flex gaps, padding, etc.
        let content = Flex::<_, Dir>::new(buttons).gap(5);
        let content = Scrollable::<_, Dir>::new(content).tracker();

        Self {
            id: ElId::unique(),
            buttons_count,
            content,
            state,
            dir: PhantomData,
        }
    }
}

impl<W: WidgetCtx, Dir: Direction> SizedWidget<W> for EncoderKeyboard<W, Dir>
where
    W::Styler: WidgetStylist<ButtonStyle<W::Color>>
        + WidgetStylist<MonoTextStyle<W::Color>>
        + WidgetStylist<ScrollableStyle<W::Color>>,
    W::Event: ButtonEvent + ScrollEvent,
{
}

impl<W: WidgetCtx, Dir: Direction> Widget<W> for EncoderKeyboard<W, Dir>
where
    W::Styler: WidgetStylist<ButtonStyle<W::Color>>
        + WidgetStylist<MonoTextStyle<W::Color>>
        + WidgetStylist<ScrollableStyle<W::Color>>,
    W::Event: ButtonEvent + ScrollEvent,
    // where
    //     W::Styler: WidgetStylist<SelectKeyboardStyle<W::Color>>,
{
    fn meta(&self) -> rsact_ui::widget::MetaTree {
        let id = self.id;
        let content_meta = self.content.meta();

        MetaTree {
            data: self.state.map(move |state| {
                if state.active {
                    Meta::none()
                } else {
                    Meta::focusable(id)
                }
            }),
            children: self.state.map(move |state| {
                if state.active {
                    vec![content_meta]
                } else {
                    vec![]
                }
            }),
        }
    }

    fn on_mount(&mut self, ctx: rsact_ui::widget::MountCtx<W>) {
        // ctx.accept_styles(self.style, self.state);
        self.content.on_mount(ctx);
    }

    fn layout(&self) -> Signal<Layout> {
        self.content.layout()
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        self.content.build_layout_tree()
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        ctx.draw_focus_outline(self.id)?;

        self.content.draw(ctx)
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse<W> {
        let state = self.state.get();

        if state.active {
            if let Some(offset) = ctx.event.as_focus_move() {
                let prev_focused = state.focused;
                let new_focused = self.state.update(|state| {
                    state.focused = (state.focused as i64 + offset as i64)
                        .clamp(0, self.buttons_count as i64)
                        as usize;
                    state.focused
                });

                // Intercept focusing event and define local focusing logic.
                ctx.bubble(BubbledData::FocusOffset(
                    new_focused as i32 - prev_focused as i32,
                ))
            } else {
                // Deactivation done on "Enter" button press
                ctx.pass_to_child(&mut self.content)
            }
        } else {
            ctx.handle_focusable(self.id, |ctx, press| {
                if state.pressed != press {
                    // Always activates,
                    let activate = !state.pressed && press;

                    self.state.update(|state| {
                        state.pressed = press;
                        if activate {
                            state.active = true;
                        }
                    });

                    // Refocusing since focus is changed. The target element in the tree is the first button in the keyboard.
                    ctx.bubble(BubbledData::FocusOffset(0))
                } else {
                    ctx.ignore()
                }
            })
        }
    }
}

// pub fn select_keyboard<Dir: Direction + 'static, A: Alphabet, W: WidgetCtx>(
//     value: Signal<String>,
// ) -> El<W>
// where
//     W::Styler: Styler<ButtonStyle<W::Color>, Class = ()>,
//     W::Event: ButtonEvent,
//     W::Styler: Styler<MonoTextStyle<W::Color>, Class = ()>,
// {
//     Flex::<W, Dir>::new(
//         A::buttons()
//             .map(|btn| {
//                 Button::new(MonoText::new(btn).el())
//                     .on_click(move || {
//                         value.update(|value| value.push_str(btn));
//                         None
//                     })
//                     .el()
//             })
//             .chain([Button::new("del").on_click(move || {
//                 value.update(|value| value.pop());
//                 None
//             }).el(), Button::new("enter").on_click(move || {
//                 value.update(|value| value.)
//               None
//             })].into_iter())
//             .collect::<Vec<_>>(),
//     )
//     .shrink()
//     .el()
// }
