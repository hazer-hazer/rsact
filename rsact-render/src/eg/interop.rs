//! How rsact talks to embedded-graphics **above** the layer split: the
//! [`DrawTargetProxy`] that lets `embedded-text`/u8g2 hand glyph pixels to a
//! `Renderer`, and the style conversions every `EgRasterizer` body needs.
//!
//! [`DrawTargetProxy`] and `eg::rasterizer::BlitTarget` are both `DrawTarget`
//! adapters pointing in opposite directions: this one feeds *into* a renderer
//! from above, `BlitTarget` feeds *out of* a rasterizer into a blitter below.

use crate::{
    color::{Color, RgbColor},
    geometry::Point,
    renderer::Renderer,
    style::{DrawStyle, StrokeAlignment},
};
use embedded_graphics::{
    geometry::OriginDimensions,
    pixelcolor::Rgb888,
    prelude::{DrawTarget, PixelColor},
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder},
};

/// Proxy to draw on Renderer as on embedded_graphics DrawTarget, works by
/// mapping any color into embedded_graphics Rgb888.
pub struct DrawTargetProxy<'a, R: Renderer> {
    renderer: &'a mut R,
}

impl<'a, R: Renderer> DrawTargetProxy<'a, R> {
    pub fn new(renderer: &'a mut R) -> Self {
        Self { renderer }
    }
}

impl<'a, R: Renderer> OriginDimensions for DrawTargetProxy<'a, R> {
    fn size(&self) -> embedded_graphics::prelude::Size {
        self.renderer.size().into()
    }
}

impl<'a, C: Color, R: Renderer<Color = C>> DrawTarget
    for DrawTargetProxy<'a, R>
{
    type Color = Rgb888;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::prelude::Pixel<Self::Color>>,
    {
        // Drop pixels the renderer would reject anyway, before paying
        // `Renderer::pixel` for each. This is the only path text takes, and a
        // label straddling a region boundary is culled in neither region, so
        // every glyph pixel is otherwise offered twice — each paying a color
        // conversion and a dispatch into the framebuffer to be discarded.
        //
        // TODO: a write filter, not a loop bound. The glyph iteration upstream
        // still runs, because the line/glyph loop belongs to `embedded-text`.
        // Owning that loop is what would make first/last-visible-glyph
        // arithmetic possible.
        //
        // Read once, outside the loop: `clip_bounds` borrows immutably and
        // `pixel` needs a mutable borrow.
        let clip = self.renderer.clip_bounds();
        pixels
            .into_iter()
            .filter(|p| clip.is_none_or(|clip| clip.contains(Point::from(p.0))))
            .try_for_each(|p| {
                self.renderer.pixel(
                    p.0.into(),
                    C::from_rgba(crate::color::Rgba {
                        r: p.1.r(),
                        g: p.1.g(),
                        b: p.1.b(),
                        a: 255,
                    }),
                )
            })
    }
}

impl Into<embedded_graphics::primitives::StrokeAlignment> for StrokeAlignment {
    fn into(self) -> embedded_graphics::primitives::StrokeAlignment {
        match self {
            Self::Inside => {
                embedded_graphics::primitives::StrokeAlignment::Inside
            },
            Self::Center => {
                embedded_graphics::primitives::StrokeAlignment::Center
            },
            Self::Outside => {
                embedded_graphics::primitives::StrokeAlignment::Outside
            },
        }
    }
}

impl<C: Color + PixelColor> DrawStyle<C> {
    pub fn into_primitive_style(self) -> PrimitiveStyle<C> {
        let mut builder = PrimitiveStyleBuilder::new()
            .stroke_width(self.stroke_width)
            .stroke_alignment(self.stroke_alignment.into());
        if let Some(fill) = self.fill {
            builder = builder.fill_color(fill);
        }
        if let Some(stroke) = self.stroke {
            builder = builder.stroke_color(stroke);
        }
        builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        geometry::{Rect, Size},
        record::{DrawOp, RecordingRenderer},
    };
    use alloc::vec::Vec;
    use embedded_graphics::pixelcolor::Rgb888;

    /// The proxy must drop pixels outside the renderer's clip, and **only**
    /// those. Both failure modes are silent: filter too little and tiling pays
    /// N× the per-pixel work, filter too much and glyphs lose columns.
    #[test]
    fn the_proxy_filters_pixels_the_renderer_would_reject() {
        let mut rec = RecordingRenderer::<Rgb888>::new(Size::new(64, 64));
        rec.push_clip(Rect::new(Point::new(10, 10), Size::new(10, 10)));

        let ink = Rgb888::new(255, 255, 255);
        let at = |x, y| {
            embedded_graphics::prelude::Pixel(
                embedded_graphics::prelude::Point::new(x, y),
                ink,
            )
        };
        DrawTargetProxy::new(&mut rec)
            .draw_iter([
                at(15, 15), // inside
                at(19, 19), // inside, last pixel of the clip
                at(20, 20), // outside: the clip's edge is exclusive
                at(5, 5),   // outside
            ])
            .unwrap();

        let drawn: Vec<_> = rec
            .ops()
            .into_iter()
            .filter_map(|op| match op {
                DrawOp::Pixel(point) => Some(point),
                _ => None,
            })
            .collect();
        assert_eq!(drawn, [Point::new(15, 15), Point::new(19, 19)]);
    }
}
