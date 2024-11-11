use crate::render::color::Color;
use rsact_reactive::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub struct Palette<C: Color> {
    background: C,
    foreground: C,
    primary: C,
    secondary: C,
    accent: C,
}

// FIXME: Wrong PartialEq usage on Signal?
#[derive(Clone, Copy, PartialEq)]
pub struct ThemeStyler<C: ThemeColor + 'static> {
    palette: Signal<Palette<C>>,
}

impl<C: ThemeColor + 'static> Default for ThemeStyler<C> {
    fn default() -> Self {
        Self { palette: Theme::default().palette().signal() }
    }
}

impl<C: ThemeColor + 'static> ThemeStyler<C> {
    pub fn new(theme: Theme<C>) -> Self {
        Self { palette: theme.palette().signal() }
    }

    pub fn set_theme(&mut self, theme: Theme<C>) {
        self.palette.set(theme.palette());
    }
}

// impl<C: ThemeColor + 'static> Styler<ButtonStyle<C>> for ThemeStyler<C> {
//     type Class = ();

//     fn default() -> Self::Class {
//         // TODO
//         ()
//     }

//     // TODO
//     fn style(
//         self,
//         _inputs: Self::Class,
//     ) -> impl Fn(
//         ButtonStyle<C>,
//         <ButtonStyle<C> as super::WidgetStyle>::Inputs,
//     ) -> ButtonStyle<C>
//            + 'static {
//         move |mut prev_style, _state| {
//             self.palette.with(|palette| {
//                 // match state {}
//                 prev_style.container.background_color =
//                     Some(palette.background);
//                 prev_style
//             })
//         }
//     }
// }

pub struct CustomTheme<C: Color> {
    palette: Palette<C>,
}

#[derive(Default)]
pub enum Theme<C: ThemeColor> {
    #[default]
    Light,
    Dark,
    Custom(CustomTheme<C>),
}

impl<C: ThemeColor> Theme<C> {
    pub fn palette(&self) -> Palette<C> {
        match self {
            Theme::Light => C::LIGHT,
            Theme::Dark => C::DARK,
            Theme::Custom(custom) => custom.palette,
        }
    }
}

pub trait ThemeColor: Color {
    const LIGHT: Palette<Self>;
    const DARK: Palette<Self>;
}

// macro_rules! impl_rgb_theme_color {
//     ($($colors: path),* {
//         $($theme: ident = $palette: expr);*
//         $(;)?
//     }) => {
//         $(
//             impl ThemeColor for $colors {
//                 $(const $theme = $palette);*
//             }
//         )*
//     };
// }

// impl_rgb_theme_color! {
//     Rgb888 {
//         LIGHT = Palette {
//             background: todo!(),
//             foreground: todo!(),
//             primary: todo!(),
//             secondary: todo!(),
//             accent: todo!(),
//         };

//         DARK = Palette {
//             background: todo!(),
//             foreground: todo!(),
//             primary: todo!(),
//             secondary: todo!(),
//             accent: todo!(),
//         };
//     }
// }
