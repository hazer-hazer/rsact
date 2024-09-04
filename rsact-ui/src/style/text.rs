use crate::render::color::Color;

#[derive(Clone, Copy, PartialEq)]
pub struct MonoTextStyle<C: Color> {
    pub text_color: C,
}

impl<C: Color> MonoTextStyle<C> {
    pub fn base() -> Self {
        Self { text_color: C::default_foreground() }
    }

    pub fn text_color(mut self, text_color: C) -> Self {
        self.text_color = text_color;
        self
    }
}
