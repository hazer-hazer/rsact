//! Anti-aliased rasterization, by tiny-skia.

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
/// Built on [`Mask::fill_path`] rather than tiny-skia's fused painting API, so
/// coverage is produced once and blended once by the blitter — and no color is
/// pinned, which puts its anti-aliasing over an Rgb565 framebuffer as readily as
/// over a `Pixmap`.
///
/// The mask is grow-only and keyed on the **primitive** — the high-water mark of
/// every `round_out(path.bounds()) ∩ clip` seen so far, not of every clip — so a
/// page of small widgets never allocates a viewport-sized one. Rows are read at
/// the *allocated* width, not the primitive's.
///
/// [`fill`](Rasterizer::fill), [`pixel`](Rasterizer::pixel) and
/// [`image`](Rasterizer::image) are inherited — none has an edge to
/// anti-alias.
pub struct TinySkiaRasterizer {
    mask: Option<Mask>,
    /// The allocated size: the high-water mark of every primitive's paint box.
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
    /// Everything is bounded by the **primitive**, not the clip: the mask's
    /// `(0, 0)` is the top-left of `round_out(path.bounds()) ∩ clip`, only those
    /// rows are cleared, and only those spans are blended. Bounding it by the
    /// clip instead — which is the *region* during a paint pass — cost a 16x16
    /// circle 129600 coverage bytes on a 480x270 page, per primitive and per
    /// pass, to ink 301 pixels.
    ///
    /// Sound because `bounds()` is the control-point hull (over-covering a curve
    /// rather than under-covering it) and `round_out` is floor/ceil, so the box
    /// includes every pixel an anti-aliased edge can touch. The failure mode if
    /// that were wrong is a dropped edge, which no op count shows, so
    /// `bounding_the_mask_does_not_change_the_picture` pins the pixels by value.
    fn emit<T: Blitter>(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        path: &tiny_skia::Path,
        color: T::Color,
    ) {
        let clip = cx.clip();
        // `round_out` widens a degenerate axis to 1 rather than returning
        // nothing, so `None` here means the coordinates overflowed `i32`.
        let Some(bounds) = path.bounds().round_out() else { return };
        let paint = Rect::new(
            Point::new(bounds.x(), bounds.y()),
            Size::new(bounds.width(), bounds.height()),
        )
        .intersection(&clip);
        // A primitive outside the clip now costs nothing. It used to allocate,
        // clone, transform and rasterize the path, then blend the whole clip.
        if paint.is_zero_sized() {
            return;
        }

        // Grow-only, and keyed on the primitive rather than the clip: a page of
        // small widgets never allocates a viewport-sized mask.
        if self.mask_size.width < paint.size.width
            || self.mask_size.height < paint.size.height
        {
            self.mask_size = Size::new(
                self.mask_size.width.max(paint.size.width),
                self.mask_size.height.max(paint.size.height),
            );
            self.mask = Mask::new(self.mask_size.width, self.mask_size.height);
        }
        let Some(mask) = self.mask.as_mut() else { return };

        let stride = mask.width() as usize;
        let width = paint.size.width as usize;
        let height = paint.size.height as usize;

        // `fill_path` draws on top of existing content, so without this every
        // shape would inherit the last one's edges. Only the rows about to be
        // read: `Mask::clear` zeroes the whole grow-only allocation, which is
        // the high-water mark of every primitive so far.
        let data = mask.data_mut();
        for row in 0..height {
            data[row * stride..row * stride + width].fill(0);
        }

        // An integer translate, so subpixel positions — and therefore the
        // anti-aliasing — are unchanged by the re-origin. `fill_path` clips to
        // the mask, which is what makes a path hanging off `paint`'s left or top
        // safe rather than wrapped.
        mask.fill_path(
            path,
            FillRule::Winding,
            true,
            Transform::from_translate(
                -paint.top_left.x as f32,
                -paint.top_left.y as f32,
            ),
        );

        let data = mask.data();
        for row in 0..height {
            let start = row * stride;
            cx.blend(
                Span::new(
                    paint.top_left.y + row as i32,
                    paint.top_left.x,
                    paint.size.width,
                ),
                color,
                &data[start..start + width],
            );
        }
    }

