use crate::{
    color::Color,
    eg::{framebuf::PackedColor, renderer::EGRenderer},
    renderer::{AntiAliasingDisabled, AntiAliasingEnabled, RenderResult},
    style::DrawStyle,
};
use embedded_graphics::pixelcolor::PixelColor;

pub mod arc;
pub mod circle;
pub mod ellipse;
pub mod line;
pub mod polygon;
pub mod rounded_rect;
pub mod sector;

pub trait EgPrimitive<C: Color + PackedColor + PixelColor> {
    fn draw(
        &self,
        renderer: &mut EGRenderer<C, AntiAliasingDisabled>,
        style: DrawStyle<C>,
    ) -> RenderResult;

    fn draw_aa(
        &self,
        renderer: &mut EGRenderer<C, AntiAliasingEnabled>,
        style: DrawStyle<C>,
    ) -> RenderResult;
}
