//! tiny-skia's rasterizer as an L2 citizen.

use crate::{
    blitter::{Blitter, Span},
    geometry::{Angle, CornerRadii, Point, Rect, Size},
    path::Path,
    raster::{RasterCtx, Rasterizer},
    style::DrawStyle,
    tiny_skia::path::PathBuilderExt,
};
use tiny_skia::{FillRule, Mask, PathBuilder, Stroke, Transform};

/// tiny-skia's scan conversion, producing **coverage** rather than pixels.
///
/// # Why this is L2 and not a backend of its own
///
/// tiny-skia's public painting API is *fused*: `PixmapMut::fill_path` rasterizes
/// and blends in one call, into a `Pixmap`. Fusing is what this design undoes,
/// so none of it is used here. [`Mask::fill_path`] is the primitive underneath
/// those calls — this repo already called it, to build clip masks — and it hands
/// back the coverage buffer itself. So coverage is produced **once** and blended
/// **once**, in our blitter.
///
/// The payoff is that no color is pinned. `impl<T: Blitter> Rasterizer<T>` with
/// no bound on `T::Color` makes tiny-skia's anti-aliasing available over an
/// **Rgb565 framebuffer**, not only over a `Pixmap` — which an L1-only tiny-skia
/// backend could never offer.
///
/// # The mask is keyed on the clip, not the region
///
/// L2 never learns a region exists; the clip is the largest rect it may write
/// anyway. The buffer is grow-only and re-used, because a `Mask` is `w·h` bytes
/// and reallocating one per primitive would dominate everything else. Rows are
/// read at the *allocated* width, and only the clip's own width and height are
/// emitted.
///
/// # What is deliberately not overridden
///
/// [`fill`](Rasterizer::fill) and [`pixel`](Rasterizer::pixel). A style-free
/// axis-aligned rect has no edge to anti-alias, so routing it through a mask
/// would cost a coverage buffer and a per-pixel blend to arrive at the same
/// pixels the blitter's own `fill_rect` writes with `slice::fill`.
/// [`image`](Rasterizer::image) is inherited for the same kind of reason: it is
/// a decode, not a rasterization.
pub struct TinySkiaRasterizer {
    mask: Option<Mask>,
    /// The allocated mask's size, which is the high-water mark of every clip
    /// seen so far — not the current clip.
    mask_size: Size,
    /// Reused per stroke so its scratch allocations survive between primitives.
    stroker: tiny_skia::PathStroker,
}

impl Default for TinySkiaRasterizer {
    fn default() -> Self {
        Self::new()
    }
}

impl TinySkiaRasterizer {
    pub fn new() -> Self {
        Self {
            mask: None,
            mask_size: Size::zero(),
            stroker: tiny_skia::PathStroker::new(),
        }
    }

    /// Rasterize `path` into the mask and blit its coverage as `color`.
    ///
    /// The transform carries absolute coordinates into mask-local ones, so the
    /// mask's `(0, 0)` is the clip's top-left — which is what lets one buffer
    /// serve any clip without re-addressing.
    fn emit<T: Blitter>(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        path: &tiny_skia::Path,
        color: T::Color,
    ) {
        let clip = cx.clip();
        if clip.is_zero_sized() {
            return;
        }

        if self.mask_size.width < clip.size.width
            || self.mask_size.height < clip.size.height
        {
            self.mask_size = Size::new(
                self.mask_size.width.max(clip.size.width),
                self.mask_size.height.max(clip.size.height),
            );
            self.mask = Mask::new(self.mask_size.width, self.mask_size.height);
        }
        let Some(mask) = self.mask.as_mut() else { return };

        // `fill_path` accumulates onto whatever is already there — it is
        // documented as drawing on top — so the previous primitive's coverage
        // has to go first, or every shape inherits the last one's edges.
        mask.clear();
        mask.fill_path(
            path,
            FillRule::Winding,
            true,
            Transform::from_translate(
                -clip.top_left.x as f32,
                -clip.top_left.y as f32,
            ),
        );

        let stride = mask.width() as usize;
        let width = clip.size.width as usize;
        let data = mask.data();
        for row in 0..clip.size.height as usize {
            let start = row * stride;
            cx.blend(
                Span::new(
                    clip.top_left.y + row as i32,
                    clip.top_left.x,
                    clip.size.width,
                ),
                color,
                &data[start..start + width],
            );
        }
    }

