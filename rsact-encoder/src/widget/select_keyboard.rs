use alloc::string::String;
use rsact_ui::prelude::*;

// #[derive(Debug, Clone, Copy, PartialEq)]
// pub struct SelectKeyboardState {
//     pressed: bool,
//     active: bool,
// }

// declare_widget_style! {
//     SelectKeyboardStyle (SelectKeyboardState) {
//         container: container,
//     }
// }

// /**
//  * Keyboard for use with encoder to input text.
//  */
// pub struct SelectKeyboard<W: WidgetCtx> {
//     id: ElId,
//     layout: Signal<Layout>,
//     state: Signal<SelectKeyboardState>,
//     style: MemoChain<SelectKeyboardStyle<W::Color>>,
//     value: Signal<String>,
// }

// impl<W: WidgetCtx> Widget<W> for SelectKeyboard<W>
// where
//     W::Styler: Styler<SelectKeyboardStyle<W::Color>, Class = ()>,
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

//     fn build_layout_tree(&self) -> MemoTree<Layout> {
//         //
//     }

//     fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
//         todo!()
//     }

//     fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse<W> {
//         todo!()
//     }
// }

// pub trait Alphabet {
//     // fn buttons<W: WidgetCtx>() -> impl Iterator<Item = (char, Button<W>)>;
//     fn buttons() -> impl Iterator<Item = &'static str>;
// }

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
