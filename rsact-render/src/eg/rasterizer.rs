//! embedded-graphics' primitive algorithms as a [`Rasterizer`] — as-is, so
//! aliased.

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
/// Not [`DrawTargetProxy`](super::interop::DrawTargetProxy), which points the
/// other way — into a renderer rather than out to a blitter.
///
/// Overrides `draw_iter` and `fill_solid`; the latter is what reaches the
/// whole-word framebuffer path, without which a styled `Rectangle` fills one
/// pixel at a time. Not `fill_contiguous`, which would need a row-sized scratch
/// buffer per primitive to bridge eg's iterator to [`RasterCtx::run`]'s slice.
pub struct BlitTarget<'a, T: Blitter>(pub RasterCtx<'a, T>);

impl<'a, T: Blitter> Dimensions for BlitTarget<'a, T> {
    /// The **clip**, not the blitter's whole extent, which gets eg's own
    /// algorithms to bound themselves against it.
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

/// embedded-graphics' primitive algorithms, as a rasterizer.
///
/// **Overrides** the seven shapes embedded-graphics has primitives for; each
/// body builds eg's own primitive and `draw_styled`s it into a [`BlitTarget`].
///
/// **Inherits** [`crate::scan`] for `polygon`, `path` and `image`, which
/// embedded-graphics does not have.
///
/// **Does not override `fill`**: the default already reaches the whole-word
/// framebuffer path, so eg's `Rectangle` would only add a hop.
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
