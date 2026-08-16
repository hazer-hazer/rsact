//! Where pixels land: a [`Blitter`] writes runs of already-clipped pixels into
//! whatever it is backed by — a framebuffer, a pixmap, a panel's address window.
//!
//! It is not the buffer itself. [`FramebufBlitter`] is a blitter *over* a
//! caller-owned [`FramebufStorage`](crate::framebuf::FramebufStorage), which it
//! borrows and gives back.

use crate::{
    color::Color,
    framebuf::{Framebuf, FramebufStorage, PackedColor, units_for},
    geometry::{Point, Rect, Size},
    renderer::RenderResult,
};
use core::ops::Range;

/// One horizontal run of pixels. **Absolute** coordinates.
///
/// `{y, x, w}` rather than `{y, x: Range}` because [`Range`] is not [`Copy`] and
/// every defaulted method reads the span twice.
///
/// **A span never wraps a row.** Emit a full-width rect through
/// [`Blitter::fill_rect`] instead — only the blitter knows its own stride, and
/// it can coalesce the rows itself.
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
    /// The offset is why this is a method rather than inline copies: forgetting
    /// it shifts a coverage array by a few pixels, which yields a plausible
    /// image rather than a failure.
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
// **Helpers, not a contract.** A row-major framebuf uses them; a rotating or
// page-packed blitter maps its own way. Free functions rather than provided
// trait methods, which would hard-code `y * width + x` and foreclose that.
//
// They yield **pixel** indices, not storage indices — dividing by the color's
// packing is the storage layer's job.

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
/// To write one, implement [`fill_span`](Self::fill_span),
/// [`bounds`](Self::bounds), [`capacity`](Self::capacity) and
/// [`begin_region`](Self::begin_region); override any of the defaulted methods
/// your target does better. A row run is the required primitive because that is
/// what a packed framebuffer (`slice::fill` inside one row) and a display window
/// (one SPI burst) are both fastest at.
///
/// # Capacity is stated twice
///
/// [`UNITS`](Self::UNITS) is what the **type** knows; [`capacity`](Self::capacity)
/// is what the **value** knows. A `&'static mut [u16; 5760]` answers the first,
/// so a target too small for a frame policy is a compile error; a
/// `&'static mut [u16]` cannot, so it is checked when lent and the caller gets a
/// `Result`. A direct-to-panel target answers `None` to both — its capacity is
/// *unbounded*, so nothing is checked at all.
///
/// The associated consts make this trait dyn-incompatible (E0038), so there is
/// no `dyn Blitter`; name the concrete type.
///
/// The drawing methods return nothing — each draws or does nothing.
/// [`begin_region`](Self::begin_region) is the exception: a region that does not
/// fit the storage is refused.
pub trait Blitter {
    type Color: Color;

    /// Units the **type** guarantees, or `None` when only the value knows.
    ///
    /// `Some(n)` for a fixed-size array, which is what lets a policy violation
    /// be a **compile error**. `None` for a runtime-length slice (checked when
    /// the target is lent) and for a target with no storage at all (never
    /// checked — see [`capacity`](Self::capacity)).
    ///
    /// Mirrors [`FramebufStorage::UNITS`](crate::framebuf::FramebufStorage::UNITS)
    /// and means the same thing: **`None` is "ask the value", never
    /// "unbounded"**.
    const UNITS: Option<usize> = None;

    /// Pixels this target packs into one unit of capacity.
    ///
    /// `1` for anything that does not pack — a pixmap, an RGB framebuffer, a
    /// direct-to-panel target — hence the default. `8` for a 1-bpp framebuffer.
    ///
    /// A const because the check it feeds is a compile-time one: a 1-bpp target
    /// under a `PIXELS_PER_UNIT = 1` policy would appear to need eight times the
    /// storage it does, and the reverse would silently under-demand.
    const PIXELS_PER_UNIT: usize = 1;

    /// The absolute rect it currently accepts writes for.
    /// After [`begin_region(r)`](Self::begin_region), this is `r`.
    fn bounds(&self) -> Rect;

