//! Turning geometry into runs of pixels.
//!
//! A [`Rasterizer`] scan-converts one primitive and emits it through a
//! [`RasterCtx`], the only thing it is handed and the reason it cannot write
//! outside the clip.
//!
//! `RasterCtx::new` is `pub(crate)`, so a downstream crate can implement this
//! trait but cannot call one; use [`Renderer`](crate::renderer::Renderer) to
//! drive rendering. Every default body delegates to [`crate::scan`].

use crate::{
    blitter::{Blitter, Span},
    geometry::{Angle, CornerRadii, Point, Rect, Size},
    image::DrawImage,
    path::Path,
    style::DrawStyle,
};

/// Where a [`Rasterizer`] emits its pixels.
///
/// Every method clips, so geometry outside [`clip()`](Self::clip) is discarded
/// rather than forbidden — bound your loops by it where that is cheap.
///
/// [`span`](Self::span) and [`rect`](Self::rect) are the fast paths;
/// [`run`](Self::run) takes a color per pixel and [`blend`](Self::blend)
/// coverage. [`reborrow`](Self::reborrow) passes it to another primitive.
pub struct RasterCtx<'a, T: Blitter> {
    blitter: &'a mut T,
    clip: Rect,
}

impl<'a, T: Blitter> RasterCtx<'a, T> {
    /// Narrows `clip` to the blitter's bounds, so no renderer can widen one
    /// past its own surface.
    pub(crate) fn new(blitter: &'a mut T, clip: Rect) -> Self {
        let clip = clip.intersection(&blitter.bounds());
        Self { blitter, clip }
    }

    /// Bound your loops with this and nothing is thrown away.
    pub fn clip(&self) -> Rect {
        self.clip
    }

    /// For delegating to another primitive.
    pub fn reborrow(&mut self) -> RasterCtx<'_, T> {
        RasterCtx { blitter: &mut *self.blitter, clip: self.clip }
    }

    pub fn span(&mut self, span: Span, color: T::Color) {
        if let Some((span, _)) = span.clip_to(&self.clip) {
            self.blitter.fill_span(span, color);
        }
    }

    pub fn pixel(&mut self, p: Point, color: T::Color) {
        if self.clip.contains(p) {
            self.blitter.pixel(p, color);
        }
    }

    pub fn rect(&mut self, rect: Rect, color: T::Color) {
        let rect = rect.intersection(&self.clip);
        if !rect.is_zero_sized() {
            self.blitter.fill_rect(rect, color);
        }
    }

    /// Slices `colors` in step with the clip.
    pub fn run(&mut self, span: Span, colors: &[T::Color]) {
        debug_assert_eq!(colors.len(), span.len());
        if let Some((clipped, offset)) = span.clip_to(&self.clip) {
            self.blitter
                .fill_run(clipped, &colors[offset..offset + clipped.len()]);
        }
    }

    pub fn blend(&mut self, span: Span, color: T::Color, coverage: &[u8]) {
        debug_assert_eq!(coverage.len(), span.len());
        if let Some((clipped, offset)) = span.clip_to(&self.clip) {
            self.blitter.blend_span(
                clipped,
                color,
                &coverage[offset..offset + clipped.len()],
            );
        }
    }
}

/// Geometry into spans.
///
/// The blitter arrives per call rather than being held, so a rasterizer may keep
/// caches — a coverage line, a `Mask`, a glyph atlas — across targets.
///
/// **Nothing is required**: every method has a default that draws, so
/// `impl Rasterizer for X {}` works and you override only what you have better
/// algorithms for. A default must decompose *exactly* onto the other methods or
/// go through [`path`](Self::path) — never approximate one shape with another,
/// which renders a plausible wrong image.
///
/// Rasterizers need not agree pixel for pixel, but must agree on **parameter
/// semantics**:
///
/// - **Angle zero is `+x`, and a positive sweep runs toward `+y`** — clockwise
///   on screen, because `y` grows downward. Same convention as
///   [`Line::with_angle`](crate::primitives::line::Line::with_angle).
/// - **`StrokeAlignment` is relative to the path**, and a `Center` stroke of odd
///   width puts the extra pixel *inside*.
/// - **A corner radius is the ellipse's semi-axis pair**, not a diameter, and is
///   clamped to the rect by `CornerRadii::clamp_for`.
/// - **An arc is a curve, so `style.fill` is ignored.** The filled-region
///   primitives are [`sector`](Self::sector) (center-bounded) and
///   [`ellipse`](Self::ellipse) (closed).
pub trait Rasterizer<T: Blitter> {
    fn fill(&mut self, cx: &mut RasterCtx<'_, T>, rect: Rect, color: T::Color) {
        crate::scan::fill(cx, rect, color)
    }

    fn pixel(&mut self, cx: &mut RasterCtx<'_, T>, p: Point, color: T::Color) {
        cx.pixel(p, color)
    }

    fn line(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        from: Point,
        to: Point,
        style: &DrawStyle<T::Color>,
    ) {
        crate::scan::line(cx, from, to, style)
    }

