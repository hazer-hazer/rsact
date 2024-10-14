// use rsact_ui::{
//     declare_widget_style,
//     el::ElId,
//     embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle},
//     embedded_text::{style::TextBoxStyleBuilder, TextBox},
//     layout::Layout,
//     prelude::*,
//     render::{color::Color, Block, Renderer},
//     style::Styler,
//     utils::cycle_index,
//     widget::{BlockModelWidget, SizedWidget, Widget, WidgetCtx},
// };
// use std::marker::PhantomData;

// pub trait MonoTextInputEvent {
//     fn as_char_select(&self) -> Option<i32>;
//     fn as_char_change(&self) -> Option<i32>;
// }

// #[derive(Clone, Copy, Debug, PartialEq)]
// pub struct TextInputState {
//     pressed: bool,
//     // active: bool,
//     focused_char: Option<usize>,
//     char_edit: bool,
// }

// impl TextInputState {
//     pub fn none() -> Self {
//         Self {
//             pressed: false,
//             // active: false,
//             focused_char: None,
//             char_edit: false,
//         }
//     }
// }

// declare_widget_style! {
//     TextInputStyle (TextInputState) {
//         container: container,
//     }
// }

// impl<C: Color> TextInputStyle<C> {
//     pub fn base() -> Self {
//         Self {
//             // TODO
//             container: BlockStyle::base(),
//         }
//     }
// }

// // TODO: Non-char Alphabet?
// pub trait Alphabet {
//     const CHARS: &[char];

//     // fn char(&self, index: usize) -> char {
//     //     Self::CHARS[index]
//     // }

//     // type Char;

//     fn len() -> usize {
//         Self::CHARS.len()
//     }
//     fn initial_index() -> usize {
//         0
//     }
//     fn index(char: char) -> usize {
//         Self::CHARS.iter().position(|ch| *ch == char).unwrap_or(0)
//     }
//     fn char(index: usize) -> char {
//         Self::CHARS[index]
//     }
// }

// pub struct MonoTextInput<W: WidgetCtx, A: Alphabet> {
//     id: ElId,
//     layout: Signal<Layout>,
//     state: Signal<TextInputState>,
//     style: MemoChain<TextInputStyle<W::Color>>,
//     chars: Signal<Vec<char>>,
//     text: Memo<String>,
//     alphabet: PhantomData<A>,
// }

// impl<W, A> BlockModelWidget<W> for MonoTextInput<W, A>
// where
//     W::Event: MonoTextInputEvent,
//     W::Styler: Styler<TextInputStyle<W::Color>, Class = ()>,
//     W: WidgetCtx,
//     A: Alphabet,
// {
// }
// impl<W, A> SizedWidget<W> for MonoTextInput<W, A>
// where
//     W::Event: MonoTextInputEvent,
//     W::Styler: Styler<TextInputStyle<W::Color>, Class = ()>,
//     W: WidgetCtx,
//     A: Alphabet,
// {
// }

// impl<W, A> Widget<W> for MonoTextInput<W, A>
// where
//     W::Event: MonoTextInputEvent,
//     W::Styler: Styler<TextInputStyle<W::Color>, Class = ()>,
//     W: WidgetCtx,
//     A: Alphabet,
// {
//     fn meta(&self) -> rsact_ui::widget::MetaTree {
//         todo!()
//     }

//     fn on_mount(&mut self, ctx: rsact_ui::widget::MountCtx<W>) {
//         ctx.accept_styles(self.style, self.state);
//     }

//     fn layout(&self) -> Signal<Layout> {
//         self.layout
//     }

//     fn build_layout_tree(&self) -> rsact_ui::prelude::MemoTree<Layout> {
//         MemoTree::childless(self.layout)
//     }

//     fn draw(
//         &self,
//         ctx: &mut rsact_ui::prelude::DrawCtx<'_, W>,
//     ) -> rsact_ui::prelude::DrawResult {
//         ctx.draw_focus_outline(self.id)?;

//         let style = self.style.get();

//         ctx.renderer.block(Block::from_layout_style(
//             ctx.layout.area,
//             self.layout.get().block_model(),
//             style.container,
//         ))?;

//         // TODO: Font props
//         self.text.with(|text| {
//             ctx.renderer.mono_text(TextBox::with_textbox_style(
//                 &text,
//                 ctx.layout.area,
//                 MonoTextStyle::new(&FONT_10X20,
// W::Color::default_foreground()),
// TextBoxStyleBuilder::new().build(),             ))
//         })
//     }

//     fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse<W> {
//         let current_state = self.state.get();

//         if let (true, Some(focused_char), Some(offset)) = (
//             current_state.char_edit && ctx.is_focused(self.id),
//             current_state.focused_char,
//             ctx.event.as_char_change(),
//         ) {
//             let current_char =
//                 self.chars.with(|chars| A::index(chars[focused_char]));
//             let new_index =
//                 cycle_index(current_char as i64 + offset as i64, A::len());

//             if new_index != current_char {
//                 self.chars.update(|chars| {
//                     let new_char = A::char(new_index);
//                     // let mut char_code_buf =
//                     //     alloc::vec::Vec::with_capacity(char.len_utf8());
//                     // value.replace_range(
//                     //     value
//                     //         .char_indices()
//                     //         .nth(new_index)
//                     //         .map(|(pos, ch)| (pos..pos + ch.len_utf8()))
//                     //         .unwrap(),
//                     //     char.encode_utf8(&mut char_code_buf),
//                     // );
//                     chars[focused_char] = new_char;
//                 });
//             }
//         }

//         if let (true, Some(mut focused_char), Some(offset)) = (
//             ctx.is_focused(self.id) && !current_state.char_edit,
//             current_state.focused_char,
//             ctx.event.as_char_select(),
//         ) {
//             // let new = focused_char
//             //     .or_else(|| {
//             //         if offset > 0 {
//             //             offset -= 1;
//             //             Some(0)
//             //         } else {
//             //             None
//             //         }
//             //     })
//             //     .map(|current| {
//             //         (current as i64 + offset as i64)
//             //             .clamp(0, value.len() as i64)
//             //             as usize
//             //     });

//             let new_focused_char = cycle_index(
//                 focused_char as i64 + offset as i64,
//                 self.chars.with(Vec::len),
//             );

//             if focused_char != new_focused_char {
//                 self.state.update(|state| {
//                     state.focused_char = Some(new_focused_char)
//                 });
//             }

//             return W::capture();
//         }

//         ctx.handle_focusable(self.id, |pressed| {
//             if current_state.pressed != pressed {
//                 let toggle_active = if !current_state.pressed && pressed {
//                     true
//                 } else {
//                     false
//                 };

//                 self.state.update(|state| {
//                     state.pressed = pressed;
//                     if toggle_active {
//                         if state.char_edit {
//                             state.char_edit = false;
//                         }
//                         if let Some(char) = state.active_char {
//                             state.active_char = None;
//                         } else {
//                             state.active = !state.active;
//                         }
//                     }
//                 });

//                 W::capture()
//             } else {
//                 W::ignore()
//             }
//         })
//     }
// }
