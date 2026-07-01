use crate::render::prelude::*;
use alloc::boxed::Box;

pub mod primary_gray;
pub mod stylist;
pub mod theme;

/*
 * TODO: Style possibly can be unified by StyleProp trait, instead of Style,
 * and every property that can be used inside styles should implement it. For
 * example BlockStyle: StyleProp, ColorStyle: StyleProp, Angle: StyleProp. So
 * style becomes a structure built out of itself where everything is
 * generalized. Pros are general implementations, cons are dangerous default
 * values (for example for Knob start angle and sweep angle should not be the
 * same "default value")
 */

pub trait Style {
    fn base() -> Self;
}

// TODO: Bitflags?
pub struct StylePseudoClass {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub active: bool,
}

impl StylePseudoClass {
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

impl Default for StylePseudoClass {
    fn default() -> Self {
        Self { hovered: false, pressed: false, focused: false, active: false }
    }
}

pub struct StyleSelector {
    pub pseudoclass: StylePseudoClass,
}

// TODO: Should there be StyledWidget with exposed fn style() builder method?
pub type WidgetStyleFn<S> = Option<Box<dyn Fn(S, &StyleSelector) -> S>>;

pub trait StyleFn<S: Style + 'static>:
    Fn(S, &StyleSelector) -> S + 'static
{
}

impl<S: Style + 'static, F: Fn(S, &StyleSelector) -> S + 'static> StyleFn<S>
    for F
{
}

#[derive(Debug, Clone, Copy)]
pub struct TreeStyle<C: Color> {
    pub text_color: ColorStyle<C>,
}

impl<C: Color> TreeStyle<C> {
    pub fn base() -> Self {
        Self { text_color: ColorStyle::DefaultForeground }
    }

