use proc_macro2::TokenStream;
use syn::{spanned::Spanned as _, DeriveInput, Error, Result};

pub(crate) fn impl_into_maybe_reactive(
    ast: &DeriveInput,
) -> Result<TokenStream> {
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
        impl #impl_gen From<#name #type_gen> for  rsact_reactive::maybe::MaybeReactive<#name #type_gen> #where_clause {
            fn from(value: #name #type_gen) -> Self {
                Self::new_inert(value)
            }
        }
    }.into();

    Ok(result)
}
