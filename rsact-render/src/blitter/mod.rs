//! **Layer 3 — where pixels physically land.**
//!
//! A [`Blitter`] accepts already-clipped, already-rasterized pixel work. Named
//! in the Skia/AGG sense. It is *not* the buffer itself — that is a
//! [`FramebufStorage`](crate::framebuf::FramebufStorage), which a
//! [`FramebufBlitter`](framebuf::FramebufBlitter) borrows.
//!
//! [`Span`] and the three addressing helpers live here rather than beside the
//! rasterizer, and the reason is the dependency direction: an L3 blitter must be
//! definable **without** an L2 rasterizer — that is what makes a `DirectBlitter`
//! straight to a panel, or a DMA2D fill, expressible — while a rasterizer is
//! meaningless without something to emit into. So `raster` depends on `blitter`
//! and never the reverse. (The plan's module map put `Span` in `raster/mod.rs`;
//! this is the one place the implementation deviates from it, deliberately.)

pub mod framebuf;
#[cfg(feature = "tiny-skia")]
pub mod pixmap;

use crate::{
    color::Color,
    geometry::{Point, Rect},
    renderer::RenderResult,
};
use core::ops::Range;

/// One horizontal run of pixels. **Absolute** coordinates.
///
/// `{y, x, w}` rather than `{y, x: Range}` because [`Range`] is not [`Copy`] and
/// every defaulted method reads the span twice.
///
/// **Spans never wrap a row.** The one case where wrapping would win — a rect
/// spanning the blitter's full width — is [`Blitter::fill_rect`], which the
/// blitter coalesces using its own stride. A wrapping span would be the
/// rasterizer asserting it knows that stride, which is L3's private fact and
/// wrong the moment a region is retargeted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Span {
    pub y: i32,
    pub x: i32,
    pub w: u32,
}

impl Span {
    pub const fn new(y: i32, x: i32, w: u32) -> Self {
        Self { y, x, w }
    }

    pub const fn x_range(&self) -> Range<i32> {
        self.x..self.x + self.w as i32
    }

    pub const fn len(&self) -> usize {
        self.w as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.w == 0
    }

    /// Clip to `rect`, returning the surviving span **and the offset into any
    /// per-pixel data indexed from the original start**.
    ///
    /// That offset is why this is a method rather than three inline copies:
    /// forgetting it shifts a coverage array by a few pixels, which yields a
    /// plausible image rather than a failure.
    pub fn clip_to(&self, rect: &Rect) -> Option<(Span, usize)> {
        if self.is_empty() || rect.is_zero_sized() {
            return None;
        }
        if self.y < rect.top_left.y
            || self.y >= rect.top_left.y + rect.size.height as i32
        {
            return None;
        }
        let x0 = self.x.max(rect.top_left.x);
        let x1 = (self.x + self.w as i32)
            .min(rect.top_left.x + rect.size.width as i32);
        if x1 <= x0 {
            return None;
        }
        Some((Span::new(self.y, x0, (x1 - x0) as u32), (x0 - self.x) as usize))
    }
}

// ── Addressing ──────────────────────────────────────────────────────────────
//
// **Helpers, not a contract.** A plain row-major framebuf uses them; a rotating
// (WS6.8) or page-packed blitter maps its own way. Free functions rather than
// provided trait methods, so that overriding is not a special case — a provided
// method would hard-code `y * width + x` and foreclose both.
//
// They yield **pixel** indices, not storage indices: dividing by the color's
// packing is the storage layer's job, and is what `Framebuf::point_to_subpart`
// already does. Same convention as `Framebuf::flat_index`, deliberately.

/// `p` in `bounds`-local coordinates.
pub const fn local(bounds: &Rect, p: Point) -> Point {
    Point::new(p.x - bounds.top_left.x, p.y - bounds.top_left.y)
}

