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
/// `{y, x, w}` and not a [`Range`], which is not [`Copy`].
///
/// **A span never wraps a row.** Emit a full-width rect through
/// [`Blitter::fill_rect`], which can coalesce rows using the stride only the
/// blitter knows.
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
    /// Slice the data by that offset in step, or a coverage array ends up
    /// shifted a few pixels — a plausible image rather than a failure.
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

// Row-major addressing helpers, not a contract: a rotating or page-packed
// blitter maps its own way, which a provided trait method would foreclose.
// They yield *pixel* indices; dividing by the color's packing is the storage
// layer's job.

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
/// Implement [`fill_span`](Self::fill_span), [`bounds`](Self::bounds),
/// [`capacity`](Self::capacity) and [`begin_region`](Self::begin_region), then
/// override whichever defaults your target does better. A row run is the
/// required primitive because a packed framebuffer (`slice::fill` within a row)
/// and a display window (one SPI burst) are both fastest at it.
///
/// Capacity is stated twice: [`UNITS`](Self::UNITS) is what the type knows and
/// makes a policy violation a compile error, [`capacity`](Self::capacity) is
/// what the value knows and is checked when the target is lent.
///
/// The associated consts make this dyn-incompatible (E0038) — there is no
/// `dyn Blitter`. Drawing methods return nothing; only
/// [`begin_region`](Self::begin_region) can refuse.
pub trait Blitter {
    type Color: Color;

    /// Units the **type** guarantees. `Some(n)` for a fixed-size array, `None`
    /// for a runtime-length slice or a target with no storage.
    ///
    /// **`None` is "ask the value", never "unbounded".**
    const UNITS: Option<usize> = None;

    /// Pixels this target packs into one unit of capacity: `8` for a 1-bpp
    /// framebuffer, `1` for anything that does not pack. Must agree with the
    /// frame policy's, or the capacity check compares unlike numbers.
    const PIXELS_PER_UNIT: usize = 1;

    /// The absolute rect it currently accepts writes for.
    /// After [`begin_region(r)`](Self::begin_region), this is `r`.
    fn bounds(&self) -> Rect;

    /// Units it can hold, or `None` for no storage bound at all — a
    /// direct-to-panel target, a GPU attachment.
    ///
    /// A unit is one `C::Storage` element, so for an unpacked color it is one
    /// pixel.
    fn capacity(&self) -> Option<usize>;

    // ── required ────────────────────────────────────────────────────────────

    /// `span` is guaranteed inside [`bounds()`](Self::bounds) by the caller
    /// (`RasterCtx`), so implementations need not re-check it.
    fn fill_span(&mut self, span: Span, color: Self::Color);

    /// Aim at `region` — retarget the buffer, open the panel's address window.
    /// Unconditional, so `bounds() == region` afterwards.
    ///
    /// **Aiming only: this must not paint.** A region arrives holding whatever
    /// the last one left in it, so *someone* has to write every pixel before the
    /// region is flushed — but which color an unpainted pixel takes is a style
    /// question, and a blitter has no style. The caller owns it; rsact-ui paints
    /// its page background as the first thing in `Page::paint_region`.
    fn begin_region(&mut self, region: Rect) -> RenderResult;

    // ── defaulted; override where the layout or hardware helps ──────────────

    /// Also the vertical-run case (`width == 1`): borders, separators.
    ///
    /// **Override on page-packed storage**, where a 1×8 run is a single byte and
    /// the row-at-a-time default turns a 64 px separator into 64
    /// read-modify-writes of the same 8 bytes.
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

    /// One color at a per-pixel weight. `coverage.len() == span.len()`;
    /// 0 = untouched, 255 = replace.
    ///
    /// Coverage is just per-pixel alpha, so this is the general weighted write,
    /// not an anti-aliasing hook: a rasterizer's edge coverage is one producer,
    /// a gradient mask, a fade or a dissolve are others, and `PixmapBlitter`
    /// implements it as full `SourceOver`.
    ///
    /// The default merely thresholds at 128, which is the one case that IS
    /// anti-aliasing-specific and bad at it — a 45° edge comes out a *gapped*
    /// hairline, both pixels of each step falling below the threshold. Override
    /// wherever the target can really blend.
    fn blend_span(&mut self, span: Span, color: Self::Color, coverage: &[u8]) {
        debug_assert_eq!(coverage.len(), span.len());
        // Coalesce into runs: a thresholded edge is mostly runs, and a run is
        // cheaper than its pixels.
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

    /// Worth overriding — thin-stroke algorithms emit nothing else, and a
    /// direct pixel write skips the range setup a length-1 span pays for.
    fn pixel(&mut self, p: Point, color: Self::Color) {
        self.fill_span(Span::new(p.y, p.x, 1), color)
    }
}

/// A blitter over a [`Framebuf`] whose storage the caller owns.
///
/// Starts aimed at nothing — call [`begin_region`](Blitter::begin_region) before
/// painting, or everything lands in a zero-sized target.
///
/// It owns `B`, so handing the blitter back hands back the buffer, which is what
/// DMA needs. [`into_storage`](Self::into_storage) unwraps it.
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
    /// Wrap the caller's storage, aimed at nothing. Infallible — whether it is
    /// big enough is answered when the target is lent to a renderer.
    pub fn new(storage: B) -> Self {
        Self { framebuf: Framebuf::new(storage) }
    }

