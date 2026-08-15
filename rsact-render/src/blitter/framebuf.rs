use crate::{
    blitter::{Blitter, Span},
    color::Color,
    framebuf::{Framebuf, FramebufStorage, PackedColor, units_for},
    geometry::{Point, Rect, Size},
    renderer::RenderResult,
};

/// A blitter over a [`Framebuf`] whose storage the caller owns.
///
/// **It always has its target.** There is no attached/detached type-state here:
/// a blitter that has no buffer is not a thing an application needs, because the
/// state it wants — "the renderer is between frames and I am holding the
/// pixels" — belongs to the *renderer*. `RasterRenderer` carries that state and
/// this type is what moves in and out of it. (The first shape of this design had
/// it the other way round; see `RasterRenderer`'s docs for what that cost.)
///
/// **No `P` parameter.** A frame policy is the application's declaration about
/// frames, not a property of a thing that merely has a size — so it stays on the
/// renderer, which is where `attach` compares it against
/// [`capacity`](Blitter::capacity).
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
    /// The **display's** size, not the region's.
    ///
    /// Needed only at construction, to decide whether a buffer covers the frame
    /// (aim it at the frame) or is a tile (aim it at nothing until the first
    /// `begin_region`). Getting that wrong is not cosmetic: aiming a full-frame
    /// buffer at nothing is the WS6.4d bug where every flush after the first
    /// sent an empty rect.
    viewport: Size,
}

impl<C, B> FramebufBlitter<C, B>
where
    C: Color + PackedColor,
    B: FramebufStorage<C>,
{
    /// Wrap the caller's storage.
    ///
    /// A buffer big enough for the whole frame is aimed at the whole frame; a
    /// smaller one is aimed at nothing until `begin_region` supplies a region.
    pub fn new(viewport: Size, storage: B) -> Self {
        let full_frame = units_for::<C>(viewport.width, viewport.height);
        let framebuf = if storage.unit_count() >= full_frame {
            Framebuf::new(viewport, storage)
        } else {
            Framebuf::tile(storage)
        };
        Self { framebuf, viewport }
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

    /// The display's size, for a renderer that needs to report it.
    pub fn viewport(&self) -> Size {
        self.viewport
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

    fn bounds(&self) -> Rect {
        self.framebuf.viewport()
    }

    fn capacity(&self) -> Option<usize> {
        Some(self.framebuf.capacity_units())
    }

    /// The color's own packing — 8 for `BinaryColor`, 1 for anything a storage
    /// word holds whole. This is what lets `RasterRenderer::attach` check that a
    /// frame policy's budget and this target's capacity count the same thing.
    fn pixels_per_unit(&self) -> usize {
        C::PPS
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
    fn begin_region(&mut self, region: Rect) -> RenderResult {
        self.framebuf.retarget(region);
        // Straight at the framebuf: the region clip is pushed by L1 *after* this
        // returns, and the whole retargeted buffer is what needs priming.
        self.framebuf.fill_solid(region, C::default_background());
        Ok(())
    }
}