/// Flat pixel index of `p` within `bounds`, whose width is the stride.
///
/// `p` must be inside `bounds`; callers clip first.
pub const fn pixel_index(bounds: &Rect, p: Point) -> usize {
    let local = local(bounds, p);
    local.y as usize * bounds.size.width as usize + local.x as usize
}

/// The contiguous flat-index range `span` occupies within `bounds`.
pub const fn span_range(bounds: &Rect, span: Span) -> Range<usize> {
    let start = pixel_index(bounds, Point::new(span.x, span.y));
    start..start + span.w as usize
}

/// Accepts already-clipped, already-rasterized pixel work.
///
/// The single required *drawing* method is a **row run**, because that is what a
/// packed framebuffer does best (`slice::fill` inside one row) and what a
/// display window does best (one SPI burst). This inverts the `draw_iter`-
/// required arrangement embedded-graphics imposes, which forces every fill
/// algorithm to destructure output it already had in span form. WS6.3b is the
/// precedent: it already overrides `fill_solid` at framebuf and renderer level
/// for exactly this reason.
///
/// # Capacity is stated twice, and the two are not redundant
///
/// [`UNITS`](Self::UNITS) is what the **type** knows; [`capacity`](Self::capacity)
/// is what the **value** knows. A `&'static mut [u16; 5760]` answers the first,
/// so a target too small for a frame policy is a **compile error**; a
/// `&'static mut [u16]` cannot, so it is checked when it is lent and the caller
/// gets a `Result`. A direct-to-panel target answers `None` to both — its
/// capacity is *unbounded*, there being no storage to overflow, so nothing is
/// checked at all.
///
/// **Associated consts make this trait dyn-incompatible (E0038), and that is
/// accepted.** An earlier draft avoided them to keep `dyn Blitter<Color = C>`
/// available "in case the rasterizer × blitter cross-product needs collapsing".
/// Maintainer's correction: an application uses **one** renderer + rasterizer +
/// blitter combination, so there is no cross-product — only the inlining a
/// monomorphized call gets, which is the thing an embedded target actually
/// wants. The consts buy a compile-time capacity proof; the erasure bought
/// nothing anyone was going to spend.
///
/// **No error channel on the drawing methods.** Every method draws or does nothing; there is nothing to
/// report. [`begin_region`](Self::begin_region) is the exception and the reason
/// is real: a region that does not fit the storage is a refusal, not a
/// degradation.
pub trait Blitter {
    type Color: Color;

    /// Units the **type** guarantees, or `None` when only the value knows.
    ///
    /// `Some(n)` for a fixed-size array, which is what lets a policy violation
    /// be a **compile error**. `None` for a runtime-length slice (checked when
    /// the target is lent) and for a target with no storage at all (never
    /// checked — see [`capacity`](Self::capacity)).
    ///
    /// It mirrors [`FramebufStorage::UNITS`](crate::framebuf::FramebufStorage::UNITS)
    /// one layer up, and means the same thing: **`None` is "ask the value",
    /// never "unbounded"**. That distinction is what a `usize::MAX` sentinel
    /// erased once already, letting an *empty* boxed slice satisfy a full-frame
    /// policy at compile time.
    const UNITS: Option<usize> = None;

    /// Pixels this target packs into one unit of capacity.
    ///
    /// `1` for anything that does not pack — a pixmap, an RGB framebuffer, a
    /// direct-to-panel target — hence the default. `8` for a 1-bpp framebuffer.
    ///
    /// **A const because the check it feeds is a compile-time one.** Comparing a
    /// frame policy's unit budget against a target's capacity is meaningless
    /// unless the two count the same thing: a 1-bpp target under a
    /// `PIXELS_PER_UNIT = 1` policy would appear to need eight times the storage
    /// it does, and the reverse would silently under-demand. Both sides are
    /// consts, so the disagreement never survives a build.
    const PIXELS_PER_UNIT: usize = 1;