    /// Fill then stroke, each as its own coverage pass.
    ///
    /// Two passes rather than one because they are two colors: a mask carries
    /// coverage, not paint, so a shape that is filled *and* stroked needs one
    /// mask per color. The stroke pass rasterizes the stroke **outline** — an
    /// ordinary filled path — which is how tiny-skia strokes internally too.
    fn draw<T: Blitter>(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        path: &tiny_skia::Path,
        style: &DrawStyle<T::Color>,
    ) {
        if let Some(fill) = style.fill {
            self.emit(cx, path, fill);
        }
        if let (Some(color), width @ 1..) = (style.stroke, style.stroke_width) {
            // TODO: `StrokeAlignment` is not expressible in tiny-skia, which
            // always strokes centred on the path. Implementing Inside/Outside
            // means offsetting the path first. Recorded rather than silently
            // ignored — it is exactly the kind of *parameter semantics*
            // divergence the post-refactor rasterizer audit has to collect.
            let mut stroke = Stroke::default();
            stroke.width = width as f32;
            stroke.line_cap = tiny_skia::LineCap::Round;
            if let Some(outline) = self.stroker.stroke(path, &stroke, 1.0) {
                self.emit(cx, &outline, color);
            }
        }
    }
}

impl<T: Blitter> Rasterizer<T> for TinySkiaRasterizer {
    fn line(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        from: Point,
        to: Point,
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        path.move_to(from.x as f32, from.y as f32);
        path.line_to(to.x as f32, to.y as f32);
        // A line has no interior; only the stroke pass can produce anything, and
        // `Mask::fill_path` refuses a zero-area path anyway.
        if let Some(path) = path.finish() {
            self.draw(cx, &path, &DrawStyle { fill: None, ..*style });
        }
    }

    fn rect(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        rect: Rect,
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        if let Some(r) = tiny_skia::Rect::from_xywh(
            rect.top_left.x as f32,
            rect.top_left.y as f32,
            rect.size.width as f32,
            rect.size.height as f32,
        ) {
            path.push_rect(r);
        }
        if let Some(path) = path.finish() {
            self.draw(cx, &path, style);
        }
    }

    fn rounded_rect(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        rect: Rect,
        corners: CornerRadii,
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        path.rounded_rect(rect, corners.clamp_for(rect.size));
        if let Some(path) = path.finish() {
            self.draw(cx, &path, style);
        }
    }

    fn circle(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        top_left: Point,
        diameter: u32,
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        path.push_circle(
            top_left.x as f32 + diameter as f32 / 2.0,
            top_left.y as f32 + diameter as f32 / 2.0,
            diameter as f32 / 2.0,
        );
        if let Some(path) = path.finish() {
            self.draw(cx, &path, style);
        }
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
        let mut path = PathBuilder::new();
        path.arc(top_left, diameter, start, sweep);
        if let Some(path) = path.finish() {
            // An arc is a curve: stroke only, matching `scan::arc` and the
            // parameter semantics on the trait.
            self.draw(cx, &path, &DrawStyle { fill: None, ..*style });
        }
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
        let radius = diameter as f32 / 2.0;
        let center = tiny_skia::Point::from_xy(
            top_left.x as f32 + radius,
            top_left.y as f32 + radius,
        );
        let mut path = PathBuilder::new();
        path.move_to(center.x, center.y);
        path.arc(top_left, diameter, start, sweep);
        path.line_to(center.x, center.y);
        path.close();
        if let Some(path) = path.finish() {
            self.draw(cx, &path, style);
        }
    }

    fn ellipse(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        bounding_box: Rect,
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        if let Some(r) = tiny_skia::Rect::from_xywh(
            bounding_box.top_left.x as f32,
            bounding_box.top_left.y as f32,
            bounding_box.size.width as f32,
            bounding_box.size.height as f32,
        ) {
            path.push_oval(r);
        }
        if let Some(path) = path.finish() {
            self.draw(cx, &path, style);
        }
    }

