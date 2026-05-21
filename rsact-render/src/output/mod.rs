use crate::{color::Color, output::pixel::Pixel, renderer::Renderer};

pub mod pixel;

pub trait RenderTarget {
    type Color: Color;

    fn draw(&mut self, pixels: impl Iterator<Item = Pixel<Self::Color>>);
}
