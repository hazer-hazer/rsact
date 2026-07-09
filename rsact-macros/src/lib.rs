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

/// 7.7 ctx-param detection, shared by `#[derive(View)]` and `#[derive(Builder)]`.
/// Find the type param bounded by `WidgetCtx`; fall back to a param literally
/// named `W`, else the first type param, else the literal `W`. Naming it
/// explicitly (rather than assuming a param literally `W`) lets widgets whose
/// ctx param is named differently still derive correctly.
fn ctx_param(generics: &syn::Generics) -> syn::Ident {
    generics
        .type_params()
        .find(|p| {
            p.bounds.iter().any(|b| {
                matches!(
                    b, syn::TypeParamBound::Trait(t)
                        if t.path.segments.last().map_or(false, |s| s.ident == "WidgetCtx")
                )
            })
        })
        .or_else(|| generics.type_params().find(|p| p.ident == "W"))
        .or_else(|| generics.type_params().next())
        .map(|p| p.ident.clone())
        .unwrap_or_else(|| syn::Ident::new("W", proc_macro2::Span::call_site()))
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
    let wctx = ctx_param(&input.generics);

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

/// `#[derive(Builder)]` — for a transient split builder (WS13 spec §3.2). Emits
/// the same `View` + `SingleViewMarker` + `Build` trio the hand-written Button /
/// Flex builders carried, generated from field/struct attributes:
///
/// - `#[builds(Widget<...>)]` (struct, required) — the retained widget type this
///   builder consumes into. Its ident seeds `debug_name` and the ctor.
/// - `#[flags(a, b, ...)]` (struct, optional) — `Build::flags` returns
///   `WidgetFlags::default().a().b()...`; absent → the trait default.
/// - `#[widget]` (field) — moved BY NAME into the retained widget struct literal
///   (`Widget { field: this.field, ... }`). The builder must declare EVERY field
///   the widget has, each `#[widget]`, so the transform is a pure by-name move
///   (no `Default`/FRU/phantom magic); ZST markers like `PhantomData<W>` are
///   moved too.
/// - `#[child(single)]` (field) — `ctx.set_single_child(&mut this.field)`.
/// - `#[children(reactive)]` (field, a `MaybeSignal<Vec<El<W>>>`) —
///   `this.field.maybe_effect(move |children, _| { ctx.set_children(children); })`.
///
/// Child-wiring statements run in field declaration order, then the retained
/// widget is constructed. `Build::layout` returns the field named `layout`.
#[proc_macro_derive(
    Builder,
    attributes(builds, widget, child, children, flags)
)]
pub fn builder(input: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(input as DeriveInput);
    impl_builder(&mut input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn impl_builder(input: &mut DeriveInput) -> Result<proc_macro2::TokenStream> {
    // --- #[builds(Ty)] — required: the retained widget type. ---
    let builds_ty: syn::Type =
        match input.attrs.iter().find(|a| a.path().is_ident("builds")) {
            Some(attr) => attr.parse_args()?,
            None => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "#[derive(Builder)] requires a #[builds(Widget<...>)] \
                     attribute naming the retained widget type",
                ));
            },
        };
    let widget_ident = match &builds_ty {
        syn::Type::Path(tp) => tp.path.segments.last().map(|s| s.ident.clone()),
        _ => None,
    }
    .ok_or_else(|| {
        syn::Error::new_spanned(
            &builds_ty,
            "#[builds(...)] must be a path type, e.g. Button<W>",
        )
    })?;
    let debug_name =
        syn::LitStr::new(&widget_ident.to_string(), widget_ident.span());

    // --- #[flags(a, b, ...)] on the struct (optional). ---
    let flags_method =
        match input.attrs.iter().find(|a| a.path().is_ident("flags")) {
            Some(attr) => {
                let parsed: syn::punctuated::Punctuated<
                    syn::Ident,
                    syn::Token![,],
                > = attr.parse_args_with(
                    syn::punctuated::Punctuated::parse_terminated,
                )?;
                let methods: Vec<syn::Ident> = parsed.into_iter().collect();
                quote! {
                    fn flags(&self) -> rsact_ui::el::WidgetFlags {
                        rsact_ui::el::WidgetFlags::default() #( .#methods() )*
                    }
                }
            },
            None => quote! {},
        };

    // --- Fields: partition into child-wiring + widget-ctor moves. ---
    let fields = match &input.data {
        syn::Data::Struct(s) => match &s.fields {
            syn::Fields::Named(named) => &named.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "#[derive(Builder)] requires a struct with named fields",
                ));
            },
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "#[derive(Builder)] can only be applied to structs",
            ));
        },
    };

    let mut child_wiring = Vec::new();
    let mut widget_ctor_fields = Vec::new();
    for field in fields {
        // Named-fields only (checked above), so `ident` is always present.
        let ident = field.ident.as_ref().unwrap();

        if field.attrs.iter().any(|a| a.path().is_ident("widget")) {
            widget_ctor_fields.push(quote! { #ident: this.#ident });
        }

        if let Some(attr) =
            field.attrs.iter().find(|a| a.path().is_ident("child"))
        {
            let mode: syn::Ident = attr.parse_args()?;
            if mode == "single" {
                child_wiring.push(quote! {
                    ctx.set_single_child(&mut this.#ident);
                });
            } else {
                return Err(syn::Error::new_spanned(
                    &mode,
                    "unknown #[child(...)] mode; expected `single`",
                ));
            }
        }

        if let Some(attr) =
            field.attrs.iter().find(|a| a.path().is_ident("children"))
        {
            let mode: syn::Ident = attr.parse_args()?;
            if mode == "reactive" {
                child_wiring.push(quote! {
                    this.#ident.maybe_effect(move |children, _| {
                        ctx.set_children(children);
                    });
                });
            } else {
                return Err(syn::Error::new_spanned(
                    &mode,
                    "unknown #[children(...)] mode; expected `reactive`",
                ));
            }
        }
    }

    // 7.7: name the WidgetCtx param, and push `'static` onto every type param
    // (matching the hand-written `impl<W: WidgetCtx + 'static>` bounds).
    for param in input.generics.type_params_mut() {
        param.bounds.push(syn::parse_quote!('static));
    }
    let wctx = ctx_param(&input.generics);

    let name = &input.ident;
    let (impl_gen, type_gen, where_clause) = input.generics.split_for_impl();

    // With no child wiring, `this`/`ctx` would go unused — bind them so the
    // (currently two-widget) prototype and any wiring-free future builder both
    // stay warning-clean.
    let has_wiring = !child_wiring.is_empty();
    let this_binding = if has_wiring {
        quote! { let mut this = *self; }
    } else {
        quote! { let this = *self; }
    };
    let ctx_arg = if has_wiring {
        quote! { mut ctx: rsact_ui::el::build::BuildCtx<#wctx> }
    } else {
        quote! { _ctx: rsact_ui::el::build::BuildCtx<#wctx> }
    };

    Ok(quote! {
        impl #impl_gen rsact_ui::el::View<#wctx> for #name #type_gen #where_clause {
            fn into_el(self) -> rsact_ui::el::El<#wctx> {
                rsact_ui::el::El::new(self)
            }
        }

        impl #impl_gen rsact_ui::el::SingleViewMarker for #name #type_gen #where_clause {}

        impl #impl_gen rsact_ui::el::build::Build<#wctx> for #name #type_gen #where_clause {
            fn build(
                self: ::alloc::boxed::Box<Self>,
                #ctx_arg,
            ) -> ::alloc::boxed::Box<dyn rsact_ui::widget::Widget<#wctx>> {
                #this_binding
                #( #child_wiring )*
                ::alloc::boxed::Box::new(#widget_ident {
                    #( #widget_ctor_fields ),*
                })
            }

            fn layout(&self) -> rsact_ui::layout::node::Layout {
                self.layout
            }

            #flags_method

            fn debug_name(&self) -> &'static str {
                #debug_name
            }
        }
    })
}