    /// The absolute rect it currently accepts writes for.
    /// After [`begin_region(r)`](Self::begin_region), this is `r`.
    fn bounds(&self) -> Rect;

    /// Units it can hold, or `None` for no storage bound at all (a
    /// `DirectBlitter`, a GPU attachment). Two states, because at the *value*
    /// level "the type cannot say" does not arise — that case belongs to the
    /// compile-time proof, which reads the storage type directly.
    ///
    /// **A unit is one `C::Storage` element** — the same vocabulary
    /// [`FramebufStorage::unit_count`] and [`region_units`] already use, so the
    /// value drops straight into [`assert_policy_fits`]. For a color that does
    /// not pack, one unit is one pixel, so a pixmap reports `width * height`.
    ///
    /// [`FramebufStorage::unit_count`]: crate::framebuf::FramebufStorage::unit_count
    /// [`region_units`]: crate::renderer::region_units
    /// [`assert_policy_fits`]: crate::region::assert_policy_fits
    fn capacity(&self) -> Option<usize>;

    // ── required ────────────────────────────────────────────────────────────

    /// `span` is guaranteed inside [`bounds()`](Self::bounds) by the caller
    /// (`RasterCtx`), so implementations need not re-check it.
    fn fill_span(&mut self, span: Span, color: Self::Color);

    /// Aim at `region`: retarget the framebuf, open the panel's address window,
    /// bind an attachment — **and prime it**. Unconditional, including for a
    /// blitter already spanning the frame, so `bounds() == region` always and a
    /// caller carries one rect.
    ///
    /// **Required, not defaulted**: a no-op default would not retarget, and
    /// every addressing helper assumes it did.
    ///
    /// **Priming belongs here, atomically with the retarget.** A region is
    /// scratch with no history, so it must start at the true background or a
    /// tile flushes with holes; the region clip is pushed by L1 *after* this
    /// returns, so the fill cannot go through the clipped path. The background
    /// is a color-level fact ([`Color::default_background`]), which is why L3 can
    /// supply it without a theme.
    ///
    /// There is no `end_region`. Every implementation of it in this codebase was
    /// a no-op, and the one real job it could have — a completion barrier for
    /// asynchronous writes — belongs where the loan goes back.
    ///
    /// [`Color::default_background`]: crate::color::Color::default_background
    fn begin_region(&mut self, region: Rect) -> RenderResult;

    // ── defaulted; override where the layout or hardware helps ──────────────

    /// Also the vertical-run case (`width == 1`): borders, separators, scrollbar
    /// tracks.
    ///
    /// **A vertically packed framebuf must override this.** The row default is
    /// correct there but ~8× pessimal: on page-packed storage a 1×8 run is one
    /// byte, and the default turns a 64 px separator into 64 read-modify-writes
    /// of the same 8 bytes.
    fn fill_rect(&mut self, rect: Rect, color: Self::Color) {
        for y in rect.rows() {
            self.fill_span(
                Span::new(y, rect.top_left.x, rect.size.width),
                color,
            );
        }
    }

    /// Distinct colors — images, gradients. `colors.len() == span.len()`,
    /// debug-asserted.
    fn fill_run(&mut self, span: Span, colors: &[Self::Color]) {
        debug_assert_eq!(colors.len(), span.len());
        for (i, color) in colors.iter().enumerate() {
            self.pixel(Point::new(span.x + i as i32, span.y), *color);
        }
    }

