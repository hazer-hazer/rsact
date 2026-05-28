macro_rules! icon_set {
    (@icon $set: ident $icon_kind: ident: $filename: literal ($($aliases: ident),*)) => {{
        let icon = crate::Icon {
            source_filename: $filename,
            // From https://github.com/Templarian/MaterialDesign-SVG
            data: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/icon-libs/material-design/", $filename, ".svg")),
            name: stringify!($icon_kind),
            kind: $set::$icon_kind,
            aliases: &[$(stringify!($aliases)),*]
        };

        icon
    }};

    (@modifiers $modifiers: expr; $($modifier: ident: $value: expr),*) => {{
        let modifiers = $modifiers;

        $(
            let modifiers = modifiers.$modifier($value);
        )*

        modifiers
    }};

    ($set: ident $set_mod_name: literal $alpha_cutoff: literal [
        $($sizes: literal $({
            $($size_mod: ident: $size_mod_value: expr),*
            $(,)?
        })?),* $(,)?
    ] {
        $(
            $icon_kind: ident: $filename: literal
            $((
                $($aliases: ident),* $(,)?
            ))?
            $({
                $($modifier: ident: $modifier_value: expr),*
                $(,)?
            })?
            $([
                $($icon_mod_size: literal {
                    $($icon_size_mod: ident: $icon_size_mod_value: expr),*
                    $(,)?
                }),*
                $(,)?
            ])?
        ),* $(,)?
    }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub enum $set {
            $($icon_kind),*
        }

        impl crate::IconSet for $set {
            fn mod_name() -> &'static str {
                $set_mod_name
            }

            fn ident() -> syn::Ident {
                quote::format_ident!("{}", stringify!($set))
            }

            fn sizes() -> &'static [u32] {
                const SIZES: &[u32] = &[$(
                    $sizes
                ),*];
                SIZES
            }

            fn icons() -> &'static [crate::Icon<Self>] {
                const ICONS: &[crate::Icon<$set>] = &[
                    $(icon_set!(@icon $set $icon_kind: $filename ($($($aliases),*)?))),*
                ];

                ICONS
            }

            fn modifiers(size: u32, kind: Self) -> crate::Modifiers {
                let modifiers = crate::Modifiers::base($alpha_cutoff);

                let modifiers = match size {
                    $(
                        $sizes => icon_set!(@modifiers modifiers; $($($size_mod: $size_mod_value),*)?),
                    )*
                    _ => modifiers,
                };

                match kind {
                    $(Self::$icon_kind => {
                        let modifiers = icon_set!(@modifiers modifiers; $($($modifier: $modifier_value),*)?);

                        match size {
                            $($(
                                $icon_mod_size => icon_set!(@modifiers modifiers; $($icon_size_mod: $icon_size_mod_value),*),
                            )*)?
                            _ => modifiers
                        }
                    }),*
                }
            }
        }
    };
}

pub(crate) use icon_set;