    fn rect(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        rect: Rect,
        style: &DrawStyle<T::Color>,
    ) {
        crate::scan::rect(cx, rect, style)
    }

    fn rounded_rect(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        rect: Rect,
        corners: CornerRadii,
        style: &DrawStyle<T::Color>,
    ) {
        crate::scan::rounded_rect(cx, rect, corners, style)
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
        crate::scan::arc(cx, top_left, diameter, start, sweep, style)
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
        crate::scan::sector(cx, top_left, diameter, start, sweep, style)
    }

    fn ellipse(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        bounding_box: Rect,
        style: &DrawStyle<T::Color>,
    ) {
        crate::scan::ellipse(cx, bounding_box, style)
    }

    fn polygon(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        points: &[Point],
        style: &DrawStyle<T::Color>,
    ) {
        crate::scan::polygon(cx, points, style)
    }

    fn path(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        path: &Path,
        style: &DrawStyle<T::Color>,
    ) {
        crate::scan::path(cx, path, style)
    }

    fn image(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        image: DrawImage<'_, T::Color>,
    ) {
        crate::scan::image(cx, image)
    }

    /// A circle is an ellipse with equal axes, dispatched through `self` so an
    /// overridden `ellipse` carries it.
    ///
    /// **Not `self.arc` with a full sweep** — an arc is a curve with no
    /// interior, so it ignores `style.fill` and would render a filled circle as
    /// an outline.
    fn circle(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        top_left: Point,
        diameter: u32,
        style: &DrawStyle<T::Color>,
    ) {
        self.ellipse(cx, Rect::new(top_left, Size::new_equal(diameter)), style)
    }