    /// Anti-aliased run. `coverage.len() == span.len()`; 0 = untouched.
    ///
    /// Exercised from day one: `TinySkiaRasterizer` emits coverage from a
    /// `Mask`. The default thresholds at 128, which is a **last resort** and not
    /// a story for 1-bpp — on a non-blending target it leaves a *gapped*
    /// hairline, because both pixels of each 45° step fall below the threshold.
    /// A rasterizer that cares must be able to ask whether blending is real;
    /// that query is deliberately not designed yet, and this default is not a
    /// substitute for it.
    ///
    /// Deliberately no `read_pixel`: blending is the only reason to read, so the
    /// capability and its use stay in one method instead of two that can
    /// disagree.
    fn blend_span(&mut self, span: Span, color: Self::Color, coverage: &[u8]) {
        debug_assert_eq!(coverage.len(), span.len());
        // Coalesce covered pixels into runs rather than emitting one span each:
        // the whole point of the span protocol is that a run is cheaper than its
        // pixels, and a thresholded edge is mostly runs.
        let mut run: Option<i32> = None;
        for (i, cov) in coverage.iter().enumerate() {
            let x = span.x + i as i32;
            if *cov >= 128 {
                run.get_or_insert(x);
            } else if let Some(start) = run.take() {
                self.fill_span(
                    Span::new(span.y, start, (x - start) as u32),
                    color,
                );
            }
        }
        if let Some(start) = run {
            let end = span.x + span.w as i32;
            self.fill_span(
                Span::new(span.y, start, (end - start) as u32),
                color,
            );
        }
    }

    /// Worth overriding: thin-stroke algorithms (Bresenham, Wu, circle outline)
    /// emit nothing but these, and a framebuffer's pixel write skips the range
    /// setup a length-1 span pays for.
    fn pixel(&mut self, p: Point, color: Self::Color) {
        self.fill_span(Span::new(p.y, p.x, 1), color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Size;

    fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect::new(Point::new(x, y), Size::new(w, h))
    }

    /// The offset is the whole reason `clip_to` exists: a coverage or color
    /// array is indexed from the span's ORIGINAL start, so clipping the span
    /// without slicing the data in step shifts an image by a few pixels — a
    /// plausible picture rather than a failure.
    #[test]
    fn clipping_a_span_reports_where_its_data_now_starts() {
        let clip = r(10, 5, 10, 10);

        // Overhanging on the left: 6 pixels dropped, so the data starts at 6.
        let (span, offset) = Span::new(7, 4, 20).clip_to(&clip).unwrap();
        assert_eq!(span, Span::new(7, 10, 10));
        assert_eq!(offset, 6);

        // Overhanging on the right only: nothing dropped from the front.
        let (span, offset) = Span::new(7, 12, 20).clip_to(&clip).unwrap();
        assert_eq!(span, Span::new(7, 12, 8));
        assert_eq!(offset, 0);

        // Entirely inside: unchanged.
        let (span, offset) = Span::new(7, 11, 3).clip_to(&clip).unwrap();
        assert_eq!(span, Span::new(7, 11, 3));
        assert_eq!(offset, 0);
    }

    #[test]
    fn a_span_outside_the_clip_survives_as_nothing() {
        let clip = r(10, 5, 10, 10);
        assert!(Span::new(4, 10, 10).clip_to(&clip).is_none(), "above");
        assert!(Span::new(15, 10, 10).clip_to(&clip).is_none(), "below");
        assert!(Span::new(7, 0, 10).clip_to(&clip).is_none(), "left");
        assert!(Span::new(7, 20, 10).clip_to(&clip).is_none(), "right");
        assert!(Span::new(7, 10, 0).clip_to(&clip).is_none(), "empty");
        assert!(Span::new(7, 10, 5).clip_to(&Rect::zero()).is_none());
    }

    /// Addressing is relative to `bounds`, whose width is the stride — which is
    /// what lets one allocation serve any region shape.
    #[test]
    fn addressing_is_relative_to_the_bounds_and_strided_by_them() {
        let bounds = r(40, 100, 16, 8);
        assert_eq!(local(&bounds, Point::new(40, 100)), Point::new(0, 0));
        assert_eq!(pixel_index(&bounds, Point::new(40, 100)), 0);
        assert_eq!(pixel_index(&bounds, Point::new(43, 102)), 2 * 16 + 3);
        assert_eq!(span_range(&bounds, Span::new(101, 44, 4)), 20..24);
    }
}
