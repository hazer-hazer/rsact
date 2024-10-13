macro_rules! icon_set {
    (@icon $set: ident $icon_kind: ident: $filename: literal) => {{
        let icon = crate::Icon {
            source_filename: $filename,
            data: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/icon-libs/material-design/svg/", $filename, ".svg")),
            name: stringify!($icon_kind),
            kind: $set::$icon_kind,
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

    // (@inner $filename: ident) => {
    //     icons!(@inner $filename: &stringify!($filename).to_case(convert_case::Case::Camel))
    // };

    // (@filename $filename: ident) => {
    //     stringify!($filename)
    // };

    // (@filename $filename: literal) => {
    //     $filename
    // };

    ($set: ident $alpha_cutoff: literal [
        $($sizes: literal $({
            $($size_mod: ident: $size_mod_value: expr),*
            $(,)?
        })?),* $(,)?
    ] {
        $(
            $icon_kind: ident: $filename: literal
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
            fn name() -> &'static str {
                stringify!($set)
            }

            fn sizes() -> &'static [u32] {
                const SIZES: &[u32] = &[$($sizes),*];
                SIZES
            }

            fn icons() -> &'static [crate::Icon<Self>] {
                const ICONS: &[crate::Icon<$set>] = &[
                    $(icon_set!(@icon $set $icon_kind: $filename)),*
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
