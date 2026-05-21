use crate::{
    color::Color,
    output::{RenderTarget, pixel::Pixel},
};
use embedded_graphics::draw_target::DrawTarget;

impl<D> RenderTarget for D
where
    D: DrawTarget,
    D::Color: Color,
{
    type Color = D::Color;

    fn draw(&mut self, pixels: impl Iterator<Item = Pixel<Self::Color>>) {
        if let Ok(_) = self.draw_iter(
            pixels.map(|p| embedded_graphics::Pixel(p.0.into(), p.1)),
        ) {
        } else {
            todo!()
        }
    }
}
