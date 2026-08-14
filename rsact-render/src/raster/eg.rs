//! embedded-graphics as an L2 [`Rasterizer`] — **as-is, no anti-aliasing**.

use crate::{
    blitter::Blitter,
    color::Color,
    eg::primitives,
    geometry::{Angle, CornerRadii, Point, Rect},
    primitives::{
        arc::Arc, circle::Circle, ellipse::Ellipse, line::Line,
        rounded_rect::RoundedRect, sector::Sector,
    },
    raster::{RasterCtx, Rasterizer},
    style::DrawStyle,
};
use embedded_graphics::{
    geometry::Dimensions, pixelcolor::PixelColor, prelude::DrawTarget,
};

/// A [`RasterCtx`] wearing embedded-graphics' `DrawTarget`, so that crate's
/// algorithms can emit into a blitter.
///
/// # Two `DrawTarget` adapters now coexist, and they are not the same thing
///
/// Expect to be confused by this exactly once:
///
/// - **`BlitTarget`** — here, **below** L1. It lets eg's `StyledDrawable`
///   algorithms write through the clip gate into an L3 blitter.
/// - **`DrawTargetProxy`** (`eg/renderer.rs`) — **above** L1. It is how
///   `embedded-text`/u8g2 hand glyph pixels to a `Renderer`, one at a time, and
///   it is unchanged by the split: text still arrives as `Renderer::pixel` until
///   WS15 gives the rasterizer a `glyphs` method.
///
/// # What is and is not overridden
///
/// `draw_iter` and `fill_solid`, and deliberately not `fill_contiguous`. The
/// first two are what eg's algorithms actually call — `fill_solid` being the one
/// that reaches WS6.3b's whole-word framebuffer path, without which a styled
/// `Rectangle` bounces through `fill_contiguous` → `draw_iter` and fills a rect
/// one pixel at a time.
///
/// `fill_contiguous` would map onto [`RasterCtx::run`], which the design sketch
/// proposed, but `run` needs a **slice** while eg hands over an iterator. The
/// only way to bridge that is a row-sized scratch buffer, and a `BlitTarget` is
/// constructed per primitive call — so it would be an allocation per primitive
/// to serve a method whose default (`draw_iter`, i.e. `cx.pixel`) is already
/// correct. Revisit when a rasterizer holds a scratch line for its own reasons;
/// `RsactRasterizer` will.
pub struct BlitTarget<'a, T: Blitter + ?Sized>(pub RasterCtx<'a, T>);

impl<'a, T: Blitter + ?Sized> Dimensions for BlitTarget<'a, T> {
    /// The **clip**, not the blitter's whole extent.
    ///
    /// This is the honest answer and also a free optimization: eg's own
    /// `clipped`/`cropped` adapters and several of its algorithms intersect
    /// against this box, so reporting the clip turns the advisory "bound your
    /// loops" row of `RasterCtx`'s table into something eg does for us.
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        self.0.clip().into()
    }
}

impl<'a, T: Blitter + ?Sized> DrawTarget for BlitTarget<'a, T>
where
    T::Color: PixelColor,
{
    type Color = T::Color;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for embedded_graphics::Pixel(point, color) in pixels {
            self.0.pixel(Point::from(point), color);
        }
        Ok(())
    }

    fn fill_solid(
        &mut self,
        area: &embedded_graphics::primitives::Rectangle,
        color: Self::Color,
    ) -> Result<(), Self::Error> {
        self.0.rect(Rect::from(*area), color);
        Ok(())
    }
}

/// embedded-graphics' primitive algorithms, as an L2 rasterizer.
///
/// **Overrides** the seven shapes embedded-graphics has primitives for; each
/// body is the delegation PR A left behind, with the receiver changed to a
/// [`BlitTarget`] — a substitution, not a rewrite.
///
/// **Inherits** `scan::` for `polygon`, `path` and `image`, which
/// embedded-graphics does not have: its `polygon` and `ImageDrawable` are both
/// logged no-ops, so the shared default is not a fallback here, it is the only
/// implementation there has ever been.
///
/// **Does not override `fill`.** The default reaches `cx.rect` →
/// [`Blitter::fill_rect`] → `Framebuf::fill_solid`, which is WS6.3b's whole-word
/// path. Routing it through eg's `Rectangle` would add a hop and arrive at the
/// same place.
///
/// **No `C` parameter.** The color comes from `T::Color`; a `PhantomData<C>`
/// would add a monomorphization axis with no code difference.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EgRasterizer;

impl<T: Blitter + ?Sized> Rasterizer<T> for EgRasterizer
where
    T::Color: Color + PixelColor,
{
    fn line(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        from: Point,
        to: Point,
        style: &DrawStyle<T::Color>,
    ) {
        let _ = primitives::line::draw(
            &mut BlitTarget(cx.reborrow()),
            &Line::new(from, to),
            style,
        );
    }

    fn rect(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        rect: Rect,
        style: &DrawStyle<T::Color>,
    ) {
        let eg_rect: embedded_graphics::primitives::Rectangle = rect.into();
        let _ = embedded_graphics::primitives::StyledDrawable::draw_styled(
            &eg_rect,
            &style.into_primitive_style(),
            &mut BlitTarget(cx.reborrow()),
        );
    }

    fn rounded_rect(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        rect: Rect,
        corners: CornerRadii,
        style: &DrawStyle<T::Color>,
    ) {
        let _ = primitives::rounded_rect::draw(
            &mut BlitTarget(cx.reborrow()),
            &RoundedRect::new(rect, corners),
            style,
        );
    }

    fn circle(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        top_left: Point,
        diameter: u32,
        style: &DrawStyle<T::Color>,
    ) {
        let _ = primitives::circle::draw(
            &mut BlitTarget(cx.reborrow()),
            &Circle::new(top_left, diameter),
            style,
        );
    }

    fn arc(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<T::Color>,
    ) {
        let _ = primitives::arc::draw(
            &mut BlitTarget(cx.reborrow()),
            &Arc::new(top_left, diameter, start, sweep),
            style,
        );
    }

    fn sector(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<T::Color>,
    ) {
        let _ = primitives::sector::draw(
            &mut BlitTarget(cx.reborrow()),
            &Sector::new(top_left, diameter, start, sweep),
            style,
        );
    }

    fn ellipse(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        bounding_box: Rect,
        style: &DrawStyle<T::Color>,
    ) {
        let _ = primitives::ellipse::draw(
            &mut BlitTarget(cx.reborrow()),
            &Ellipse::new(bounding_box.top_left, bounding_box.size),
            style,
        );
    }
}
