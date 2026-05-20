use crate::render::prelude::*;
use alloc::boxed::Box;

pub mod primary_gray;
pub mod theme;

pub type WidgetStyleFn<S> = Option<Box<dyn Fn(S) -> S>>;

#[derive(Clone, Copy)]
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

pub trait TreeStyled<C: Color>: Sized {
    fn with_tree(self, tree: TreeStyle<C>) -> Self;
}

#[macro_export]
macro_rules! declare_widget_style {
    ($name: ident ($($inputs: ty)?) {
        $(
            $field:ident : $ty:ident $({
                $($opt_method_name: ident: $opt_method_ty: ident),*
                $(,)?
            })?
        ),* $(,)?
    }) => {
        #[derive(derivative::Derivative)]
        #[derivative(Clone, Copy, PartialEq)]
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

    // Color //
    (@ty color) => {
        $crate::render::prelude::ColorStyle<C>
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
        $crate::style::declare_widget_style!(@opt_method_list $field: container {
            background_color: background_color,
            border_color: border_color,
            border_radius: border_radius
        })
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

    (@ty $ty: ty) => {
        $ty
    };

    (@method $field: ident: $ty: ty) => {
        pub fn $field(mut self, $field: $ty) -> Self {
            self.$field = $field;
            self
        }
    };
}

pub use declare_widget_style;
