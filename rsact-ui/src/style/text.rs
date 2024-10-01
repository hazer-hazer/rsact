use embedded_text::alignment::{HorizontalAlignment, VerticalAlignment};

use crate::render::color::Color;

#[derive(Clone, Copy, PartialEq)]
pub struct MonoTextStyle<C: Color> {
    pub text_color: C,
    pub align: HorizontalAlignment,
    pub vertical_align: VerticalAlignment,
}

impl<C: Color> MonoTextStyle<C> {
    pub fn base() -> Self {
        Self {
            text_color: C::default_foreground(),
            align: HorizontalAlignment::Left,
            vertical_align: VerticalAlignment::Top,
        }
    }

    pub fn text_color(mut self, text_color: C) -> Self {
        self.text_color = text_color;
        self
    }
}