    /// Units it can hold, or `None` for no storage bound at all — a
    /// direct-to-panel target, a GPU attachment.
    ///
    /// **A unit is one `C::Storage` element**, the same vocabulary
    /// [`FramebufStorage::unit_count`] and [`region_units`] use, so the value
    /// drops straight into [`assert_policy_fits`]. For a color that does not
    /// pack, one unit is one pixel.
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
    /// **Prime as well as retarget**, in that one call. A region arrives
    /// holding whatever the last one left in it, so an implementation must fill
    /// it with [`Color::default_background`] or the frame flushes with holes.
    /// Do it directly, not through anything that clips: the region's own clip is
    /// not established until this returns.
    ///
    /// There is no `end_region`: the one real job it could have — a completion
    /// barrier for asynchronous writes — belongs where the loan goes back.
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
    /// The default thresholds at 128, which is a last resort: on a non-blending
    /// target it leaves a *gapped* hairline, both pixels of each 45° step
    /// falling below the threshold. A rasterizer that cares needs to ask whether
    /// blending is real, and that query is not designed yet.
    ///
    /// No `read_pixel`: blending is the only reason to read, so the capability
    /// and its use stay in one method rather than two that can disagree.
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

/// A blitter over a [`Framebuf`] whose storage the caller owns.
///
/// It starts aimed at nothing: the rect it writes to arrives with
/// [`begin_region`](Blitter::begin_region), so call that before painting or
/// everything lands in a zero-sized target.
///
/// This type **is** the loan — it owns `B`, so handing it back hands back the
/// buffer, which is what DMA needs (a borrow the core can still write through is
/// UB, hence `embedded-dma`'s `ReadBuffer` being `unsafe`).
/// [`into_storage`](Self::into_storage) unwraps it where the transport wants the
/// raw slice.
pub struct FramebufBlitter<C, B>
where
    C: Color + PackedColor,
    B: FramebufStorage<C>,
{
    framebuf: Framebuf<C, B>,
}

impl<C, B> FramebufBlitter<C, B>
where
    C: Color + PackedColor,
    B: FramebufStorage<C>,
{
    /// Wrap the caller's storage. Infallible: a color buffer of any size is a
    /// valid color buffer, and whether it is big enough is a question about the
    /// frame policy, answered when the target is lent to a renderer.
    ///
    /// Starts aimed at nothing; `begin_region` supplies the rect.
    pub fn new(storage: B) -> Self {
        Self { framebuf: Framebuf::new(storage) }
    }

    /// Give the storage back, with the region that was painted into it.
    ///
    /// One rect suffices: `begin_region` retargets unconditionally, so the
    /// buffer's extent and the painted region are the same rectangle for every
    /// surface. It is both what to index the buffer at — rows are strided at its
    /// width — and what to send.
    pub fn into_storage(self) -> (B, Rect) {
        let at = self.framebuf.viewport();
        (self.framebuf.into_buffer(), at)
    }

    /// Read access to the lent storage — what a caller flushing a tile walks,
    /// without giving the buffer back.
    pub fn framebuf(&self) -> &Framebuf<C, B> {
        &self.framebuf
    }
}

impl<C, B> Blitter for FramebufBlitter<C, B>
where
    C: Color + PackedColor,
    B: FramebufStorage<C>,
{
    type Color = C;

    /// What the storage **type** guarantees — `Some(N)` for a `&mut [T; N]`,
    /// which is what makes a too-small buffer a compile error, and `None` for a
    /// runtime-length slice, which is checked when the target is lent.
    const UNITS: Option<usize> = <B as FramebufStorage<C>>::UNITS;

    /// The color's own packing: 8 for `BinaryColor`, 1 for anything a storage
    /// word holds whole.
    const PIXELS_PER_UNIT: usize = C::PPS;

    fn bounds(&self) -> Rect {
        self.framebuf.viewport()
    }

    fn capacity(&self) -> Option<usize> {
        Some(self.framebuf.capacity_units())
    }

    /// A span is a one-row rect, so this is the whole-word fill at height one.
    /// Going through
    /// [`Framebuf::fill_solid`](crate::framebuf::Framebuf::fill_solid) rather
    /// than re-deriving the index keeps the addressing in one place; a second
    /// copy is how a tiled buffer ends up with fast fills in the wrong row and
    /// correct per-pixel writes.
    fn fill_span(&mut self, span: Span, color: C) {
        self.framebuf.fill_solid(
            Rect::new(Point::new(span.x, span.y), Size::new(span.w, 1)),
            color,
        );
    }

    /// Straight to the framebuffer's own rect fill, which already steps rows by
    /// the stride and splits each into head/whole/tail — so a full-width rect
    /// costs one `slice::fill` per row rather than one per span plus the loop.
    fn fill_rect(&mut self, rect: Rect, color: C) {
        self.framebuf.fill_solid(rect, color);
    }

    fn fill_run(&mut self, span: Span, colors: &[C]) {
        debug_assert_eq!(colors.len(), span.len());
        for (i, color) in colors.iter().enumerate() {
            self.framebuf
                .set_pixel(Point::new(span.x + i as i32, span.y), *color);
        }
    }

    /// A real read-modify-write, which is what a framebuffer can offer and a
    /// write-only panel cannot.
    ///
    /// Per pixel, and deliberately: blending *is* per pixel. Note the cost —
    /// this defeats write-combining, and it is why a region must be primed with
    /// the true background before painting, or the first anti-aliased edge in a
    /// region blends against whatever the previous region left there.
    fn blend_span(&mut self, span: Span, color: C, coverage: &[u8]) {
        debug_assert_eq!(coverage.len(), span.len());
        for (i, cov) in coverage.iter().enumerate() {
            if *cov == 0 {
                continue;
            }
            let p = Point::new(span.x + i as i32, span.y);
            let blended = if *cov == u8::MAX {
                color
            } else {
                match self.framebuf.pixel(p) {
                    Some(dst) => dst.mix(*cov as f32 / 255.0, color),
                    // Outside the buffer: `RasterCtx` guarantees this cannot
                    // happen, so the unblended color is a degradation nothing
                    // should reach rather than a designed fallback.
                    None => color,
                }
            };
            self.framebuf.set_pixel(p, blended);
        }
    }

    fn pixel(&mut self, p: Point, color: C) {
        self.framebuf.set_pixel(p, color);
    }

    /// Retarget **and** prime, atomically.
    ///
    /// The retarget makes `region`'s own width the stride, so any region fitting
    /// the capacity is addressable; the prime is what stops a merged region's
    /// dead space — the gap between two damage rects, container padding, the
    /// area under a transparent `Flex` — flushing as whatever the previous
    /// region left there.
    ///
    /// # Errors
    ///
    /// If `region` needs more units than the buffer holds. Refused rather than
    /// asserted: the capacity check at attach means the planner never asks, so
    /// this is a backstop for a renderer driven outside that path — and one that
    /// aborts the device is worse than one that logs and skips.
    fn begin_region(&mut self, region: Rect) -> RenderResult {
        let want = units_for::<C>(region.size.width, region.size.height);
        let have = self.framebuf.capacity_units();
        if want > have {
            log::error!(
                "region {region:?} needs {want} storage units, the attached \
                 buffer holds {have}; skipping it"
            );
            return Err(());
        }
        self.framebuf.retarget(region);
        // Straight at the framebuf: the region's clip is not established until
        // this returns, and the whole retargeted buffer needs priming.
        self.framebuf.fill_solid(region, C::default_background());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Size;

    fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect::new(Point::new(x, y), Size::new(w, h))
    }

    /// A coverage or color array is indexed from the span's *original* start,
    /// so clipping the span without slicing the data in step shifts an image by
    /// a few pixels — a plausible picture rather than a failure.
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