    /// Fill then stroke, one coverage pass each — a mask carries coverage, not
    /// paint, so two colors need two passes. The stroke pass fills the stroke's
    /// **outline**, which is how tiny-skia strokes internally too.
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
            // TODO: `StrokeAlignment` is ignored — tiny-skia always strokes
            // centred on the path, so Inside/Outside need the path offset first.
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
        // A line has no interior, so only the stroke pass can produce
        // anything.
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
        geometry::Size,
        region::Unbounded,
        renderer::{RasterRenderer, Renderer},
        tiny_skia::blitter::PixmapBlitter,
    };
    use tiny_skia::Pixmap;

    /// The pixmap is the test's, as it is any caller's — nothing in rsact
    /// allocates one.
    ///
    /// Filled white so the checks below can look for pixels that are *not*
    /// white. `Pixmap::new` alone is transparent black, against which an
    /// `alpha != 0` assertion holds on every untouched pixel and proves nothing.
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

    /// Partial coverage must reach the blitter *as* partial coverage, not
    /// thresholded to on/off — that is what anti-aliasing over foreign storage
    /// depends on.
    ///
    /// Asserted on a diagonal; an axis-aligned edge has no partially covered
    /// pixels to produce.
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

    /// Records which absolute pixels arrived with non-zero coverage, and how
    /// much per-pixel work reaching them cost.
    struct Traffic {
        bounds: Rect,
        inked: alloc::vec::Vec<Point>,
        blend_calls: usize,
        coverage_bytes: usize,
    }

    impl Traffic {
        fn new(bounds: Rect) -> Self {
            Self {
                bounds,
                inked: alloc::vec::Vec::new(),
                blend_calls: 0,
                coverage_bytes: 0,
            }
        }
    }

    impl Blitter for Traffic {
        type Color = tiny_skia::Color;
        fn bounds(&self) -> Rect {
            self.bounds
        }
        fn capacity(&self) -> Option<usize> {
            None
        }
        fn fill_span(&mut self, span: Span, _color: Self::Color) {
            for x in span.x_range() {
                self.inked.push(Point::new(x, span.y));
            }
        }
        fn blend_span(
            &mut self,
            span: Span,
            _color: Self::Color,
            coverage: &[u8],
        ) {
            self.blend_calls += 1;
            self.coverage_bytes += coverage.len();
            for (i, cov) in coverage.iter().enumerate() {
                if *cov != 0 {
                    self.inked.push(Point::new(span.x + i as i32, span.y));
                }
            }
        }
        fn begin_region(
            &mut self,
            region: Rect,
        ) -> crate::renderer::RenderResult {
            self.bounds = region;
            Ok(())
        }
    }

    /// The five shapes below, drawn once each, as (inked pixel count, checksum).
    ///
    /// A value golden rather than a file: the point is only that bounding
    /// `emit`'s work to the primitive did not change the picture, and a
    /// mismatch here says which shape moved.
    const PICTURE: [(&str, usize, u64); 5] = [
        ("circle", 301, 0x8020_bfe6_06dc_1e66),
        ("rounded_rect", 1168, 0x26ab_a746_3382_6118),
        ("line", 732, 0x2aca_3687_6b55_eeaa),
        ("sector", 875, 0x6f84_15ec_250e_ebf9),
        ("off the left edge", 620, 0x1ddd_19ba_ca13_4d58),
    ];

    /// Order-independent, position-sensitive: swapping two pixels' positions
    /// changes it, reordering the *reports* does not.
    fn checksum(inked: &[Point]) -> u64 {
        inked
            .iter()
            .map(|p| {
                let k = (p.y as i64 * 4096 + p.x as i64) as u64;
                k.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            })
            .fold(0u64, u64::wrapping_add)
    }

    /// **Bounding `emit` must not change the picture.**
    ///
    /// `emit` clears and blends the whole clip today, so a small shape in a
    /// large clip costs the clip's area. Bounding that to the primitive's own
    /// `round_out(path.bounds())` is only sound if tiny-skia never inks outside
    /// the bounds it reports — and the failure mode if it does is a thin dropped
    /// edge, which no op count would show. So the picture is pinned by value,
    /// captured before the bound existed.
    ///
    /// The clip is far larger than every shape, and one shape hangs off its
    /// left edge so the `bounds ∩ clip` intersection is exercised.
    #[test]
    fn bounding_the_mask_does_not_change_the_picture() {
        let clip = Rect::new(Point::zero(), Size::new(200, 120));
        let style = DrawStyle::default()
            .fill(tiny_skia::Color::BLACK)
            .stroke(tiny_skia::Color::BLACK)
            .stroke_width(3);

        type Draw = fn(
            &mut TinySkiaRasterizer,
            &mut RasterCtx<'_, Traffic>,
            &DrawStyle<tiny_skia::Color>,
        );
        // One case per path through `draw`: circle, rounded corners, a
        // stroke-only line, a curve with an interior, and a clipped shape.
        let cases: [Draw; 5] = [
            |r, cx, st| r.circle(cx, Point::new(20, 20), 16, st),
            |r, cx, st| {
                r.rounded_rect(
                    cx,
                    Rect::new(Point::new(60, 30), Size::new(40, 24)),
                    CornerRadii::new_equal(Size::new_equal(6)),
                    st,
                )
            },
            |r, cx, st| {
                r.line(cx, Point::new(10, 100), Point::new(190, 60), st)
            },
            |r, cx, st| {
                r.sector(
                    cx,
                    Point::new(120, 20),
                    50,
                    Angle::ZERO,
                    Angle::from_degrees(120.0),
                    st,
                )
            },
            |r, cx, st| r.circle(cx, Point::new(-10, 40), 30, st),
        ];

        let mut actual = alloc::vec::Vec::new();
        for (draw, (name, _, _)) in cases.iter().zip(PICTURE) {
            let mut traffic = Traffic::new(clip);
            let mut rasterizer = TinySkiaRasterizer::new();
            {
                let mut cx = RasterCtx::new(&mut traffic, clip);
                draw(&mut rasterizer, &mut cx, &style);
            }
            traffic.inked.sort_by_key(|p| (p.y, p.x));
            traffic.inked.dedup();
            actual.push((
                name,
                traffic.inked.len(),
                checksum(&traffic.inked),
                traffic.blend_calls,
                traffic.coverage_bytes,
            ));
        }

        for (name, inked, sum, calls, bytes) in &actual {
            std::eprintln!(
                "{name:<20} inked={inked:<6} checksum={sum:#018x} \
                 blend_calls={calls:<5} coverage_bytes={bytes}"
            );
        }

        for ((name, want_inked, want_sum), (_, got_inked, got_sum, _, _)) in
            PICTURE.iter().zip(&actual)
        {
            assert_eq!(
                (*got_inked, *got_sum),
                (*want_inked, *want_sum),
                "{name}: the picture changed"
            );
        }
    }

    /// **A primitive costs its own size, not the clip's.**
    ///
    /// `emit` used to clear the whole grow-only mask and blend every row of the
    /// clip at full clip width, per primitive and per pass. On a 200x120 clip a
    /// 16x16 circle therefore cost 240 `blend_span` calls over 48000 coverage
    /// bytes to ink 301 pixels — and the clip is the *region* during a paint
    /// pass, so on a whole-frame 480x270 page that is 129600 bytes per
    /// primitive.
    ///
    /// The bound is the primitive's own `round_out(path.bounds())`;
    /// `bounding_the_mask_does_not_change_the_picture` is what says it is the
    /// right one.
    #[test]
    fn a_primitive_costs_its_own_size_not_the_clips() {
        let clip = Rect::new(Point::zero(), Size::new(200, 120));
        // Fill only, so this is one pass and the arithmetic is checkable by
        // hand: a 16x16 circle is 16 rows, not the clip's 120.
        let style = DrawStyle::default().fill(tiny_skia::Color::BLACK);

        let mut traffic = Traffic::new(clip);
        let mut rasterizer = TinySkiaRasterizer::new();
        {
            let mut cx = RasterCtx::new(&mut traffic, clip);
            rasterizer.circle(&mut cx, Point::new(20, 20), 16, &style);
        }

        assert!(
            traffic.blend_calls <= 18,
            "a 16x16 circle took {} blend_span calls; bounded by its own \
             extent it is at most 16 rows plus a pixel of anti-aliased spill \
             on each side",
            traffic.blend_calls
        );
        assert!(
            traffic.coverage_bytes <= 18 * 18,
            "and {} coverage bytes, against 18*18 for its own box",
            traffic.coverage_bytes
        );
        assert!(
            !traffic.inked.is_empty(),
            "it must still have drawn something"
        );
    }

    /// The same rasterizer over a **framebuffer**, which the fused tiny-skia
    /// API could never do.
    #[cfg(feature = "embedded-graphics")]
    #[test]
    fn tiny_skias_anti_aliasing_works_over_a_framebuffer() {
        use crate::{
            blitter::FramebufBlitter,
            color::Color,
            framebuf::PackedColor,
            geometry::{Point, Rect},
        };
        use embedded_graphics::pixelcolor::Rgb888;

        let size = Size::new_equal(32);
        // Zeroed. `begin_region` aims and nothing more, so the background below
        // is what the anti-aliased edge composites against — which is the whole
        // subject of this test.
        let buf: &'static mut [u32] = alloc::vec![0u32; 32 * 32].leak();
        let mut r =
            RasterRenderer::<_, FramebufBlitter<Rgb888, _>, Unbounded>::with_blitter(
                TinySkiaRasterizer::new(),
                size,
                FramebufBlitter::new(buf),
            )
            .unwrap();
        // A blitter is aimed by `begin_region` and by nothing else.
        let frame = Rect::new(Point::zero(), size);
        Renderer::begin_region(&mut r, frame).unwrap();
        Renderer::fill_solid(
            &mut r,
            frame,
            <Rgb888 as Color>::default_background(),
        )
        .unwrap();
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

    /// A pixmap smaller than the frame paints the same picture as a full-frame
    /// one.
    ///
    /// Catches what an op log cannot: the rebase is `PixmapBlitter`'s
    /// addressing, and a wrong sign or stride yields a plausible image with an
    /// intact log.
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
            // The caller's background fill: pixmaps are recycled below, so from
            // the second band on the region arrives holding the previous one's
            // pixels.
            Renderer::fill_solid(&mut tiled, region, tiny_skia::Color::WHITE)
                .unwrap();
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

    /// A pixmap's bound is a **byte budget**, not a shape: the storage is
    /// re-strided per region, so any region fitting the bytes is painted.
    ///
    /// A pixmap does have a fixed `width()`, which suggests a 64×8 one should
    /// refuse a 32×16 region — but `take`/`from_vec` re-shape the same
    /// allocation, and both regions are 512 pixels.
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
