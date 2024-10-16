use rsact_ui::{prelude::SelectOption, widget::prelude::*};

// pub struct DropDown<W: WidgetCtx, K: PartialEq> {
//     id: ElId,
//     layout: Signal<Layout>,
//     list: Memo<Vec<SelectOption<W, K>>>,
//     selected: Signal<Option<usize>>,
// }

// impl<W: WidgetCtx, K: PartialEq> DropDown<W, K> {
//     pub fn new(list: impl IntoMemo<Vec<SelectOption<W, K>>>) -> Self {
//         Self {
//             id: ElId::unique(),
//             layout: Layout::shrink(LayoutKind::Container(ContainerLayout {
//                 block_model: (),
//                 horizontal_align: (),
//                 vertical_align: (),
//                 content_size: (),
//             })),
//             list,
//             selected,
//         }
//     }
// }
