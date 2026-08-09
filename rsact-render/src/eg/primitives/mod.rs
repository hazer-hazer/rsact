use crate::{
    color::Color,
    eg::{
        framebuf::{PackedColor, Surface},
        renderer::EGRenderer,
    },
    region::FramePolicy,
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
    /// `B` is the renderer's surface (WS6.4d) — generic here because a
    /// primitive draws the same way into a full framebuffer and into a tile.
    fn draw<B: Surface<C>, P: FramePolicy>(
        &self,
        renderer: &mut EGRenderer<C, AntiAliasingDisabled, B, P>,
        style: DrawStyle<C>,
    ) -> RenderResult;

    fn draw_aa<B: Surface<C>, P: FramePolicy>(
        &self,
        renderer: &mut EGRenderer<C, AntiAliasingEnabled, B, P>,
        style: DrawStyle<C>,
    ) -> RenderResult;
}
