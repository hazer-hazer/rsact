// #![cfg_attr(not(feature = "std"), no_std)]

use proc_macro::TokenStream;
use syn::{DeriveInput, Result, parse_macro_input};

extern crate proc_macro2;
extern crate syn;
#[macro_use]
extern crate quote;

// TODO: Split into rsact-reactive-macros and rsact-ui-macros crates, or at least split into separate modules.

fn impl_into_maybe_reactive(ast: &DeriveInput) -> Result<TokenStream> {
    // match &ast.data {
    //     syn::Data::Enum(_) | syn::Data::Struct(_) => Ok(()),
    //     _ => Err(Error::new(
    //         ast.span(),
    //         "MaybeReactive should only be used on struct's or enum's",
    //     )),
    // }?;

    let name = &ast.ident;
    let (impl_gen, type_gen, where_clause) = ast.generics.split_for_impl();

    let result = quote! {
        impl #impl_gen rsact_reactive::prelude::IntoMaybeReactive<#name #type_gen> for #name #type_gen #where_clause {
            fn maybe_reactive(self) -> rsact_reactive::prelude::MaybeReactive<#name #type_gen> {
                rsact_reactive::prelude::MaybeReactive::new_inert(self)
            }
        }
    }.into();

    Ok(result)
}

#[proc_macro_derive(IntoMaybeReactive)]
pub fn into_maybe_reactive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    impl_into_maybe_reactive(&input)
        .unwrap_or_else(|err| err.to_compile_error().into())
        .into()
}

#[proc_macro_derive(View)]
pub fn view(input: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(input as DeriveInput);

    // `Widget::el` requires `Self: 'static`, so every generic type parameter
    // of the widget must be `'static`. Widgets generic over `Dir`/`V`/`K`/`I`
    // would otherwise fail the bound the hand-written impls carried explicitly.
    for param in input.generics.type_params_mut() {
        param.bounds.push(syn::parse_quote!('static));
    }

    // 7.7: find the WidgetCtx type param by its `WidgetCtx` bound; fall back to
    // a param literally named `W`, else the first type param.
    let wctx: syn::Ident = input
        .generics
        .type_params()
        .find(|p| {
            p.bounds.iter().any(|b| matches!(
                b, syn::TypeParamBound::Trait(t)
                    if t.path.segments.last().map_or(false, |s| s.ident == "WidgetCtx")
            ))
        })
        .or_else(|| input.generics.type_params().find(|p| p.ident == "W"))
        .or_else(|| input.generics.type_params().next())
        .map(|p| p.ident.clone())
        .unwrap_or_else(|| syn::Ident::new("W", proc_macro2::Span::call_site()));

    let name = &input.ident;
    let (impl_gen, type_gen, where_clause) = input.generics.split_for_impl();

    let result = quote! {
        impl #impl_gen rsact_ui::el::View<#wctx> for #name #type_gen #where_clause {
            fn into_el(self) -> rsact_ui::el::El<#wctx> {
                rsact_ui::el::El::new(self)
            }
        }

        impl #impl_gen rsact_ui::el::SingleViewMarker for #name #type_gen #where_clause {}

        // Identity `Build`: an unsplit widget is its own builder — run its
        // in-place `Widget::build`, then hand back `self` as the retained widget.
        impl #impl_gen rsact_ui::el::build::Build<#wctx> for #name #type_gen #where_clause {
            fn build(
                mut self: ::alloc::boxed::Box<Self>,
                ctx: rsact_ui::el::build::BuildCtx<#wctx>,
            ) -> ::alloc::boxed::Box<dyn rsact_ui::widget::Widget<#wctx>> {
                rsact_ui::widget::Widget::build(&mut *self, ctx);
                self
            }
            fn layout(&self) -> rsact_ui::layout::node::Layout {
                rsact_ui::widget::Widget::layout(self)
            }
            fn flags(&self) -> rsact_ui::el::WidgetFlags {
                rsact_ui::widget::Widget::flags(self)
            }
            fn debug_name(&self) -> &'static str {
                rsact_ui::widget::Widget::debug_name(self)
            }
        }
    }
    .into();

    result
}
