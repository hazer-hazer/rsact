//! embedded-graphics as an L2 [`Rasterizer`] — **as-is, no anti-aliasing**.

use crate::{
    blitter::Blitter,
    color::Color,
    geometry::{Angle, CornerRadii, Point, Rect},
    raster::{RasterCtx, Rasterizer},
    style::DrawStyle,
};
use embedded_graphics::{
    geometry::Dimensions,
    pixelcolor::PixelColor,
    prelude::DrawTarget,
    primitives::{
        Arc, Circle, Ellipse, Line, Rectangle, RoundedRectangle, Sector,
        StyledDrawable,
    },
};

/// A [`RasterCtx`] wearing embedded-graphics' `DrawTarget`, so that crate's
/// algorithms can emit into a blitter.
///
/// # Two `DrawTarget` adapters coexist, and they are not the same thing
///
/// Expect to be confused by this exactly once:
///
/// - **`BlitTarget`** — here, **below** L1. It lets eg's `StyledDrawable`
///   algorithms write through the clip gate into an L3 blitter.
/// - **[`DrawTargetProxy`](super::interop::DrawTargetProxy)** — **above** L1. It
///   is how `embedded-text`/u8g2 hand glyph pixels to a `Renderer`, one at a
///   time, and it is unchanged by the split: text still arrives as
///   `Renderer::pixel` until WS15 gives the rasterizer a `glyphs` method.
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
pub struct BlitTarget<'a, T: Blitter>(pub RasterCtx<'a, T>);

impl<'a, T: Blitter> Dimensions for BlitTarget<'a, T> {
    /// The **clip**, not the blitter's whole extent.
    ///
    /// This is the honest answer and also a free optimization: eg's own
    /// `clipped`/`cropped` adapters and several of its algorithms intersect
    /// against this box, so reporting the clip turns the advisory "bound your
    /// loops" row of `RasterCtx`'s table into something eg does for us.
    fn bounding_box(&self) -> Rectangle {
        self.0.clip().into()
    }
}

impl<'a, T: Blitter> DrawTarget for BlitTarget<'a, T>
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
        area: &Rectangle,
        color: Self::Color,
    ) -> Result<(), Self::Error> {
        self.0.rect(Rect::from(*area), color);
        Ok(())
    }
}

/// embedded-graphics' primitive algorithms, as an L2 rasterizer.
///
/// **Overrides** the seven shapes embedded-graphics has primitives for. Each
/// body is one call: build eg's own primitive from the arguments and
/// `draw_styled` it into a [`BlitTarget`].
///
/// **They used to be seven files.** `eg/primitives/` held a `pub fn draw` per
/// shape, each taking rsact's `Line`/`Arc`/`Circle`/… and converting to eg's —
/// a shape left behind when PR A deleted the anti-aliased halves those modules
/// existed to pair with. Inlining them here removed the intermediate rsact
/// primitive entirely: the arguments go straight into eg's constructor, which is
/// what they always did two hops later.
///
/// **Inherits** [`crate::scan`] for `polygon`, `path` and `image`, which
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

impl<T: Blitter> Rasterizer<T> for EgRasterizer
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
        let _ = Line::new(from.into(), to.into()).draw_styled(
            &style.into_primitive_style(),
            &mut BlitTarget(cx.reborrow()),
        );
    }

    fn rect(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        rect: Rect,
        style: &DrawStyle<T::Color>,
    ) {
        let _ = Rectangle::from(rect).draw_styled(
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
        let _ = RoundedRectangle::new(rect.into(), corners.into()).draw_styled(
            &style.into_primitive_style(),
            &mut BlitTarget(cx.reborrow()),
        );
    }

    /// Overridden rather than inherited on purpose: the default decomposes a
    /// circle onto `ellipse`, and embedded-graphics has its own circle
    /// algorithm — taking the default would silently discard it.
    fn circle(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        top_left: Point,
        diameter: u32,
        style: &DrawStyle<T::Color>,
    ) {
        let _ = Circle::new(top_left.into(), diameter).draw_styled(
            &style.into_primitive_style(),
            &mut BlitTarget(cx.reborrow()),
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
        let _ = Arc::new(top_left.into(), diameter, start.into(), sweep.into())
            .draw_styled(
                &style.into_primitive_style(),
                &mut BlitTarget(cx.reborrow()),
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
        let _ =
            Sector::new(top_left.into(), diameter, start.into(), sweep.into())
                .draw_styled(
                    &style.into_primitive_style(),
                    &mut BlitTarget(cx.reborrow()),
                );
    }

    fn ellipse(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        bounding_box: Rect,
        style: &DrawStyle<T::Color>,
    ) {
        let _ = Ellipse::new(
            bounding_box.top_left.into(),
            bounding_box.size.into(),
        )
        .draw_styled(
            &style.into_primitive_style(),
            &mut BlitTarget(cx.reborrow()),
        );
    }
}
