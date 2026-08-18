//! Drawing into a tiny-skia [`Pixmap`] — the desktop and simulator target.

use crate::{
    blitter::{Blitter, Span, span_range},
    geometry::{Point, Rect, Size},
    renderer::RenderResult,
};
use alloc::vec::Vec;
use tiny_skia::{IntSize, Pixmap, PremultipliedColorU8};

const BYTES_PER_PIXEL: usize = 4;

/// A blitter over a tiny-skia `Pixmap` the caller owns.
///
/// Held as bytes while lent, so [`begin_region`](Blitter::begin_region) can
/// reshape the storage per region: the bound is a **byte budget**, not the
/// pixmap's shape. Reshaping stays inside the original allocation.
///
/// Take the pixels back with [`into_pixmap`](Self::into_pixmap).
pub struct PixmapBlitter {
    pixels: Vec<u8>,
    /// The reshaping ceiling. Remembered rather than read back from
    /// `Vec::capacity`, which may exceed what was asked for.
    capacity: usize,
    /// The rect the storage is currently shaped for, in absolute coordinates.
    region: Rect,
}

fn bytes_for(size: Size) -> usize {
    size.width as usize * size.height as usize * BYTES_PER_PIXEL
}

impl PixmapBlitter {
    /// Wrap the caller's pixmap, aimed at its own extent until the first
    /// `begin_region`. Infallible — whether it is big enough is a question
    /// about a frame policy.
    pub fn new(pixmap: Pixmap) -> Self {
        let region = Rect::new(
            Point::zero(),
            Size::new(pixmap.width(), pixmap.height()),
        );
        Self { capacity: bytes_for(region.size), pixels: pixmap.take(), region }
    }

    /// Take the pixmap back, sized to the region painted, with where it was.
    /// `None` for a zero-sized region, which tiny-skia cannot represent.
    pub fn into_pixmap(self) -> Option<(Pixmap, Rect)> {
        let at = self.region;
        if at.is_zero_sized() {
            return None;
        }
        let mut pixels = self.pixels;
        pixels.resize(bytes_for(at.size), 0);
        let size = IntSize::from_wh(at.size.width, at.size.height)?;
        Some((Pixmap::from_vec(pixels, size)?, at))
    }

    fn premultiplied(color: tiny_skia::Color) -> PremultipliedColorU8 {
        color.premultiply().to_color_u8()
    }

    fn set(&mut self, index: usize, color: PremultipliedColorU8) {
        let at = index * BYTES_PER_PIXEL;
        if at + BYTES_PER_PIXEL > self.pixels.len() {
            return;
        }
        self.pixels[at] = color.red();
        self.pixels[at + 1] = color.green();
        self.pixels[at + 2] = color.blue();
        self.pixels[at + 3] = color.alpha();
    }
}

impl Blitter for PixmapBlitter {
    type Color = tiny_skia::Color;

    fn bounds(&self) -> Rect {
        self.region
    }

    // `UNITS` stays `None`: a `Pixmap`'s extent is a runtime fact, so only
    // `capacity` can answer. `PIXELS_PER_UNIT` stays 1 — `Color` does not pack.

    /// One unit is one pixel.
    fn capacity(&self) -> Option<usize> {
        Some(self.capacity / BYTES_PER_PIXEL)
    }

    fn fill_span(&mut self, span: Span, color: Self::Color) {
        let premul = Self::premultiplied(color);
        for i in span_range(&self.region, span) {
            self.set(i, premul);
        }
    }

    fn fill_run(&mut self, span: Span, colors: &[Self::Color]) {
        debug_assert_eq!(colors.len(), span.len());
        for (i, color) in span_range(&self.region, span).zip(colors) {
            self.set(i, Self::premultiplied(*color));
        }
    }

    /// `SourceOver`, scaling the source alpha by coverage. This is where a
    /// `TinySkiaRasterizer`'s mask is blended — once, and outside tiny-skia.
    fn blend_span(&mut self, span: Span, color: Self::Color, coverage: &[u8]) {
        debug_assert_eq!(coverage.len(), span.len());
        for (i, cov) in span_range(&self.region, span).zip(coverage) {
            if *cov == 0 {
                continue;
            }
            let at = i * BYTES_PER_PIXEL;
            if at + BYTES_PER_PIXEL > self.pixels.len() {
                continue;
            }
            let src = color.to_color_u8();
            // Straight (non-premultiplied) channels, premultiplied on the way
            // in. Not `Color::mix`: the destination carries its own alpha.
            let a = (src.alpha() as u32 * *cov as u32) / 255;
            let inv = 255 - a;
            let blend =
                |s: u8, d: u8| ((s as u32 * a + d as u32 * inv) / 255) as u8;
            let dst = (
                self.pixels[at],
                self.pixels[at + 1],
                self.pixels[at + 2],
                self.pixels[at + 3],
            );
            self.pixels[at] = blend(src.red(), dst.0);
            self.pixels[at + 1] = blend(src.green(), dst.1);
            self.pixels[at + 2] = blend(src.blue(), dst.2);
            self.pixels[at + 3] =
                (a + (dst.3 as u32 * inv) / 255).min(255) as u8;
        }
    }

    fn pixel(&mut self, p: Point, color: Self::Color) {
        if !self.region.contains(p) {
            return;
        }
        let index = crate::blitter::pixel_index(&self.region, p);
        self.set(index, Self::premultiplied(color));
    }

    /// Reshape the storage to `region`. Does **not** paint — see
    /// [`Blitter::begin_region`].
    ///
    /// The reshape zeroes any bytes the new region adds, but nothing more: a
    /// region that shrinks and grows again is left holding the previous
    /// region's pixels, so the caller's background fill is what makes an
    /// anti-aliased edge composite against its own background rather than an
    /// unrelated one.
    ///
    /// # Errors
    ///
    /// If `region` needs more bytes than were lent. Refused rather than
    /// reallocated — the storage is the caller's, and growing it would quietly
    /// take ownership of memory that is not ours.
    fn begin_region(&mut self, region: Rect) -> RenderResult {
        let want = bytes_for(region.size);
        if want > self.capacity {
            log::error!(
                "region {region:?} needs {want} bytes, the attached pixmap \
                 holds {}; skipping it",
                self.capacity,
            );
            return Err(());
        }
        self.pixels.resize(want, 0);
        self.region = region;
        Ok(())
    }
}
