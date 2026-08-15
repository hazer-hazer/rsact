//! The desktop/simulator sink: an L3 blitter over a tiny-skia [`Pixmap`].

use crate::{
    blitter::{Blitter, Span, span_range},
    color::Color,
    geometry::{Point, Rect, Size},
    renderer::RenderResult,
};
use alloc::vec::Vec;
use tiny_skia::{IntSize, Pixmap, PremultipliedColorU8};

/// Four, and named because the arithmetic below reads better with it.
const BYTES_PER_PIXEL: usize = 4;

/// A blitter over a tiny-skia `Pixmap` the caller owns.
///
/// # Why the storage is held as bytes
///
/// A `Pixmap` has a fixed `width()` and indexes `y * width + x`, so it looks at
/// first as though it must be bounded by *shape*. It is not: `Pixmap::take()`
/// yields its `Vec<u8>` and `Pixmap::from_vec` rebuilds one at any size the
/// vector's length matches, so [`begin_region`](Blitter::begin_region) reshapes
/// the storage per region — the trick WS6.4d already established — and the bound
/// is a **byte budget** exactly as for a framebuffer. A shrink keeps the
/// vector's capacity and a regrow stays inside it, so nothing reallocates. The
/// caller still hands in and gets back a real `Pixmap`; bytes are only the form
/// it is held in while lent.
///
/// **It always has its target**, and the renderer carries the attached/detached
/// state — see `RasterRenderer`'s docs for why that is the right way round.
/// This type IS the loan: the simulator gets it back from
/// `RasterRenderer::detach` and takes the pixels with
/// [`into_pixmap`](Self::into_pixmap).
pub struct PixmapBlitter {
    pixels: Vec<u8>,
    /// Bytes the attached vector was allocated with — the reshaping ceiling.
    ///
    /// `Vec::capacity` is not a contract (it may exceed what was asked for), so
    /// the figure the policy was checked against is remembered rather than
    /// re-read.
    capacity: usize,
    /// The rect the storage is currently shaped for, in absolute coordinates.
    region: Rect,
}

fn bytes_for(size: Size) -> usize {
    size.width as usize * size.height as usize * BYTES_PER_PIXEL
}

impl PixmapBlitter {
    /// Wrap the caller's pixmap. **Infallible**: any pixmap is a valid target,
    /// and whether it is big enough is a question about a frame policy this type
    /// has never heard of.
    ///
    /// Takes no viewport — unlike a bare `[u16; N]` a `Pixmap` knows its own
    /// shape, so it starts aimed at its own extent at the origin. That aim is
    /// overwritten by the first `begin_region` anyway; it exists so a caller who
    /// never begins a region still gets a whole pixmap back rather than a
    /// degenerate one.
    pub fn new(pixmap: Pixmap) -> Self {
        let region = Rect::new(
            Point::zero(),
            Size::new(pixmap.width(), pixmap.height()),
        );
        Self { capacity: bytes_for(region.size), pixels: pixmap.take(), region }
    }

    /// Take the pixmap back, sized to the region actually painted — so what the
    /// caller receives is exactly the tile, `encode_png`-able as-is. The
    /// vector's *capacity* survives the truncation, which is what makes
    /// re-wrapping it free.
    ///
    /// `None` if the target was aimed at a zero-sized region, which is the one
    /// case tiny-skia cannot represent. An `Option` rather than a `1x1` stand-in
    /// or an `expect`: handing back a pixmap that is not what was painted is a
    /// plausible-wrong-image failure, and panicking is the caller's decision to
    /// make, not ours.
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

    // `UNITS` stays at the trait default (`None`): a `Pixmap`'s extent is a
    // runtime fact — there is no `Pixmap<const W, const H>` — so only the value
    // can answer, and `capacity()` is where it does. `PIXELS_PER_UNIT` stays at
    // 1: `tiny_skia::Color` does not pack.

    /// One unit is one pixel, so the units the capacity proof counts and the
    /// pixels the pixmap holds are the same number.
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

    /// A real read-modify-write against the destination, which is what makes
    /// this the interesting blitter for `TinySkiaRasterizer`: coverage produced
    /// once by a `Mask` is blended once, here, rather than inside tiny-skia's
    /// fused painter.
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
            // Coverage scales the source alpha; the rest is `SourceOver` on
            // straight (non-premultiplied) channels, then premultiplied on the
            // way in. Kept explicit rather than routed through `Color::mix`
            // because the destination carries its own alpha.
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
    /// reallocated: the whole contract is that the storage is the caller's and
    /// fixed, so growing it would quietly take ownership of memory that is not
    /// ours. The policy check at attach time guarantees the planner never asks.
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
        // A region arrives holding whatever the previous one left in it, and
        // every blend here composites against the destination — so without this
        // the first anti-aliased edge would mix with an unrelated pixel.
        self.fill_rect(region, Self::Color::default_background());
        Ok(())
    }
}