    /// Give the storage back, with the region painted into it, and the rect it
    /// covers — both how to read the buffer and where to send it.
    ///
    /// Rows are strided at [`Framebuf::row_stride`], which is the region's width
    /// **padded to a whole storage unit**: a 122-pixel 1-bpp row is 16 bytes, so
    /// a row never straddles a byte and a mono panel takes the buffer
    /// unrepacked. Equal to the width whenever a pixel has a word of its own.
    ///
    /// [`Framebuf::row_stride`]: crate::framebuf::Framebuf::row_stride
    pub fn into_storage(self) -> (B, Rect) {
        let at = self.framebuf.viewport();
        (self.framebuf.into_buffer(), at)
    }

    /// Read the lent storage without giving the buffer back.
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

    /// What the storage type guarantees.
    const UNITS: Option<usize> = <B as FramebufStorage<C>>::UNITS;

    /// The color's own packing.
    const PIXELS_PER_UNIT: usize = C::PPS;

    fn bounds(&self) -> Rect {
        self.framebuf.viewport()
    }

    fn capacity(&self) -> Option<usize> {
        Some(self.framebuf.capacity())
    }

    /// A one-row rect, so this is the whole-word fill at height one. Goes
    /// through [`Framebuf::fill_solid`](crate::framebuf::Framebuf::fill_solid)
    /// rather than re-deriving the index.
    fn fill_span(&mut self, span: Span, color: C) {
        self.framebuf.fill_solid(
            Rect::new(Point::new(span.x, span.y), Size::new(span.w, 1)),
            color,
        );
    }

    /// Straight to the framebuffer's rect fill, which already steps rows by the
    /// stride, so a full-width rect costs one `slice::fill` per row.
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

    /// A real read-modify-write, per pixel, which a write-only panel could not
    /// offer. Defeats write-combining, so it is the slow path.
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
                    // Unreachable — `RasterCtx` has already clipped.
                    None => color,
                }
            };
            self.framebuf.set_pixel(p, blended);
        }
    }

    fn pixel(&mut self, p: Point, color: C) {
        self.framebuf.set_pixel(p, color);
    }

    /// Retarget to `region`'s own width. Contents are **not** cleared — see
    /// [`Blitter::begin_region`]: the caller paints the region's background.
    ///
    /// # Errors
    ///
    /// If `region` needs more units than the buffer holds. A backstop — the
    /// capacity check at `attach` means the planner never asks — so it logs and
    /// skips rather than aborting the device.
    fn begin_region(&mut self, region: Rect) -> RenderResult {
        let want = units_for::<C>(region.size.width, region.size.height);
        let have = self.framebuf.capacity();
        if want > have {
            log::error!(
                "region {region:?} needs {want} storage units, the attached \
                 buffer holds {have}; skipping it"
            );
            return Err(());
        }
        self.framebuf.retarget(region);
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

    /// The data is indexed from the span's *original* start, so failing to
    /// slice it in step shifts an image by a few pixels.
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

    /// Addressing is relative to `bounds`, whose width is the stride.
    #[test]
    fn addressing_is_relative_to_the_bounds_and_strided_by_them() {
        let bounds = r(40, 100, 16, 8);
        assert_eq!(local(&bounds, Point::new(40, 100)), Point::new(0, 0));
        assert_eq!(pixel_index(&bounds, Point::new(40, 100)), 0);
        assert_eq!(pixel_index(&bounds, Point::new(43, 102)), 2 * 16 + 3);
        assert_eq!(span_range(&bounds, Span::new(101, 44, 4)), 20..24);
    }
}