    pub fn text_color(mut self, text_color: Option<C>) -> Self {
        self.text_color.set_high_priority(text_color);
        self
    }
}

#[macro_export]
macro_rules! declare_widget_style {
    ($name: ident ($($inputs: ty)?) {
        $(
            $field:ident : $ty:ident $({
                $($opt_method_name: ident: $opt_method_ty: ident),*
                $(,)?
            })? $(= $default: expr)?
        ),* $(,)?
    }) => {
        #[derive(derivative::Derivative)]
        #[derivative(Clone, Copy, PartialEq, Debug)]
        pub struct $name<C: $crate::render::color::Color> {
            $(pub $field: $crate::style::declare_widget_style!(@ty $ty)),*
        }

        impl<C: $crate::render::color::Color> $name<C> {
            $(
                $crate::style::declare_widget_style!{
                    @method $field: $ty $({
                        $($opt_method_name: $opt_method_ty),*
                    })?
                }
            )*
        }

        impl<C: $crate::render::color::Color> $crate::style::Style for $name<C> {
            fn base() -> Self {
                Self {
                    $($field: $crate::style::declare_widget_style!(@default $field: $ty $(= $default)?)),*
                }
            }
        }
    };

    // Color //
    (@ty color) => {
        $crate::render::prelude::ColorStyle<C>
    };

    (@default $field: ident: color) => {
        $crate::render::prelude::ColorStyle::Unset
    };

    (@method $field: ident: color $({
        $($opt_method_name: ident: $opt_method_ty: ident),*
        $(,)?
    })?) => {
        pub fn $field(mut self, $field: C) -> Self {
            self.$field.set_high_priority(Some($field));
            self
        }

        $crate::style::declare_widget_style! {
            @opt_method_list $field: color $({
                $($opt_method_name: $opt_method_ty),*
            })?
        }
    };

    (@opt_method_list text_color: color) => {
        $crate::stable::declare_widget_style!(@opt_method_list $field: color {
            transparent_text: transparent,
        })
    };

    // (@opt_method_list background_color: color) => {
    //     $crate::stable::declare_widget_style!(@opt_method_list $field: color {
    //         transparent_background: transparent,
    //     })
    // };

    (@opt_method_list $field: ident: color) => {
        $crate::style::declare_widget_style! {
            @opt_method_list $field: color {
                transparent: transparent,
            }
        }
    };

    (@opt_method $field: ident: color $transparent_method: ident: transparent) => {
        pub fn $transparent_method(mut self) -> Self {
            self.$field.set_transparent();
            self
        }
    };

    // (@opt_method text_color: color) => {
    //     pub fn transparent_text(mut self) -> Self {
    //         self.text_color.set_transparent();
    //         self
    //     }
    // };
    // (@opt_method color: color) => {
    //     pub fn transparent(mut self) -> Self {
    //         self.color.set_transparent();
    //         self
    //     }
    // };
    // (@opt_method background_color: color) => {
    //     pub fn transparent_background(mut self) -> Self {
    //         self.background_color.set_transparent();
    //         self
    //     }
    // };

    // (@ty border) => {
    //     $crate::style::block::BorderStyle<C>
    // };

    // (@method $field: ident: border)

    // BorderStyle //
    (@ty border) => {
        $crate::style::block::BorderStyle<C>
    };

    (@default $field: ident: border) => {
        $crate::style::block::BorderStyle::base()
    };

    (@method $field: ident: border $({
        $($opt_method_name: ident: $opt_method_ty: ident),*
        $(,)?
    })?) => {
        pub fn $field(mut self, border: $crate::style::block::BorderStyle<C>) -> Self {
            self.$field = border;
            self
        }

        $crate::style::declare_widget_style! {
            @opt_method_list $field: border $({
                $($opt_method_name: $opt_method_ty),*
                $(,)?
            })?
        }
    };

    (@opt_method_list border: border) => {
        $crate::style::declare_widget_style! {
            @opt_method_list $field: border {
                border_color: border_color,
                border_radius: border_radius,
            }
        }
    };

    // BlockStyle //
    (@ty container) => {
        $crate::render::prelude::BlockStyle<C>
    };

    (@default $field: ident: container) => {
        $crate::render::prelude::BlockStyle::base()
    };

    (@method $field: ident: container $({
        $($opt_method_name: ident: $opt_method_ty: ident),*
        $(,)?
    })?) => {
        pub fn $field(mut self, $field: $crate::render::prelude::BlockStyle<C>) -> Self {
            self.$field = $field;
            self
        }

        $crate::style::declare_widget_style! {
            @opt_method_list $field: container $({
                $($opt_method_name: $opt_method_ty),*
            })?
        }
    };

    (@opt_method_list container: container) => {
        $crate::style::declare_widget_style! {
            @opt_method_list container: container {
                background_color: background_color,

                border_color: border_color,
                border_radius: border_radius,

                outline_color: outline_color,
                outline_radius: outline_radius,
                outline_offset: outline_offset,
                outline_width: outline_width,
            }
        }
    };

    (@opt_method $field: ident: container $background_color: ident: background_color) => {
        pub fn $background_color(mut self, background_color: C) -> Self {
            self.$field.background_color.set_high_priority(Some(background_color));
            self
        }
    };
    (@opt_method $field: ident: container $border_color: ident: border_color) => {
        pub fn $border_color(mut self, border_color: C) -> Self {
            self.$field.border.color.set_high_priority(Some(border_color));
            self
        }
    };
    (@opt_method $field: ident: container $border_radius: ident: border_radius) => {
        pub fn $border_radius(mut self, border_radius: impl Into<$crate::render::prelude::BorderRadius>) -> Self {
            self.$field.border.radius = border_radius.into();
            self
        }
    };
    (@opt_method $field: ident: container $outline_color: ident: outline_color) => {
        pub fn $outline_color(mut self, outline_color: C) -> Self {
            self.$field.outline.color.set_high_priority(Some(outline_color));
            self
        }
    };
    (@opt_method $field: ident: container $outline_radius: ident: outline_radius) => {
        pub fn $outline_radius(mut self, outline_radius: impl Into<$crate::render::prelude::BorderRadius>) -> Self {
            self.$field.outline.radius = outline_radius.into();
            self
        }
    };
    (@opt_method $field: ident: container $outline_offset: ident: outline_offset) => {
        pub fn $outline_offset(mut self, outline_offset: i32) -> Self {
            self.$field.outline.offset = outline_offset;
            self
        }
    };
    (@opt_method $field: ident: container $outline_width: ident: outline_width) => {
        pub fn $outline_width(mut self, outline_width: u32) -> Self {
            self.$field.outline.width = outline_width;
            self
        }
    };


    // Fallbacks //
    (@ty $ty: ty) => {
        $ty
    };

    (@default $field: ident: $ty: ty) => {
        compile_error!(concat!("Missing default value for '", stringify!($field), "'"))
    };

    (@method $field: ident: $ty: ty) => {
        pub fn $field(mut self, $field: $ty) -> Self {
            self.$field = $field;
            self
        }
    };

    (@default $field: ident: $ty: ident = $default: expr) => {
        $default
    };

    (@opt_method_list $field: ident: $ty: ident $({
        $($opt_method_name: ident: $opt_method_ty: ident),*
        $(,)?
    })?) => {
        $($(
            $crate::style::declare_widget_style! {
                @opt_method $field: $ty $opt_method_name: $opt_method_ty
            }
        )*)?
    };
}

pub use declare_widget_style;
