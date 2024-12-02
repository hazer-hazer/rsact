use core::marker::PhantomData;
use layout::{flex::flex_content_size, size::RectangleExt};
use rsact_icons::system::SystemIcon;
use rsact_ui::{
    embedded_graphics::prelude::{Point, Transform},
    prelude::*,
    render::Renderable,
};

// pub trait EncoderKeyboardEvent {
//     fn as_encoder_keyboard_select(&self) -> Option<i32>;
// }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EncoderKeyboardState {
    pressed: bool,
    active: bool,
    // TODO: Merge with active?
    selected: usize,
}

impl EncoderKeyboardState {
    fn none() -> Self {
        Self { pressed: false, active: false, selected: 0 }
    }
}

declare_widget_style! {
    EncoderKeyboardStyle (EncoderKeyboardState) {
        container: container,
        selected: container {
            selected_background_color: background_color,
            selected_border_color: border_color,
            selected_border_radius: border_radius,
        },
    }
}

impl<C: Color> EncoderKeyboardStyle<C> {
    pub fn base() -> Self {
        Self {
            container: BlockStyle::base(),
            selected: BlockStyle::base().border(
                BorderStyle::base().radius(5).color(C::default_foreground()),
            ),
        }
    }
}

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

pub enum EncoderKeyboardKind {
    Char(char),
    Delete,
    Done,
    // TODO: CapsLock
}

pub struct EncoderKeyboardKey<W: WidgetCtx> {
    el: El<W>,
    kind: EncoderKeyboardKind,
}

impl<W: WidgetCtx> EncoderKeyboardKey<W>
where
    W::Styler: WidgetStylist<MonoTextStyle<W::Color>>
        + WidgetStylist<IconStyle<W::Color>>,
{
    fn new(inner: El<W>, kind: EncoderKeyboardKind) -> Self {
        Self { el: Container::new(inner).padding(5u32).el(), kind }
    }

    fn char(char: char) -> Self {
        Self::new(MonoText::new(char).el(), EncoderKeyboardKind::Char(char))
    }

    fn delete() -> Self {
        Self::new(
            Icon::new(SystemIcon::Cancel).el(),
            EncoderKeyboardKind::Delete,
        )
    }

    fn done() -> Self {
        Self::new(Icon::new(SystemIcon::Check).el(), EncoderKeyboardKind::Done)
    }
}

/**
 * Keyboard for use with encoder to input text.
 */
