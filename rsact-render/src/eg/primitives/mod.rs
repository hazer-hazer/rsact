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
    /// `N` is the renderer's surface capacity (WS6.4d) — generic here because a
    /// primitive draws the same way into a full framebuffer and into a tile.
    fn draw<const N: usize>(
        &self,
        renderer: &mut EGRenderer<C, AntiAliasingDisabled, N>,
        style: DrawStyle<C>,
    ) -> RenderResult;

    fn draw_aa<const N: usize>(
        &self,
        renderer: &mut EGRenderer<C, AntiAliasingEnabled, N>,
        style: DrawStyle<C>,
    ) -> RenderResult;
}
