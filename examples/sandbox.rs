// use rsact::observe;

// // What I want
// fn main() {
//     let list = reactive::<Vec<i32>>();
//     let current_value = reactive::<i32>();

//     let ui = row![
//         col![button("Remove").on_click(|| {
//             list.pop();
//         })],
//         col![
//             button("-").on_click(|| current_value -= 1),
//             text(|| current_value)
//             button("+").on_click(|| current_value += 1)
//         ],
//         col![button("Add").on_click(|| {
//             list.write().push(current_value);
//             current_value = 0;
//         }),],
//     ];

//     // Tick UI event loop
// }

fn main() {}
