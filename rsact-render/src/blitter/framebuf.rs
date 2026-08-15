use crate::{
    blitter::{Blitter, Span},
    color::Color,
    framebuf::{Framebuf, FramebufStorage, PackedColor, units_for},
    geometry::{Point, Rect, Size},
    renderer::RenderResult,
};

/// A blitter over a [`Framebuf`] whose storage the caller owns.
///
/// **It is a colour buffer and nothing else: capacity plus addressing.** It does
/// not know the display's size, it does not know the frame policy, and it does
/// not know what a rasterizer or a renderer is. The rect it currently answers
/// for arrives with [`begin_region`](Blitter::begin_region), which is the only
/// thing that ever aims it.
///
/// An earlier shape took a `viewport: Size` at construction, to choose between
/// "aimed at the whole frame" and "aimed at nothing". That branch was a fossil:
/// it mattered only while `begin_region` *skipped* full-frame surfaces, and
/// WS6.4d made it retarget unconditionally — so the construction-time aim is
/// overwritten before a single pixel lands, and the field was written and never
/// read.
///
/// **It always has its target.** There is no attached/detached type-state here:
/// the state an application wants — "the renderer is between frames and I am
/// holding the pixels" — belongs to the *renderer*, and this type is what moves
/// in and out of it.
///
/// This type IS the loan: it owns `B`, so handing it back to the caller hands
/// back the buffer, which is WS6.7's DMA-soundness requirement (a borrow the
/// core can still write through is UB, which is why `embedded-dma`'s
/// `ReadBuffer` is `unsafe`). [`into_storage`](Self::into_storage) unwraps it
/// where the raw slice is what the transport wants.
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
    /// Wrap the caller's storage. **Infallible**, because there is nothing to
    /// check: a colour buffer of any size is a valid colour buffer, and whether
    /// it is big enough is a question about the *frame policy*, which this type
    /// has never heard of. That comparison happens when the target is lent to a
    /// renderer.
    ///
    /// Starts aimed at nothing. `begin_region` supplies the rect.
    pub fn new(storage: B) -> Self {
        Self { framebuf: Framebuf::tile(storage) }
    }

    /// Give the storage back, with the region that was painted into it.
    ///
    /// The rect is the only one a caller needs: `begin_region` retargets
    /// unconditionally, so the buffer's extent and the painted region are the
    /// same rectangle for every surface — a tile and a full-frame framebuffer
    /// alike. It is both what to index the buffer at (rows are strided at its
    /// width) and what to send.
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

    /// The colour's own packing: 8 for `BinaryColor`, 1 for anything a storage
    /// word holds whole.
    const PIXELS_PER_UNIT: usize = C::PPS;

    fn bounds(&self) -> Rect {
        self.framebuf.viewport()
    }

    fn capacity(&self) -> Option<usize> {
        Some(self.framebuf.capacity_units())
    }

    /// A span is a one-row rect, so this is WS6.3b's whole-word fill with a
    /// height of one: partial head word, `slice::fill` over the whole words,
    /// partial tail word. Going through
    /// [`Framebuf::fill_solid`](crate::framebuf::Framebuf::fill_solid) rather
    /// than re-deriving the index is the point of WS6.4e making that method
    /// inherent — a second copy of the addressing is how a tiled buffer ends up
    /// with fast fills in the wrong row and correct per-pixel writes.
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
    /// If `region` needs more units than the buffer holds. Refused, not
    /// asserted: the capacity check at attach guarantees the planner never asks,
    /// so this is the backstop for a renderer driven outside that path, and a
    /// backstop that aborts the device is worse than one that logs and skips
    /// (WS1.8).
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
        // Straight at the framebuf: the region clip is pushed by L1 *after* this
        // returns, and the whole retargeted buffer is what needs priming.
        self.framebuf.fill_solid(region, C::default_background());
        Ok(())
    }
}
