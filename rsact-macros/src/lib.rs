#![cfg_attr(not(feature = "std"), no_std)]

use maybe_reactive::impl_into_maybe_reactive;
use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

extern crate proc_macro2;
extern crate syn;
#[macro_use]
extern crate quote;

mod maybe_reactive;

#[proc_macro_derive(IntoMaybeReactive)]
pub fn into_maybe_reactive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    impl_into_maybe_reactive(&input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
