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
        // A draw-target error is a real device condition (e.g. display-bus
        // failure). Log and drop this pixel batch rather than `todo!()`, which
        // would abort the device on a transient glitch.
        if self
            .draw_iter(pixels.map(|p| embedded_graphics::Pixel(p.0.into(), p.1)))
            .is_err()
        {
            log::error!("draw target draw_iter failed; dropping pixel batch");
        }
    }
}