    fn polygon(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        points: &[Point],
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        if let Some(first) = points.first() {
            path.move_to(first.x as f32, first.y as f32);
            for point in &points[1..] {
                path.line_to(point.x as f32, point.y as f32);
            }
            path.close();
        }
        if let Some(path) = path.finish() {
            self.draw(cx, &path, style);
        }
    }

    fn path(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        path: &Path,
        style: &DrawStyle<T::Color>,
    ) {
        let ts: tiny_skia::Path = path.clone().into();
        self.draw(cx, &ts, style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        blitter::pixmap::PixmapBlitter,
        geometry::Size,
        region::Unbounded,
        renderer::{RasterRenderer, Renderer},
    };
    use tiny_skia::Pixmap;

    /// The pixmap is the TEST's, as it is any caller's — nothing in rsact
    /// allocates one.
    ///
    /// White, because a fresh tiny-skia canvas is **opaque white** rather than
    /// transparent: an assertion built on `alpha != 0` is true on every pixel of
    /// an untouched surface and proves nothing. Every check below looks for
    /// pixels that are *not* white.
    fn pixmap(size: Size) -> Pixmap {
        let mut p = Pixmap::new(size.width, size.height).unwrap();
        p.fill(tiny_skia::Color::WHITE);
        p
    }

    type Skia = RasterRenderer<TinySkiaRasterizer, PixmapBlitter, Unbounded>;

    fn renderer(size: Size) -> Skia {
        Skia::with_blitter(
            TinySkiaRasterizer::new(),
            size,
            PixmapBlitter::new(pixmap(size)),
        )
        .unwrap()
    }

    fn inked(p: &Pixmap) -> usize {
        p.pixels()
            .iter()
            .filter(|c| (c.red(), c.green(), c.blue()) != (255, 255, 255))
            .count()
    }

    /// **The point of making tiny-skia an L2 rasterizer**, as a test.
    ///
    /// A `Mask` is colorless, so coverage is produced once and blended once — in
    /// *our* blitter. What that buys is anti-aliasing over storage tiny-skia has
    /// never heard of, and this is the check: partial coverage must reach a
    /// blitter as partial coverage, not as a thresholded on/off.
    ///
    /// Asserted on a diagonal, because an axis-aligned edge has no partially
    /// covered pixels to produce.
    #[test]
    fn coverage_reaches_the_blitter_as_coverage() {
        use crate::blitter::{Blitter, Span};
        use crate::geometry::{Point, Rect};

        /// Records the coverage values it is handed, and nothing else.
        struct Coverage {
            bounds: Rect,
            seen: alloc::vec::Vec<u8>,
        }
        impl Blitter for Coverage {
            type Color = tiny_skia::Color;
            fn bounds(&self) -> Rect {
                self.bounds
            }
            fn capacity(&self) -> Option<usize> {
                None
            }
            fn fill_span(&mut self, span: Span, _color: Self::Color) {
                // A thresholding blitter would arrive here instead; record it as
                // fully-covered so the assertion below can tell them apart.
                self.seen.extend(core::iter::repeat_n(255, span.len()));
            }
            fn blend_span(
                &mut self,
                _span: Span,
                _color: Self::Color,
                coverage: &[u8],
            ) {
                self.seen.extend_from_slice(coverage);
            }
            fn begin_region(
                &mut self,
                region: Rect,
            ) -> crate::renderer::RenderResult {
                self.bounds = region;
                Ok(())
            }
        }

        let bounds = Rect::new(Point::zero(), Size::new_equal(32));
        let mut rec = Coverage { bounds, seen: alloc::vec::Vec::new() };
        {
            let mut cx = RasterCtx::new(&mut rec, bounds);
            TinySkiaRasterizer::new().polygon(
                &mut cx,
                &[Point::new(2, 2), Point::new(29, 8), Point::new(8, 29)],
                &DrawStyle::default().fill(tiny_skia::Color::BLACK),
            );
        }

        let partial = rec.seen.iter().filter(|c| **c > 0 && **c < 255).count();
        assert!(
            partial > 10,
            "only {partial} pixels arrived with partial coverage — the \
             rasterizer is thresholding, not anti-aliasing, and the whole \
             reason it is L2 rather than a fused backend is gone"
        );
    }

    /// The same rasterizer over a **framebuffer**, which the fused tiny-skia API
    /// could never do: `Mask` carries no color, so `impl<T: Blitter>` with no
    /// bound on `T::Color` makes its anti-aliasing available over Rgb888 storage
    /// as readily as over a `Pixmap`.
    #[cfg(feature = "embedded-graphics")]
    #[test]
    fn tiny_skias_anti_aliasing_works_over_a_framebuffer() {
        use crate::{
            blitter::framebuf::FramebufBlitter,
            color::Color,
            framebuf::PackedColor,
            geometry::{Point, Rect},
        };
        use embedded_graphics::pixelcolor::Rgb888;

        let size = Size::new_equal(32);
        // `begin_region` primes the target with the background, so the buffer's
        // initial contents do not matter.
        let buf: &'static mut [u32] = alloc::vec![0u32; 32 * 32].leak();
        let mut r =
            RasterRenderer::<_, FramebufBlitter<Rgb888, _>, Unbounded>::with_blitter(
                TinySkiaRasterizer::new(),
                size,
                FramebufBlitter::new(buf),
            )
            .unwrap();
        // A blitter is aimed by `begin_region` and by nothing else.
        Renderer::begin_region(&mut r, Rect::new(Point::zero(), size)).unwrap();
        Renderer::polygon(
            &mut r,
            &[Point::new(2, 2), Point::new(29, 8), Point::new(8, 29)],
            &DrawStyle::default().fill(<Rgb888 as Color>::default_foreground()),
        )
        .unwrap();

        let (_, blitter) = r.detach();
        let (units, _) = blitter.into_storage();
        let bg = <Rgb888 as Color>::default_background().into_storage();
        let fg = <Rgb888 as Color>::default_foreground().into_storage();
        let painted = units.iter().filter(|u| **u != bg).count();
        let blended = units.iter().filter(|u| **u != bg && **u != fg).count();
        assert!(painted > 200, "the triangle painted only {painted} pixels");
        assert!(
            blended > 10,
            "only {blended} pixels are neither background nor foreground — the \
             coverage was thresholded on its way into the framebuffer"
        );
    }

    /// WS6.4d: a pixmap smaller than the frame paints the same picture as a
    /// full-frame one.
    ///
    /// It catches the class of bug the op logs cannot: the rebase is now
    /// `PixmapBlitter`'s addressing rather than a `Transform`, and getting a
    /// sign or a stride wrong yields a plausible image with an intact log.
    #[test]
    fn a_partial_pixmap_paints_what_a_full_one_would() {
        use crate::geometry::{Point, Rect};

        const W: u32 = 48;
        const H: u32 = 48;
        const BAND: u32 = 12;
        let viewport = Size::new(W, H);

        fn content<R: Renderer<Color = tiny_skia::Color>>(r: &mut R) {
            Renderer::fill_solid(
                r,
                Rect::new(Point::new(4, 6), Size::new(30, 20)),
                tiny_skia::Color::BLACK,
            )
            .unwrap();
            Renderer::rect(
                r,
                Rect::new(Point::new(2, 2), Size::new(44, 44)),
                &DrawStyle::default()
                    .stroke(tiny_skia::Color::from_rgba8(0, 160, 0, 255))
                    .stroke_width(2),
            )
            .unwrap();
            for i in 0..40i32 {
                Renderer::pixel(
                    r,
                    Point::new(i, 47 - i),
                    tiny_skia::Color::from_rgba8(200, 0, 0, 255),
                )
                .unwrap();
            }
        }

        let mut full = renderer(viewport);
        content(&mut full);
        let (_, blitter) = full.detach();
        let (reference, _) = blitter.into_pixmap().unwrap();

        let mut composed = pixmap(viewport);
        let band = Size::new(W, BAND);
        let mut tiled = Skia::with_blitter(
            TinySkiaRasterizer::new(),
            viewport,
            PixmapBlitter::new(pixmap(band)),
        )
        .unwrap();
        let mut spare = pixmap(band);

        for i in 0..(H / BAND) as i32 {
            let region = Rect::new(Point::new(0, i * BAND as i32), band);
            tiled.begin_region(region).unwrap();
            content(&mut tiled);
            tiled.end_region().unwrap();

            let (parked, blitter) = tiled.detach();
            let (tile, at) = blitter.into_pixmap().unwrap();
            // The caller's blit: raw pixels plus where they go.
            for row in 0..at.size.height as usize {
                for col in 0..at.size.width as usize {
                    let src = tile.pixels()[row * at.size.width as usize + col];
                    let x = at.top_left.x as usize + col;
                    let y = at.top_left.y as usize + row;
                    composed.pixels_mut()[y * W as usize + x] = src;
                }
            }
            tiled = parked.attach(PixmapBlitter::new(spare)).unwrap();
            spare = tile;
        }

        assert!(
            inked(&reference) > (W * H) as usize / 8,
            "the reference frame painted only {} of {} pixels",
            inked(&reference),
            W * H
        );
        let mismatches: alloc::vec::Vec<usize> = (0..(W * H) as usize)
            .filter(|&i| reference.pixels()[i] != composed.pixels()[i])
            .collect();
        assert!(
            mismatches.is_empty(),
            "{} of {} pixels differ between a full pixmap and a banded one; \
             first at ({}, {})",
            mismatches.len(),
            W * H,
            mismatches[0] % W as usize,
            mismatches[0] / W as usize,
        );
    }

    /// WS6.4d: a pixmap's bound is a **byte budget**, not a shape — the storage
    /// is re-strided per region, so any region fitting the bytes is painted.
    ///
    /// This replaced a test asserting the opposite: a 64×8 pixmap used to
    /// *refuse* a 32×16 region on the grounds that a pixmap has a fixed
    /// `width()`. It does — but `take`/`from_vec` let the same allocation be
    /// re-shaped, and both regions are 512 pixels.
    #[test]
    fn a_pixmap_is_reshaped_per_region_not_bound_to_its_shape() {
        use crate::{
            blitter::Blitter,
            geometry::{Point, Rect},
        };

        // 64x8 = 512 px = 2048 bytes.
        let mut b = PixmapBlitter::new(pixmap(Size::new(64, 8)));
        let budget = b.capacity();

        let square = Rect::new(Point::new(8, 16), Size::new(32, 16));
        assert!(b.begin_region(square).is_ok(), "same bytes, other shape");
        assert_eq!(b.bounds(), square);

        // Paint at the region's far corner: it only lands if the stride
        // followed the reshape.
        b.pixel(Point::new(39, 31), tiny_skia::Color::BLACK);
        let (tile, at) = b.into_pixmap().unwrap();
        assert_eq!(at, square);
        let corner = tile.pixels()[15 * 32 + 31];
        assert_ne!(
            (corner.red(), corner.green(), corner.blue()),
            (255, 255, 255),
            "the bottom-right pixel of a reshaped region did not land"
        );

        // A narrow tall region — the case a fixed-width pixmap could never hold.
        let mut b = PixmapBlitter::new(pixmap(Size::new(64, 8)));
        assert!(
            b.begin_region(Rect::new(Point::zero(), Size::new(4, 128)))
                .is_ok()
        );
        assert_eq!(
            b.capacity(),
            budget,
            "the caller's allocation was replaced"
        );
    }

    /// The bound still bites: a region needing more bytes than were lent is
    /// refused rather than silently reallocating behind the caller.
    #[test]
    fn a_region_over_the_byte_budget_is_refused() {
        use crate::{
            blitter::Blitter,
            geometry::{Point, Rect},
        };
        let mut b = PixmapBlitter::new(pixmap(Size::new(64, 8)));
        // 2052 bytes wanted against 2048 lent — one pixel too many.
        let over = Rect::new(Point::zero(), Size::new(513, 1));
        assert!(
            b.begin_region(over).is_err(),
            "a region over the lent byte budget must be refused; reallocating \
             would quietly take ownership of storage that is not ours"
        );
    }
}