    // TODO: `glyphs`, blitting the font layer's coverage bitmap row by row
    // through `cx.blend`. Until then text goes per-pixel via `DrawTargetProxy`.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        image::{DrawImage, ImageOwned},
        path::PathSegment,
        renderer::{NullColor, RenderResult},
        style::DrawStyle,
    };
    use alloc::{boxed::Box, vec, vec::Vec};

    /// Records where it was written, and nothing else. Its `bounds` is smaller
    /// than the clips the tests pass, so they exercise the narrowing too.
    struct Recorder {
        bounds: Rect,
        writes: Vec<Point>,
    }

    impl Recorder {
        fn new(bounds: Rect) -> Self {
            Self { bounds, writes: Vec::new() }
        }
    }

    impl Blitter for Recorder {
        type Color = NullColor;

        fn bounds(&self) -> Rect {
            self.bounds
        }

        fn capacity(&self) -> Option<usize> {
            None
        }

        fn fill_span(&mut self, span: Span, _color: NullColor) {
            for x in span.x_range() {
                self.writes.push(Point::new(x, span.y));
            }
        }

        fn begin_region(&mut self, region: Rect) -> RenderResult {
            self.bounds = region;
            Ok(())
        }
    }

    fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect::new(Point::new(x, y), Size::new(w, h))
    }

    /// No sequence of `RasterCtx` calls can put a pixel outside
    /// `clip ∩ bounds`, however far outside both the geometry it is given lies.
    /// Every method is called here with deliberately absurd coordinates.
    #[test]
    fn nothing_escapes_the_clip_however_hard_a_rasterizer_sprays() {
        let bounds = r(10, 10, 20, 20);
        let clip = r(12, 12, 8, 8);
        let mut rec = Recorder::new(bounds);
        {
            let mut cx = RasterCtx::new(&mut rec, clip);
            cx.rect(r(-100, -100, 1000, 1000), NullColor);
            cx.span(Span::new(13, -50, 500), NullColor);
            cx.span(Span::new(-5, 12, 8), NullColor);
            cx.pixel(Point::new(0, 0), NullColor);
            cx.pixel(Point::new(13, 13), NullColor);
            let colors = vec![NullColor; 500];
            cx.run(Span::new(14, -50, 500), &colors);
            let coverage = vec![255u8; 500];
            cx.blend(Span::new(15, -50, 500), NullColor, &coverage);
            // Through a reborrow, which is how every composing default works.
            cx.reborrow().rect(r(-100, -100, 1000, 1000), NullColor);
        }

        assert!(!rec.writes.is_empty(), "the test sprayed and nothing landed");
        let escaped: Vec<_> =
            rec.writes.iter().filter(|p| !clip.contains(**p)).collect();
        assert!(
            escaped.is_empty(),
            "{} writes escaped the clip; first {:?}",
            escaped.len(),
            escaped.first()
        );
    }

    /// `RasterCtx::new` narrows the clip to the blitter's bounds, so a renderer
    /// handing over a clip wider than its surface cannot widen anything.
    #[test]
    fn a_clip_wider_than_the_blitter_is_narrowed_on_construction() {
        let bounds = r(10, 10, 4, 4);
        let mut rec = Recorder::new(bounds);
        let mut cx = RasterCtx::new(&mut rec, r(0, 0, 100, 100));
        assert_eq!(cx.clip(), bounds);
        cx.rect(r(0, 0, 100, 100), NullColor);
        assert_eq!(rec.writes.len(), 16);
    }

    /// `Plain` overrides nothing, so every call runs a default — and every
    /// default must put pixels somewhere. Catches a primitive silently becoming
    /// a no-op.
    #[test]
    fn every_default_draws() {
        struct Plain;
        impl<T: Blitter> Rasterizer<T> for Plain {}

        let bounds = r(0, 0, 40, 40);
        let style = DrawStyle::default()
            .fill(NullColor)
            .stroke(NullColor)
            .stroke_width(2);

        let drew = |name: &str,
                    f: &mut dyn FnMut(
            &mut Plain,
            &mut RasterCtx<'_, Recorder>,
        )| {
            let mut rec = Recorder::new(bounds);
            {
                let mut cx = RasterCtx::new(&mut rec, bounds);
                f(&mut Plain, &mut cx);
            }
            assert!(
                !rec.writes.is_empty(),
                "`Rasterizer::{name}`'s default drew nothing — a default that \
                 does not draw is the silent no-op this trait exists to forbid"
            );
        };

        drew("fill", &mut |p, cx| p.fill(cx, r(4, 4, 10, 10), NullColor));
        drew("pixel", &mut |p, cx| p.pixel(cx, Point::new(5, 5), NullColor));
        drew("line", &mut |p, cx| {
            p.line(cx, Point::new(2, 2), Point::new(30, 20), &style)
        });
        drew("line (axis-aligned)", &mut |p, cx| {
            p.line(cx, Point::new(2, 2), Point::new(30, 2), &style)
        });
        drew("rect", &mut |p, cx| p.rect(cx, r(4, 4, 20, 12), &style));
        drew("rounded_rect", &mut |p, cx| {
            p.rounded_rect(
                cx,
                r(4, 4, 20, 20),
                CornerRadii::new_equal_radius(5),
                &style,
            )
        });
        drew("circle", &mut |p, cx| p.circle(cx, Point::new(4, 4), 20, &style));
        drew("arc", &mut |p, cx| {
            p.arc(
                cx,
                Point::new(4, 4),
                20,
                Angle::ZERO,
                Angle::HALF_CIRCLE,
                &style,
            )
        });
        drew("sector", &mut |p, cx| {
            p.sector(
                cx,
                Point::new(4, 4),
                20,
                Angle::ZERO,
                Angle::HALF_CIRCLE,
                &style,
            )
        });
        drew("ellipse", &mut |p, cx| p.ellipse(cx, r(4, 4, 24, 14), &style));
        drew("polygon", &mut |p, cx| {
            p.polygon(
                cx,
                &[Point::new(5, 5), Point::new(30, 8), Point::new(20, 30)],
                &style,
            )
        });
        drew("path", &mut |p, cx| {
            let path = Path {
                segments: vec![
                    PathSegment::MoveTo(Point::new(4, 4)),
                    PathSegment::LineTo(Point::new(30, 10)),
                    PathSegment::ArcTo {
                        center: Point::new(20, 20),
                        radius: 8,
                        start: Angle::ZERO,
                        sweep: Angle::HALF_CIRCLE,
                    },
                    PathSegment::Close,
                ],
            };
            p.path(cx, &path, &style)
        });
        drew("image", &mut |p, cx| {
            let owned = ImageOwned::<NullColor>::new(
                vec![255u8; 8 * 8 * 4].into_boxed_slice() as Box<[u8]>,
                Size::new_equal(8),
            );
            p.image(cx, DrawImage::new(owned.as_ref(), Point::new(4, 4)))
        });
    }

    /// A **filled** circle must be filled. Defaulting `circle` to a full-sweep
    /// `arc` would draw an outline instead, an arc having no interior.
    #[test]
    fn a_filled_circle_is_filled_and_a_filled_arc_is_not_a_circle() {
        struct Plain;
        impl<T: Blitter> Rasterizer<T> for Plain {}

        let bounds = r(0, 0, 40, 40);
        let filled = DrawStyle::default().fill(NullColor);

        let mut circle = Recorder::new(bounds);
        {
            let mut cx = RasterCtx::new(&mut circle, bounds);
            Plain.circle(&mut cx, Point::new(4, 4), 24, &filled);
        }
        // A filled 24px circle covers ~452 pixels; an outline would be ~75.
        assert!(
            circle.writes.len() > 300,
            "a filled circle painted only {} pixels — that is an outline, not \
             a disc",
            circle.writes.len()
        );
        // And the centre is inside it, which an outline's is not.
        assert!(circle.writes.contains(&Point::new(16, 16)));

        // An arc with the same style paints nothing: `fill` is ignored.
        let mut arc = Recorder::new(bounds);
        {
            let mut cx = RasterCtx::new(&mut arc, bounds);
            Plain.arc(
                &mut cx,
                Point::new(4, 4),
                24,
                Angle::ZERO,
                Angle::FULL_CIRCLE,
                &filled,
            );
        }
        assert!(
            arc.writes.is_empty(),
            "an arc is a curve: `style.fill` must be ignored, not treated as a \
             disc"
        );
    }
}