pub struct EncoderKeyboard<W: WidgetCtx, Dir: Direction> {
    id: ElId,
    layout: Signal<Layout>,
    keys: Vec<EncoderKeyboardKey<W>>,
    value: Signal<String>,
    state: Signal<EncoderKeyboardState>,
    style: MemoChain<EncoderKeyboardStyle<W::Color>>,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx> EncoderKeyboard<W, RowDir>
where
    W::Styler: WidgetStylist<MonoTextStyle<W::Color>>
        + WidgetStylist<IconStyle<W::Color>>,
{
    pub fn row(value: Signal<String>) -> Self {
        Self::new(value)
    }
}

impl<W: WidgetCtx> EncoderKeyboard<W, ColDir>
where
    W::Styler: WidgetStylist<MonoTextStyle<W::Color>>
        + WidgetStylist<IconStyle<W::Color>>,
{
    pub fn col(value: Signal<String>) -> Self {
        Self::new(value)
    }
}

impl<W: WidgetCtx, Dir: Direction + 'static> EncoderKeyboard<W, Dir>
where
    W::Styler: WidgetStylist<MonoTextStyle<W::Color>>
        + WidgetStylist<IconStyle<W::Color>>,
{
    pub fn new(value: Signal<String>) -> Self {
        let state = EncoderKeyboardState::none().signal();
        let style = EncoderKeyboardStyle::base().memo_chain();

        let mut keys = ('a'..='z')
            .map(|char| EncoderKeyboardKey::char(char))
            .collect::<Vec<_>>();

        keys.push(EncoderKeyboardKey::delete());
        keys.push(EncoderKeyboardKey::done());

        let layout = Layout::shrink(LayoutKind::Flex(
            FlexLayout::base(
                Dir::AXIS,
                flex_content_size(Dir::AXIS, keys.iter().map(|key| &key.el))
                    .inert()
                    .memo(),
            )
            .block_model(BlockModel::zero().padding(2)),
        ))
        .signal();

        Self {
            id: ElId::unique(),
            layout,
            keys,
            value,
            state,
            style,
            dir: PhantomData,
        }
    }
}

impl<W: WidgetCtx, Dir: Direction> SizedWidget<W> for EncoderKeyboard<W, Dir> where
    W::Styler: WidgetStylist<EncoderKeyboardStyle<W::Color>>
{
}

impl<W: WidgetCtx, Dir: Direction> BlockModelWidget<W>
    for EncoderKeyboard<W, Dir>
where
    W::Styler: WidgetStylist<EncoderKeyboardStyle<W::Color>>,
{
}

impl<W: WidgetCtx, Dir: Direction> Widget<W> for EncoderKeyboard<W, Dir>
where
    W::Styler: WidgetStylist<EncoderKeyboardStyle<W::Color>>,
{
    fn meta(&self) -> rsact_ui::widget::MetaTree {
        MetaTree::childless(Meta::focusable(self.id).inert())
    }

    fn on_mount(&mut self, ctx: rsact_ui::widget::MountCtx<W>) {
        // TODO
        ctx.accept_styles(self.style, self.state);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        MemoTree {
            data: self.layout.memo(),
            children: self
                .keys
                .iter()
                .map(|key| key.el.build_layout_tree())
                .collect::<Vec<_>>()
                .inert()
                .memo(),
        }
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        let state = self.state.get();
        let style = self.style.get();

        if !state.active {
            ctx.draw_focus_outline(self.id)?;
        }

        let keys_offset = {
            let selected_key_layout =
                ctx.layout.children().nth(state.selected).unwrap();

            let selected_key_offset =
                ctx.layout.inner.center_offset_of(selected_key_layout.inner);

            if state.active {
                Block::from_layout_style(
                    selected_key_layout.outer.translate(selected_key_offset),
                    BlockModel::zero().border_width(1),
                    style.selected,
                )
                .render(ctx.renderer)?;
            }

            selected_key_offset
        };

        ctx.renderer.clipped(ctx.layout.inner, |renderer| {
            self.keys.iter().zip(ctx.layout.children()).try_for_each(
                |(key, key_layout)| {
                    let mut ctx = DrawCtx {
                        state: ctx.state,
                        renderer,
                        layout: &key_layout.translate(keys_offset),
                        tree_style: ctx.tree_style,
                    };

                    key.el.draw(&mut ctx)
                },
            )
        })
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse<W> {
        let state = self.state.get();

        if state.active && ctx.is_focused(self.id) {
            if let Some(offset) = ctx.event.as_focus_move() {
                let current = state.selected;

                let new = (current as i32 + offset)
                    .clamp(0, self.keys.len().saturating_sub(1) as i32)
                    as usize;

                if new != current {
                    self.state.update(|state| {
                        state.selected = new;
                    });
                }

                return ctx.capture();
            }
        }

        ctx.handle_focusable(self.id, |ctx, pressed| {
            if state.pressed != pressed {
                if state.active {
                    let key = &self.keys[state.selected];

                    match key.kind {
                        EncoderKeyboardKind::Char(char) => {
                            self.value.update(|value| value.push(char))
                        },
                        EncoderKeyboardKind::Delete => {
                            self.value.update(|value| {
                                value.pop();
                            })
                        },
                        EncoderKeyboardKind::Done => {
                            self.state.update(|state| state.active = false);
                        },
                    }

                    ctx.capture()
                } else if !state.pressed && pressed {
                    // Enter edit mode
                    self.state.update(|state| {
                        state.active = true;
                    });

                    ctx.capture()
                } else {
                    ctx.ignore()
                }
            } else {
                ctx.ignore()
            }
        })
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
