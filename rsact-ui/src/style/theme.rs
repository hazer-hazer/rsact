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

#[derive(Clone, Copy)]
pub struct ThemeStyler<C: Color + 'static> {
    palette: Signal<Palette<C>>,
}

impl<C: Color + 'static> Default for ThemeStyler<C> {
    fn default() -> Self {
        Self { palette: Theme::default().palette().into_signal() }
    }
}

impl<C: Color + 'static> ThemeStyler<C> {
    pub fn new(theme: Theme) -> Self {
        Self { palette: theme.palette().into_signal() }
    }

    pub fn set_theme(&self, theme: Theme) {
        self.palette.set(theme.palette());
    }
}

impl<C: Color + 'static> Styler<ButtonStyle<C>> for ThemeStyler<C> {
    type Class = ();

    fn default() -> Self::Class {
        // TODO
        ()
    }

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

#[derive(Default)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

impl Theme {
    pub fn palette<C: Color>(&self) -> Palette<C> {
        todo!()
    }
}
