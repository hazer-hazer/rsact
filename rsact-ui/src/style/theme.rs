use core::default;

use super::Styler;
use crate::{render::color::Color, widgets::button::ButtonStyle};
use rsact_core::prelude::*;

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
pub struct ThemeStyler<C: PaletteColor + 'static> {
    palette: Signal<Palette<C>>,
}

impl<C: PaletteColor + 'static> Default for ThemeStyler<C> {
    fn default() -> Self {
        Self { palette: Theme::default().palette().into_signal() }
    }
}

impl<C: PaletteColor + 'static> ThemeStyler<C> {
    pub fn new(theme: Theme<C>) -> Self {
        Self { palette: theme.palette().into_signal() }
    }

    pub fn set_theme(&self, theme: Theme<C>) {
        self.palette.set(theme.palette());
    }
}

impl<C: PaletteColor + 'static> Styler<ButtonStyle<C>> for ThemeStyler<C> {
    type Class = ();

    fn default() -> Self::Class {
        // TODO
        ()
    }

    // TODO
    fn style(
        self,
        inputs: Self::Class,
    ) -> impl Fn(
        ButtonStyle<C>,
        <ButtonStyle<C> as super::WidgetStyle>::Inputs,
    ) -> ButtonStyle<C>
           + 'static {
        move |mut prev_style, state| {
            self.palette.with(|palette| {
                // match state {}
                prev_style.container.background_color =
                    Some(palette.background);
                prev_style
            })
        }
    }
}

pub struct CustomTheme<C: Color> {
    palette: Palette<C>,
}

#[derive(Default)]
pub enum Theme<C: PaletteColor> {
    #[default]
    Light,
    Dark,
    Custom(CustomTheme<C>),
}

impl<C: PaletteColor> Theme<C> {
    pub fn palette(&self) -> Palette<C> {
        match self {
            Theme::Light => C::LIGHT,
            Theme::Dark => C::DARK,
            Theme::Custom(custom) => custom.palette,
        }
    }
}

pub trait PaletteColor: Color {
    const LIGHT: Palette<Self>;
    const DARK: Palette<Self>;
}

macro_rules! impl_rgb_palette_color {
    ($($colors: path),* {
        $($theme: ident = $palette: expr);*
        $(;)?
    }) => {
        $(
            impl PaletteColor for $colors {
                $(const $theme = $palette)*
            }
        )*
    };
}
