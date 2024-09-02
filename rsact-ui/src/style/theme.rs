use crate::render::color::Color;

pub struct Palette<C: Color> {
    background: C,
    foreground: C,
}

pub enum Theme {
    Light,
    Dark,
}
