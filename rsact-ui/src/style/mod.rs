use crate::render::color::Color;

pub mod accent;
pub mod block;
pub mod cascad;
pub mod primary_gray;
pub mod theme;

/**
 * Prioritized extended Option type for colors.
 * [`ColorStyle::Unset`], [`ColorStyle::LowPriority`] and
 * `ColorStyle::Default*` are low-priority option, which are overwritten by
 * any new non-Unset color. HighPriority and Transparent are only
 * overwritten by new high-priority color.
 * Unset and Default* are the lowest priority
 * variant which cannot be set, and only must be used as initial value.
 */
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorStyle<C: Color> {
    /// Color is unset
    Unset,
    /// Color is set to transparent. High priority
    Transparent,
    /// Color set with low priority
    LowPriority(C),
    /// Color set with high priority
    HighPriority(C),
    /// Color is unset and will fallback to default foreground
    DefaultForeground,
    /// Color is unset and will fallback to default background
    DefaultBackground,
}

impl<C: Color> ColorStyle<C> {
    pub fn get(self) -> Option<C> {
        match self {
            ColorStyle::Unset => None,
            ColorStyle::Transparent => None,
            ColorStyle::LowPriority(color)
            | ColorStyle::HighPriority(color) => Some(color),
            ColorStyle::DefaultForeground => Some(C::default_foreground()),
            ColorStyle::DefaultBackground => Some(C::default_background()),
        }
    }

    pub fn expect(self) -> C {
        self.get().expect("Color must be set at this point")
    }

    pub fn set_low_priority(&mut self, new: Option<C>) {
        if let Some(new) = new {
            match self {
                Self::DefaultBackground
                | Self::DefaultForeground
                | Self::LowPriority(_)
                | Self::Unset => *self = Self::LowPriority(new),
                _ => {},
            }
        }
    }

    pub fn set_high_priority(&mut self, new: Option<C>) {
        match new {
            Some(color) => {
                *self = Self::HighPriority(color);
            },
            None => {
                *self = Self::Transparent;
            },
        }
    }

    pub fn set_transparent(&mut self) {
        *self = Self::Transparent
    }
}

pub trait WidgetStyle: PartialEq + Clone {
    type Color: Color;
    type Inputs;
}

#[derive(Default, PartialEq, Clone, Copy)]
pub struct NullStyler;

impl<S: WidgetStyle> Styler<S> for NullStyler
where
    S: Clone,
{
    type Class = ();

    fn default() -> Self::Class {
        ()
    }

    fn style(
        self,
        _class: Self::Class,
    ) -> impl Fn(S, S::Inputs) -> S + 'static {
        move |style, _| style.clone()
    }
}

// TODO: Refine Styler:
// - Class is unused
pub trait Styler<S: WidgetStyle> {
    type Class;

    fn default() -> Self::Class;
    fn style(self, class: Self::Class) -> impl Fn(S, S::Inputs) -> S + 'static;
}

// impl<S: WidgetStyle, F> Styler<S> for F
// where
//     F: Fn(S, S::Inputs) -> S + 'static,
// {
//     type Class = ();

//     fn default() -> Self::Class {
//         ()
//     }

//     fn style(
//         self,
//         _class: Self::Class,
//     ) -> impl Fn(S, S::Inputs) -> S + 'static {
//         self
//     }
// }

// impl<S: WidgetStyle> Styler<S> for S {}

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
        #[derive($crate::derivative::Derivative)]
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

        impl<C: $crate::render::color::Color> $crate::style::WidgetStyle for $name<C> {
            type Color = C;
            type Inputs = $crate::style::declare_widget_style!(@inputs $($inputs)?);
        }
    };

    (@inputs $inputs: ty) => {
        $inputs
    };
    (@inputs) => {
        ()
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
        $crate::style::ColorStyle<C>
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
        $crate::style::block::BlockStyle<C>
    };

    (@method $field: ident: container $({
        $($opt_method_name: ident: $opt_method_ty: ident),*
        $(,)?
    })?) => {
        pub fn $field(mut self, $field: $crate::style::block::BlockStyle<C>) -> Self {
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
        pub fn $border_radius(mut self, border_radius: impl Into<$crate::style::block::BorderRadius>) -> Self {
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
