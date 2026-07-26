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
                rsact_ui::el::El::New(rsact_ui::el::ElData::new(::alloc::boxed::Box::new(self)))
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
            fn layout_data(&self) -> rsact_ui::layout::LayoutData {
                rsact_ui::widget::Widget::layout_data(self)
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
/// - `#[layout(delegate = "field")]` (struct, optional) — `Build::layout`
///   returns `self.field.layout()` instead of the default `self.layout`
///   (for `Show`-style wrappers that own no layout of their own).
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
/// widget is constructed. `Build::layout` returns the field named `layout`,
/// unless `#[layout(delegate = "field")]` overrides it.
///
/// Note: the RETAINED widget must not override `Widget::flags`/`debug_name` —
/// they are read once, pre-build, from `Build` (seeding `ElState`); a retained
/// override is silently dead. Declare them here (`#[flags]`/`#[builds]`).
#[proc_macro_derive(
    Builder,
    attributes(builds, widget, child, children, flags, layout)
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

    // --- M2: #[layout(delegate = "field")] on the struct (optional). ---
    // Default: return the builder's own `layout` field's owned `LayoutData`
    // (WS5.1: the builder's `layout` is a `LayoutBuilder`, `.data()` is its
    // owned `LayoutData`). Delegate: return a child field's `layout_data()`
    // (`Show`-style wrappers, spec §5.4).
    let mut is_delegate = false;
    let layout_body =
        match input.attrs.iter().find(|a| a.path().is_ident("layout")) {
            Some(attr) => {
                is_delegate = true;
                let nv: syn::MetaNameValue = attr.parse_args()?;
                if !nv.path.is_ident("delegate") {
                    return Err(syn::Error::new_spanned(
                        &nv,
                        "expected #[layout(delegate = \"field\")]",
                    ));
                }
                let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(name),
                    ..
                }) = &nv.value
                else {
                    return Err(syn::Error::new_spanned(
                        &nv.value,
                        "expected a string literal naming a builder field",
                    ));
                };
                let field_ident = syn::Ident::new(&name.value(), name.span());
                if !fields
                    .iter()
                    .any(|f| f.ident.as_ref() == Some(&field_ident))
                {
                    return Err(syn::Error::new_spanned(
                        name,
                        format!(
                            "no field named `{}` on this builder",
                            name.value()
                        ),
                    ));
                }
                quote! { self.#field_ident.layout_data() }
            },
            None => {
                if !fields
                    .iter()
                    .any(|f| f.ident.as_ref().is_some_and(|i| i == "layout"))
                {
                    return Err(syn::Error::new_spanned(
                        &input.ident,
                        "builder has no `layout` field; add one or use \
                     #[layout(delegate = \"field\")] to return a child's \
                     layout",
                    ));
                }
                quote! { self.layout.data().clone() }
            },
        };

    let mut child_wiring = Vec::new();
    let mut widget_ctor_fields = Vec::new();
    for field in fields {
        // Named-fields only (checked above), so `ident` is always present.
        let ident = field.ident.as_ref().unwrap();

        // M1: the three roles are mutually exclusive. A double-annotated field
        // would be silently double-processed (child-wired AND husk-moved). A
        // widget that genuinely needs to wire a child and keep its husk must
        // hand-write its `Build` impl (per-type coexistence is by design).
        let has_widget =
            field.attrs.iter().any(|a| a.path().is_ident("widget"));
        let child_attr =
            field.attrs.iter().find(|a| a.path().is_ident("child"));
        let children_attr =
            field.attrs.iter().find(|a| a.path().is_ident("children"));
        let roles = usize::from(has_widget)
            + usize::from(child_attr.is_some())
            + usize::from(children_attr.is_some());
        if roles > 1 {
            return Err(syn::Error::new_spanned(
                field,
                "a builder field can carry only one of #[widget], \
                 #[child(...)], #[children(...)]; child fields are build-only \
                 (the arena owns the child) — to wire a child AND keep its \
                 husk in the retained widget, hand-write the Build impl",
            ));
        }

        if has_widget {
            // WS5.1: the builder's `layout` field is a `LayoutBuilder`; the
            // retained widget holds an owned `LayoutData`, so convert it via
            // `.into_data()` (AFTER its reactive bindings are drained below).
            // Delegate builders own no `layout` `LayoutBuilder` field, so their
            // `#[widget]` fields (if any) move by name unchanged.
            if !is_delegate && ident == "layout" {
                widget_ctor_fields
                    .push(quote! { #ident: this.#ident.into_data() });
            } else {
                widget_ctor_fields.push(quote! { #ident: this.#ident });
            }
        }

        if let Some(attr) = child_attr {
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

        if let Some(attr) = children_attr {
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

    // WS5.1: a non-delegate builder owns a `layout: LayoutBuilder` whose
    // reactive-prop bindings must be drained + wired through `ctx` at build
    // (`bind_layout`), so `this`/`ctx` are always used there. A delegate
    // builder has no layout bindings, so it needs them only if it has child
    // wiring. Bind `mut`/`_` accordingly so both stay warning-clean.
    let has_wiring = !child_wiring.is_empty();
    let drains_layout = !is_delegate;
    let uses_ctx = has_wiring || drains_layout;
    let this_binding = if uses_ctx {
        quote! { let mut this = *self; }
    } else {
        quote! { let this = *self; }
    };
    let ctx_arg = if uses_ctx {
        quote! { mut ctx: rsact_ui::el::build::BuildCtx<#wctx> }
    } else {
        quote! { _ctx: rsact_ui::el::build::BuildCtx<#wctx> }
    };
    // Drain the builder's reactive layout-prop bindings and wire each through
    // `ctx.bind_layout` (it has `ctx.id`). Empty for a fully-static layout.
    let layout_drain = if drains_layout {
        quote! {
            for __binding in this.layout.take_bindings() {
                __binding(&mut ctx);
            }
        }
    } else {
        quote! {}
    };
    // Non-delegate builders own a settable `layout` `LayoutBuilder`, so
    // `set_show` writes its `show`. Delegate/identity builders use the trait
    // default no-op.
    let set_show_method = if drains_layout {
        quote! {
            fn set_show(&mut self, show: rsact_reactive::prelude::Memo<bool>) {
                self.layout.show(show);
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        impl #impl_gen rsact_ui::el::View<#wctx> for #name #type_gen #where_clause {
            fn into_el(self) -> rsact_ui::el::El<#wctx> {
                rsact_ui::el::El::New(rsact_ui::el::ElData::new(::alloc::boxed::Box::new(self)))
            }
        }

        impl #impl_gen rsact_ui::el::SingleViewMarker for #name #type_gen #where_clause {}

        impl #impl_gen rsact_ui::el::build::Build<#wctx> for #name #type_gen #where_clause {
            fn build(
                self: ::alloc::boxed::Box<Self>,
                #ctx_arg,
            ) -> ::alloc::boxed::Box<dyn rsact_ui::widget::Widget<#wctx>> {
                #this_binding
                #layout_drain
                #( #child_wiring )*
                ::alloc::boxed::Box::new(#widget_ident {
                    #( #widget_ctor_fields ),*
                })
            }

            fn layout_data(&self) -> rsact_ui::layout::LayoutData {
                #layout_body
            }

            #set_show_method

            #flags_method

            fn debug_name(&self) -> &'static str {
                #debug_name
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::impl_builder;
    use syn::parse_quote;

    fn derive(
        mut input: syn::DeriveInput,
    ) -> syn::Result<proc_macro2::TokenStream> {
        impl_builder(&mut input)
    }

    /// M1: a field must carry at most ONE of #[widget]/#[child]/#[children] —
    /// double-annotation would silently wire the child AND move the husk.
    #[test]
    fn widget_plus_child_on_one_field_is_an_error() {
        let err = derive(parse_quote! {
            #[builds(Button<W>)]
            struct ButtonBuilder<W: WidgetCtx> {
                layout: Layout,
                #[widget]
                #[child(single)]
                content: El<W>,
            }
        })
        .unwrap_err();
        assert!(err.to_string().contains("only one of"), "got: {err}");
    }

    #[test]
    fn child_plus_children_on_one_field_is_an_error() {
        let err = derive(parse_quote! {
            #[builds(Flex<W>)]
            struct FlexBuilder<W: WidgetCtx> {
                layout: Layout,
                #[child(single)]
                #[children(reactive)]
                children: MaybeSignal<Vec<El<W>>>,
            }
        })
        .unwrap_err();
        assert!(err.to_string().contains("only one of"), "got: {err}");
    }

    /// Regression: single-role fields keep deriving fine.
    #[test]
    fn single_role_fields_derive_ok() {
        let ts = derive(parse_quote! {
            #[builds(Button<W>)]
            struct ButtonBuilder<W: WidgetCtx> {
                #[widget]
                layout: Layout,
                #[child(single)]
                content: El<W>,
            }
        })
        .unwrap();
        let flat = ts.to_string().replace(' ', "");
        assert!(flat.contains("self.layout"), "default layout body: {flat}");
    }

    /// M2: #[layout(delegate = "el")] emits `self.el.layout()` (spec §5.4).
    #[test]
    fn layout_delegate_emits_field_layout_call() {
        let ts = derive(parse_quote! {
            #[builds(Show<W>)]
            #[layout(delegate = "el")]
            struct ShowBuilder<W: WidgetCtx> {
                #[child(single)]
                el: El<W>,
            }
        })
        .unwrap();
        let flat = ts.to_string().replace(' ', "");
        assert!(
            flat.contains("self.el.layout_data()"),
            "delegate body: {flat}"
        );
    }

    #[test]
    fn layout_delegate_to_missing_field_is_an_error() {
        let err = derive(parse_quote! {
            #[builds(Show<W>)]
            #[layout(delegate = "missing")]
            struct ShowBuilder<W: WidgetCtx> {
                #[child(single)]
                el: El<W>,
            }
        })
        .unwrap_err();
        assert!(err.to_string().contains("no field named"), "got: {err}");
    }

    /// M2: no `layout` field and no delegate → a spanned, actionable error
    /// (was: confusing downstream E0609 on the emitted `self.layout`).
    #[test]
    fn missing_layout_field_without_delegate_is_an_error() {
        let err = derive(parse_quote! {
            #[builds(Show<W>)]
            struct ShowBuilder<W: WidgetCtx> {
                #[child(single)]
                el: El<W>,
            }
        })
        .unwrap_err();
        assert!(err.to_string().contains("no `layout` field"), "got: {err}");
    }
}
