use crate::{
    color::Color,
    eg::framebuf::{Framebuffer, PackedColor},
    output::pixel::Pixel,
    region::FramePolicy,
    renderer::{
        AntiAliasing, AntiAliasingDisabled, AntiAliasingEnabled, Attached,
        RenderResult, Renderer,
    },
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
/// Implemented for [`Attached`] renderers **only**. Painting into a surface the
/// application is holding is not a mistake this trait can be used to make: a
/// detached renderer has no `Slot`, so it satisfies nothing here, and the error
/// arrives at the call site rather than as a warning in a log nobody reads.
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
/// # `AA` is kept; `B` and `P` are what go
///
/// The anti-aliasing marker stays a parameter here, so
/// [`EgPrimitive::draw`] can only be handed a non-AA renderer and
/// [`EgPrimitive::draw_aa`] only an AA one — the same witness the concrete
/// `EGRenderer<C, AA, B, P>` signatures carried. It is load-bearing in a way
/// the storage parameters never were: `Renderer::line` and the primitives'
/// mutual `draw_aa` calls dispatch on it, so erasing it would let a `draw` path
/// silently resolve to AA code and back.
///
/// What goes is `B` and `P`. Those describe where pixels are *stored*, a
/// decision no primitive participates in, and naming them forced fourteen
/// signatures to restate it.
pub trait EgPrimitiveRenderer<
    C: Color + PackedColor + PixelColor,
    AA: AntiAliasing,
>: Renderer<Color = C> + DrawTarget<Color = C, Error = ()>
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
> EgPrimitiveRenderer<C, AA>
    for crate::eg::renderer::EGRenderer<C, AA, B, P, Attached>
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
    /// that can satisfy [`EgPrimitiveRenderer`]. The `AntiAliasingDisabled`
    /// witness is kept, so this cannot be handed an AA renderer.
    fn draw<R: EgPrimitiveRenderer<C, AntiAliasingDisabled>>(
        &self,
        renderer: &mut R,
        style: DrawStyle<C>,
    ) -> RenderResult;

    /// Draw with anti-aliasing — the blended-edge path.
    fn draw_aa<R: EgPrimitiveRenderer<C, AntiAliasingEnabled>>(
        &self,
        renderer: &mut R,
        style: DrawStyle<C>,
    ) -> RenderResult;
}
