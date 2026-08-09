use crate::{
    color::Color,
    eg::framebuf::{Framebuffer, PackedColor},
    output::pixel::Pixel,
    region::FramePolicy,
    renderer::{AntiAliasing, RenderResult, Renderer},
    style::DrawStyle,
};
use embedded_graphics::{draw_target::DrawTarget, pixelcolor::PixelColor};

pub mod arc;
pub mod circle;
pub mod ellipse;
pub mod line;
pub mod polygon;
pub mod rounded_rect;
pub mod sector;

/// What a primitive actually needs from a renderer to draw itself.
///
/// The primitives are pure geometry: they emit styled shapes, blended pixels
/// and lines, and they neither know nor care where those land. Naming
/// `EGRenderer<C, AA, B, P>` in their signatures forced every one of them to
/// carry `B` (the surface) and `P` (the frame policy) as generics they never
/// mention again — sixteen signatures restating a storage decision none of them
/// participate in, and two more parameters to thread every time that decision
/// changes shape. This is that requirement stated directly instead.
///
/// The three members are exactly what the seven primitives call, and nothing
/// else:
///
/// - [`DrawTarget`] for embedded-graphics' own `draw_styled`, which is the
///   whole non-AA path;
/// - [`pixel_alpha`](Self::pixel_alpha) for the AA paths, which blend against
///   the destination one pixel at a time;
/// - [`draw_pixels`](Self::draw_pixels) for `Polygon`'s scanline fill, plus
///   [`Renderer::line`] for its edges.
///
/// # Anti-aliasing is still selected by which method you call
///
/// [`EgPrimitive::draw`] and [`EgPrimitive::draw_aa`] are separate methods, and
/// the two `Renderer` impls on `EGRenderer` call the matching one — that is the
/// real selector. The `AA` type parameter on the renderer was a *second*
/// encoding of the same fact, and it is not preserved in this bound: passing an
/// AA-capable renderer to `draw` now type-checks and draws a non-AA shape, which
/// is what you asked for. The two call sites are both inside the AA-specific
/// impls, so nothing observable changes.
pub trait EgPrimitiveRenderer<C: Color + PackedColor + PixelColor>:
    Renderer<Color = C> + DrawTarget<Color = C, Error = ()>
{
    /// Blend `pixel`'s colour into whatever the destination already holds.
    fn pixel_alpha(&mut self, pixel: Pixel<C>, blend: f32) -> RenderResult;

    /// Write a run of already-positioned pixels.
    fn draw_pixels(
        &mut self,
        pixels: impl IntoIterator<Item = Pixel<C>>,
    ) -> RenderResult;
}

impl<
    C: Color + PackedColor + PixelColor,
    AA: AntiAliasing,
    B: Framebuffer<C>,
    P: FramePolicy,
> EgPrimitiveRenderer<C> for crate::eg::renderer::EGRenderer<C, AA, B, P>
where
    Self: Renderer<Color = C>,
{
    fn pixel_alpha(&mut self, pixel: Pixel<C>, blend: f32) -> RenderResult {
        Self::pixel_alpha(self, pixel, blend)
    }

    fn draw_pixels(
        &mut self,
        pixels: impl IntoIterator<Item = Pixel<C>>,
    ) -> RenderResult {
        Self::draw_pixels(self, pixels)
    }
}

pub trait EgPrimitive<C: Color + PackedColor + PixelColor> {
    /// Draw without anti-aliasing.
    ///
    /// Generic over the renderer rather than naming `EGRenderer`: a primitive
    /// draws the same way into a full framebuffer, a tile, and anything else
    /// that can satisfy [`EgPrimitiveRenderer`].
    fn draw<R: EgPrimitiveRenderer<C>>(
        &self,
        renderer: &mut R,
        style: DrawStyle<C>,
    ) -> RenderResult;

    /// Draw with anti-aliasing — the blended-edge path.
    fn draw_aa<R: EgPrimitiveRenderer<C>>(
        &self,
        renderer: &mut R,
        style: DrawStyle<C>,
    ) -> RenderResult;
}
