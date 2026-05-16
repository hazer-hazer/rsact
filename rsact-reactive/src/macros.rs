// macro_rules! define_impl_for_common_types {
//     (
//         impl $trait:ident $(<$($generics: tt)>*)? for $target:ty {
//             $(
//                 fn $method:ident(&self $(, $arg_name:ident : $arg_type:ty)*) -> $output:ty;
//             )*
//         }
//     ) => {
//         impl $trait<>
//     };
// }
