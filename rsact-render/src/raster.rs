//! **Layer 2 — geometry into spans.**
//!
//! A [`Rasterizer`] turns a primitive into runs of pixels and hands them to a
//! [`Blitter`] through a [`RasterCtx`], which is the only thing it is ever given
//! and the reason it cannot write outside the clip.
//!
//! **This is a crate-internal seam.** `RasterCtx::new` is `pub(crate)`, so a
//! downstream crate can write a `Blitter` but cannot drive a `Rasterizer`;
//! [`Renderer`](crate::renderer::Renderer) is the published backend seam, and
//! the extension promise binds once, there.
//!
//! The shared scan conversion every default delegates to is [`crate::scan`], a
//! sibling module rather than a child: this file is the *contract* (the clip
//! gate and the trait), that one is 600 lines of *algorithm*, and the two are
//! read for different reasons. The concrete rasterizers live in their backend's
//! module — `eg::rasterizer`, `tiny_skia::rasterizer` — so the tree is cut by
//! feature gate, not by layer.

use crate::{
    blitter::{Blitter, Span},
    geometry::{Angle, CornerRadii, Point, Rect, Size},
    image::DrawImage,
    path::Path,
    style::DrawStyle,
};

/// The **only** way a rasterizer touches a blitter.
///
/// Both fields are private and the constructor is `pub(crate)`, so a
/// [`Rasterizer`] impl has no path to `T`, and `clip ⊆ blitter.bounds()` holds
/// by construction.
///
/// | | enforced? |
/// |---|---|
/// | writing outside the blitter | **impossible** — private field |
/// | writing outside the clip | **impossible** — every method intersects |
/// | retargeting mid-primitive | **impossible** — `begin_region` is not here |
/// | *bounding your loops* by the clip | advisory — spray-and-clip is correct, slow |
///
/// The last row cannot be closed by types. It can be measured, and where it is
/// cheap it is simply taken: [`crate::scan::polygon`]'s fill bounds its scan by
/// [`clip()`](Self::clip) rather than by the polygon's own box.
pub struct RasterCtx<'a, T: Blitter> {
    blitter: &'a mut T,
    clip: Rect,
}

impl<'a, T: Blitter> RasterCtx<'a, T> {
    /// Only a renderer builds one — this is where `clip ⊆ bounds` is made true.
    ///
    /// The intersection is what makes the guarantee *structural* rather than a
    /// convention every L1 has to remember, which is why it is here and not at
    /// the call site.
    pub(crate) fn new(blitter: &'a mut T, clip: Rect) -> Self {
        let clip = clip.intersection(&blitter.bounds());
        Self { blitter, clip }
    }

    /// Advisory: bound your loops with this and nothing is thrown away.
    pub fn clip(&self) -> Rect {
        self.clip
    }

    /// For delegating to another primitive — every default body that composes
    /// needs it.
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

    /// Clipping slices per-pixel data in step — [`Span::clip_to`] returns the
    /// offset so that arithmetic exists once. Getting it wrong shifts an image
    /// by a few pixels, which is a plausible picture rather than a failure.
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

/// Geometry into spans. **Stateless about *where* it draws** — the blitter
/// arrives per call.
///
/// That is required, not symmetry: a blitter that cannot read its own pixels
/// needs anti-aliasing composited into a one-scanline scratch and *then*
/// emitted, so two blitters are live inside one primitive call. It also lets
/// caches — a coverage line, a `Mask`, a glyph atlas — survive a blitter swap.
///
/// **One method per primitive, never a `PrimitiveKind` match.** A new variant
/// breaks every downstream `match` on a version bump, and the `_ =>` wildcard
/// that silences it turns every future primitive into a permanent silent no-op.
/// This repo demonstrates the visible alternative twice over: `polygon` and
/// `image` are logged no-ops in the embedded-graphics backend precisely because
/// they are named methods, so the hole is findable. The `cx` repetition is the
/// price; it is load-bearing (it is *why* the clip cannot be escaped) and
/// confined to this definition.
///
/// # No primitive is ever unsupported
///
/// Every method has a default that **draws**. Consequences:
///
/// - A new rasterizer is `impl Rasterizer for X {}` plus the overrides it has
///   better algorithms for.
/// - A **new primitive** must arrive with an exact decomposition onto the
///   existing set — and [`path`](Self::path) makes that always possible, since
///   every 2D shape is a path. A primitive that cannot be expressed as one is a
///   new *capability*, not new geometry, and does not go on this trait.
/// - A default must never be a **lookalike** (a squircle drawn as a rounded
///   rect): that renders a plausible wrong image, the worst failure mode in this
///   design. Exact-or-via-`path`, never approximate.
///
/// **"Exact" means geometry, not pixels.** A default calls back through `self`
/// where it composes, so it inherits *that rasterizer's* quality automatically.
/// Rasterizer *parity* is explicitly not a goal — differing output is the reason
/// there is more than one. What must agree between rasterizers is only
/// **parameter semantics**, and the ones settled so far are:
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
///
/// The trait therefore requires **nothing**. [`fill`](Self::fill) and
/// [`pixel`](Self::pixel) are listed first because every override chain bottoms
/// out in them, and `fill` is style-free because clears, backgrounds and region
/// priming must not pay for style resolution.
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

