//! Drawing into a tiny-skia [`Pixmap`] — the desktop and simulator target.

use crate::{
    blitter::{Blitter, Span, span_range},
    color::Color,
    geometry::{Point, Rect, Size},
    renderer::RenderResult,
};
use alloc::vec::Vec;
use tiny_skia::{IntSize, Pixmap, PremultipliedColorU8};

const BYTES_PER_PIXEL: usize = 4;

/// A blitter over a tiny-skia `Pixmap` the caller owns.
///
/// The pixels are held as bytes while lent. A `Pixmap` has a fixed `width()`,
/// which suggests the bound must be a *shape* — but `take()` yields its
/// `Vec<u8>` and `from_vec` rebuilds one at any size the length matches, so
/// [`begin_region`](Blitter::begin_region) reshapes the storage per region and
/// the bound is a **byte budget**, as it is for a framebuffer. Reshaping stays
/// within the original allocation, so nothing reallocates.
///
/// This type is the loan itself: the caller gets it back from
/// `RasterRenderer::detach` and takes the pixels with
/// [`into_pixmap`](Self::into_pixmap).
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
    /// Wrap the caller's pixmap. Infallible — whether it is big enough is a
    /// question about a frame policy this type has never heard of.
    ///
    /// It starts aimed at the pixmap's own extent, which the first
    /// `begin_region` overwrites; the initial aim only matters to a caller who
    /// never begins one.
    pub fn new(pixmap: Pixmap) -> Self {
        let region = Rect::new(
            Point::zero(),
            Size::new(pixmap.width(), pixmap.height()),
        );
        Self { capacity: bytes_for(region.size), pixels: pixmap.take(), region }
    }

    /// Take the pixmap back, sized to the region actually painted, along with
    /// where that region was.
    ///
    /// `None` for a zero-sized region — the one shape tiny-skia cannot
    /// represent. Returning a `1x1` stand-in instead would hand back a pixmap
    /// that is not what was painted.
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

    /// Reshape the storage to `region`, then prime it.
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
        // A region arrives holding the previous one's pixels, and every blend
        // composites against the destination: an anti-aliased edge would
        // otherwise mix with an unrelated pixel.
        self.fill_rect(region, Self::Color::default_background());
        Ok(())
    }
}