    /// Exact: a circle is an ellipse with equal axes.
    ///
    /// **Not `self.arc` with a full sweep**, which the design sketch proposed
    /// and which is wrong for a reason worth keeping: an arc is a *curve*, so it
    /// has no interior and ignores `style.fill`. Routing `circle` through it
    /// would render a filled circle as an outline — a plausible wrong image,
    /// which is precisely the failure mode "no lookalike defaults" forbids.
    ///
    /// Through `self`, so a rasterizer that overrides `ellipse` gets a circle of
    /// the same quality for free. `EgRasterizer` overrides this anyway:
    /// embedded-graphics has its own circle algorithm and taking any default
    /// would silently discard it.
    fn circle(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        top_left: Point,
        diameter: u32,
        style: &DrawStyle<T::Color>,
    ) {
        self.ellipse(cx, Rect::new(top_left, Size::new_equal(diameter)), style)
    }

    // `glyphs` arrives with WS15. Its default is not geometry: the font layer
    // supplies a coverage bitmap, so the default blits it row by row through
    // `cx.blend`. Until then text keeps its per-pixel path through the existing
    // `DrawTargetProxy`, and the layering is clean everywhere else.
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

    /// A blitter that records where it was written, and nothing else.
    ///
    /// Its `bounds` is deliberately *smaller* than the clips the tests hand
    /// `RasterCtx`, so every test also exercises the intersect-on-construct that
    /// makes `clip ⊆ bounds` true.
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

    /// **The invariant the whole layer split rests on.** A rasterizer is handed
    /// nothing but a `RasterCtx`, and no sequence of calls on one can put a
    /// pixel outside `clip ∩ bounds` — not because implementations check, but
    /// because the blitter is a private field behind methods that all intersect.
    ///
    /// Sprayed deliberately: every method is called with geometry far outside
    /// both rects, which is the "spray-and-clip is correct, slow" row of the
    /// table on `RasterCtx` stated as a test.
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

    /// `RasterCtx::new` narrows the clip to the blitter's bounds, so an L1 that
    /// hands over a clip wider than its surface cannot widen anything.
    #[test]
    fn a_clip_wider_than_the_blitter_is_narrowed_on_construction() {
        let bounds = r(10, 10, 4, 4);
        let mut rec = Recorder::new(bounds);
        let mut cx = RasterCtx::new(&mut rec, r(0, 0, 100, 100));
        assert_eq!(cx.clip(), bounds);
        cx.rect(r(0, 0, 100, 100), NullColor);
        assert_eq!(rec.writes.len(), 16);
    }

    /// **D3, as a test: no primitive is ever unsupported.**
    ///
    /// `Plain` overrides nothing, so every call below runs a *default* — and
    /// every default must put pixels somewhere. This is what makes
    /// `impl Rasterizer for X {}` a legal rasterizer rather than a stub, and it
    /// is the check that would have caught `polygon` and `image` being logged
    /// no-ops on the embedded-graphics backend for as long as they were.
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

    /// A **filled** circle must be filled.
    ///
    /// The design sketch defaulted `circle` to a full-sweep `arc`, which is a
    /// curve and therefore has no interior — so a filled circle would have come
    /// out as an outline. That is a *plausible wrong image*, the failure mode
    /// "no lookalike defaults" exists to refuse, and it is the reason the default
    /// routes through `ellipse` instead.
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

        // An arc with the same style paints nothing, because an arc has no
        // interior and `fill` is documented as ignored on it.
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
