use crate::{
    color::{Color, Rgba},
    geometry::*,
    image::DrawImage,
    path::Path,
    style::DrawStyle,
};

use core::marker::PhantomData;

pub type RenderResult = Result<(), ()>;

/// Storage units a `w × h` region needs, **padding each row** to a whole unit —
/// a 122-pixel 1-bpp row costs 16 bytes, not 15.25.
///
/// ```
/// # use rsact_render::renderer::region_units;
/// assert_eq!(region_units(240, 24, 1), 5760); // RGB565: one unit per pixel
/// assert_eq!(region_units(122, 24, 8), 384);  // 1-bpp: 16 bytes per row
/// ```
pub const fn region_units(w: u32, h: u32, pixels_per_unit: usize) -> usize {
    // Treat 0 as unpacked: over-estimates rather than dividing by zero.
    let pps = if pixels_per_unit == 0 { 1 } else { pixels_per_unit };
    let row_units = ((w as usize) + pps - 1) / pps;
    row_units * (h as usize)
}

// #[derive(PartialEq, Clone)]
// pub enum AntiAliasing {
//     Disabled,
//     Enabled,
// }

// #[derive(Default, Clone, PartialEq, IntoMaybeReactive)]
// pub struct RendererOptions {
//     pub anti_aliasing: Option<AntiAliasing>,
// }

// impl RendererOptions {
//     pub fn new() -> Self {
//         Self { anti_aliasing: None }
//     }

//     // TODO: Simple `with_anti_aliasing` method shortcut
//     pub fn anti_aliasing(mut self, aa: AntiAliasing) -> Self {
//         self.anti_aliasing = Some(aa);
//         self
//     }
// }

/// Whether a renderer is holding its surface. Never named directly:
/// [`attach`](RasterRenderer::attach) and [`detach`](RasterRenderer::detach)
/// move between [`Attached`] and [`Detached`].
pub trait Attachment<S> {
    /// The surface field's type in this state: `S` attached, `()` detached.
    type Slot;
}

/// The renderer is holding a surface and can draw. Drawing after a `detach` is
/// a compile error, not a lost frame:
///
/// ```compile_fail
/// # use rsact_render::{blitter::FramebufBlitter, geometry::Size,
/// #                    eg::rasterizer::EgRasterizer, region::Unbounded,
/// #                    renderer::{RasterRenderer, Renderer}};
/// # use embedded_graphics::pixelcolor::Rgb888;
/// # type Screen = RasterRenderer<
/// #     EgRasterizer, FramebufBlitter<Rgb888, &'static mut [u32]>, Unbounded>;
/// # let buf: &'static mut [u32] = vec![0; 16 * 16].leak();
/// let r = Screen::with_blitter(
///     EgRasterizer, Size::new_equal(16), FramebufBlitter::new(buf)).unwrap();
/// let (parked, _blitter) = r.detach();
/// let _ = Renderer::size(&parked);
/// ```
pub struct Attached;

impl<S> Attachment<S> for Attached {
    type Slot = S;
}

/// The caller is holding the surface; the renderer keeps only its
/// configuration. Where a renderer waits while its buffer is being shipped.
pub struct Detached;

impl<S> Attachment<S> for Detached {
    type Slot = ();
}

/// Primitive drawing, independent of any backend.
///
/// Clips nest: [`push_clip`](Renderer::push_clip) stores `area ∩ top` and
/// [`pop_clip`](Renderer::pop_clip) never pops the region being painted.
pub trait Renderer {
    type Color: Color;

    /// The largest region this renderer will accept. [`Unbounded`] unless it
    /// paints through a buffer smaller than the frame.
    ///
    /// [`Unbounded`]: crate::region::Unbounded
    type Policy: crate::region::FramePolicy;

    fn size(&self) -> Size;

    /// About to paint `region`, in absolute screen coordinates.
    ///
    /// Override to re-aim a surface smaller than the frame, or to set a scissor
    /// rect. A full-frame surface needs neither, hence the empty default.
    fn begin_region(&mut self, region: Rect) -> RenderResult {
        let _ = region;
        Ok(())
    }

    /// Finish the region opened by [`begin_region`](Renderer::begin_region) —
    /// where a GPU would end its render pass.
    fn end_region(&mut self) -> RenderResult {
        Ok(())
    }

    /// Restrict drawing to `area` until the matching
    /// [`pop_clip`](Renderer::pop_clip). Calls must be balanced; an unmatched
    /// pop degrades rather than panicking.
    fn push_clip(&mut self, area: Rect);

    /// Undo the innermost [`push_clip`]. No-op if the stack is empty.
    ///
    /// [`push_clip`]: Renderer::push_clip
    fn pop_clip(&mut self);

    /// The region drawing is confined to, for callers that skip work that
    /// cannot land. Report the region being painted, not the surface behind it.
    ///
    /// **Never report narrower than the real clip** — too wide, or `None`, only
    /// costs redundant paint. `None` disables skipping entirely, which is right
    /// for a sink like [`NullRenderer`] whose `size()` is zero.
    fn clip_bounds(&self) -> Option<Rect> {
        None
    }

    fn fill_solid(&mut self, rect: Rect, color: Self::Color) -> RenderResult;

    fn pixel(&mut self, point: Point, color: Self::Color) -> RenderResult;

    fn line(
        &mut self,
        from: Point,
        to: Point,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn rect(
        &mut self,
        rect: Rect,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn rounded_rect(
        &mut self,
        rect: Rect,
        corners: CornerRadii,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn circle(
        &mut self,
        top_left: Point,
        diameter: u32,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn arc(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn ellipse(
        &mut self,
        bounding_box: Rect,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn sector(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn polygon(
        &mut self,
        points: &[Point],
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn path(
        &mut self,
        path: &Path,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn image<'a>(&mut self, image: DrawImage<'a, Self::Color>) -> RenderResult;
}

// pub trait LayerRenderer {
//     fn on_layer(
//         &mut self,
//         index: usize,
//         f: impl FnOnce(&mut Self) -> RenderResult,
//     ) -> RenderResult;
// }

/// Minimal color, for [`NullRenderer`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NullColor;

impl Color for NullColor {
    const WHITE: Self = Self;
    const BLACK: Self = Self;

    fn default_foreground() -> Self {
        NullColor
    }

    fn default_background() -> Self {
        NullColor
    }

    fn from_rgba(rgba: crate::color::Rgba) -> Self {
        let _ = rgba;
        NullColor
    }

    fn into_rgba(&self) -> crate::color::Rgba {
        Rgba { r: 0, g: 0, b: 0, a: 0 }
    }

    fn accents() -> [Self; 6] {
        [NullColor; 6]
    }

    fn map(&self, _f: impl Fn(u8) -> u8) -> Self {
        *self
    }

    fn fold(&self, _other: Self, _f: impl Fn(u8, u8) -> u8) -> Self {
        *self
    }
}

/// A renderer that draws nothing, for headless tests and for passes that must
/// run widget bodies without rasterizing. `C` is generic so it can stand in for
/// the application's own color.
pub struct NullRenderer<C = NullColor> {
    _color: PhantomData<C>,
}

// Hand-written: `#[derive(Default)]` would demand `C: Default`.
impl<C> Default for NullRenderer<C> {
    fn default() -> Self {
        Self { _color: PhantomData }
    }
}

impl<C: Color> Renderer for NullRenderer<C> {
    type Color = C;

    /// Draws nothing, so no region is ever too large.
    type Policy = crate::region::Unbounded;

    fn size(&self) -> Size {
        Size::zero()
    }

    fn push_clip(&mut self, _area: Rect) {}

    fn pop_clip(&mut self) {}

    fn fill_solid(&mut self, _rect: Rect, _color: Self::Color) -> RenderResult {
        Ok(())
    }

    fn pixel(&mut self, _point: Point, _color: Self::Color) -> RenderResult {
        Ok(())
    }

    fn line(
        &mut self,
        _from: Point,
        _to: Point,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn rect(
        &mut self,
        _rect: Rect,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn rounded_rect(
        &mut self,
        _rect: Rect,
        _corners: CornerRadii,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn circle(
        &mut self,
        _top_left: Point,
        _diameter: u32,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn arc(
        &mut self,
        _top_left: Point,
        _diameter: u32,
        _start: Angle,
        _sweep: Angle,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn ellipse(
        &mut self,
        _bounding_box: Rect,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn sector(
        &mut self,
        _top_left: Point,
        _diameter: u32,
        _start: Angle,
        _sweep: Angle,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn polygon(
        &mut self,
        _points: &[Point],
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn path(
        &mut self,
        _path: &Path,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn image<'a>(
        &mut self,
        _image: DrawImage<'a, Self::Color>,
    ) -> RenderResult {
        Ok(())
    }
}

/// A [`Renderer`] that draws with a [`Rasterizer`] `R` into a [`Blitter`] `T`,
/// under frame policy `P`.
///
/// Build it once and keep it — it holds the clip stack and the rasterizer's
/// caches. The target is what comes and goes: [`attach`] takes a blitter,
/// [`detach`] gives it back, and only an attached renderer can draw.
///
/// ```
/// # use rsact_render::{blitter::FramebufBlitter, geometry::{Point, Rect, Size},
/// #                    eg::rasterizer::EgRasterizer, region::Unbounded,
/// #                    renderer::{RasterRenderer, Renderer}};
/// # use embedded_graphics::pixelcolor::Rgb888;
/// # type Screen = RasterRenderer<
/// #     EgRasterizer, FramebufBlitter<Rgb888, &'static mut [u32]>, Unbounded>;
/// # let viewport = Size::new_equal(16);
/// # let buf: &'static mut [u32] = vec![0; 16 * 16].leak();
/// let renderer = Screen::parked(EgRasterizer, viewport);   // once, at boot
/// let mut renderer = renderer.attach(FramebufBlitter::new(buf)).unwrap();
///
/// renderer.begin_region(Rect::new(Point::zero(), viewport)).unwrap();
/// renderer.fill_solid(Rect::new(Point::zero(), viewport), Rgb888::new(0, 0, 0))
///     .unwrap();
///
/// let (renderer, blitter) = renderer.detach();
/// let (buf, painted) = blitter.into_storage();             // yours again
/// # assert_eq!(painted, Rect::new(Point::zero(), viewport));
/// # assert_eq!(buf.len(), 16 * 16);
/// ```
///
/// [`attach`]: RasterRenderer::attach
/// [`detach`]: RasterRenderer::detach
/// [`Rasterizer`]: crate::raster::Rasterizer
/// [`Blitter`]: crate::blitter::Blitter
/// [`RasterCtx`]: crate::raster::RasterCtx
pub struct RasterRenderer<
    R,
    T,
    P = crate::region::Unbounded,
    A: Attachment<T> = Attached,
> {
    rasterizer: R,
    /// The lent target: `T` when [`Attached`], `()` when [`Detached`].
    blitter: A::Slot,
    /// Clip stack, seeded with the surface rect and never emptied. Push stores
    /// `area ∩ top`, so the top is always the effective clip and a nested clip
    /// can only narrow. Survives a detach.
    clips: alloc::vec::Vec<Rect>,
    /// The display's size, however small the blitter's storage is.
    viewport: Size,
    /// `fn() -> P` so the policy leaks neither dropck nor auto-traits.
    policy: PhantomData<fn() -> P>,
}

impl<R, T, P> RasterRenderer<R, T, P, Detached> {
    /// A renderer with no target yet. Unbounded, so it can be built before the
    /// blitter type is known to be a [`Blitter`](crate::blitter::Blitter).
    pub fn new(rasterizer: R, viewport: Size) -> Self {
        Self {
            rasterizer,
            blitter: (),
            clips: alloc::vec![Rect::new(Point::zero(), viewport)],
            viewport,
            policy: PhantomData,
        }
    }
}

/// Why a target could not be lent, handing both halves back. Only reachable for
/// a target whose type cannot state its capacity — a `&'static mut [u16; N]` is
/// proved at compile time instead.
pub struct AttachError<R, T, P> {
    /// Still parked, still holding its clip stack.
    pub renderer: RasterRenderer<R, T, P, Detached>,
    /// The target, untouched.
    pub blitter: T,
    /// Units the frame policy's largest region needs.
    pub needed: usize,
    /// Units the target holds.
    pub available: usize,
}

// Hand-written so `R` and `T` need not be `Debug`.
impl<R, T, P> core::fmt::Debug for AttachError<R, T, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "AttachError {{ needed: {}, available: {} }}",
            self.needed, self.available
        )
    }
}

impl<R, T, P> RasterRenderer<R, T, P, Detached>
where
    T: crate::blitter::Blitter,
    P: crate::region::FramePolicy,
{
    /// Lend the renderer a target, returning the [`Attached`] renderer.
    ///
    /// Where the target is checked against the policy:
    ///
    /// | the target | when |
    /// |---|---|
    /// | `&'static mut [u16; 5760]` | compile time — `UNITS` is `Some` |
    /// | `&'static mut [u16]`, a `Pixmap` | here, as an `Err` |
    /// | a direct-to-panel blitter | never — no storage to overflow |
    ///
    /// …and against **what**: the policy's largest region where it names one,
    /// the viewport where it does not. [`Unbounded`](crate::region::Unbounded) is
    /// the second case, and it is a real bound rather than a waiver —
    /// `plan_regions` clamps every region to the viewport, so a viewport-sized
    /// region is the largest that can arrive.
    ///
    /// A **packing** disagreement is always a compile error, both
    /// `PIXELS_PER_UNIT` being consts:
    ///
    /// ```compile_fail
    /// # use rsact_render::{blitter::FramebufBlitter, geometry::Size,
    /// #                    eg::rasterizer::EgRasterizer, region::Tiles,
    /// #                    renderer::RasterRenderer};
    /// # use embedded_graphics::pixelcolor::BinaryColor;
    /// // 1-bpp storage under a policy counting one pixel per unit; the buffer
    /// // is far from too small.
    /// let buf: &'static mut [u8] = vec![0u8; 4096].leak();
    /// let _ = RasterRenderer::<
    ///     EgRasterizer,
    ///     FramebufBlitter<BinaryColor, &'static mut [u8]>,
    ///     Tiles<240, 24>,
    /// >::with_blitter(
    ///     EgRasterizer, Size::new_equal(240), FramebufBlitter::new(buf),
    /// );
    /// ```
    ///
    /// # Errors
    ///
    /// [`AttachError`] when the target's *value* is too small. Never panics —
    /// whether that should abort is the caller's `unwrap` to write.
    pub fn attach(
        self,
        blitter: T,
    ) -> Result<RasterRenderer<R, T, P, Attached>, AttachError<R, T, P>> {
        // Only codegen evaluates a `const` block, so this is a build failure
        // that `cargo check` and rust-analyzer will not show.
        const {
            if let Some(needed) = crate::region::policy_units::<P>() {
                assert!(
                    P::PIXELS_PER_UNIT == T::PIXELS_PER_UNIT,
                    "this frame policy's pixel packing disagrees with the \
                     target's — a 1-bpp target under a 1-pixel-per-unit policy \
                     would appear to need eight times the storage it does. See \
                     the instantiation in this error for both types"
                );
                if let Some(units) = T::UNITS {
                    assert!(
                        needed <= units,
                        "this target is too small for the renderer's frame \
                         policy — see the instantiation in this error for the \
                         target type and the policy"
                    );
                }
            }
        }

        // Only for a target whose type could not answer. `capacity() == None`
        // is *unbounded*, unlike `UNITS`'s "ask the value".
        //
        // The bound is the policy's largest region where it states one, and the
        // **viewport** where it does not. `Unbounded` is the latter: it means
        // "any region fits", so `policy_units` is `None`, so neither half of the
        // const check above runs and an undersized slice used to be accepted
        // outright. `plan_regions` clamps every region to the viewport, so a
        // viewport-sized region is exactly the largest that can ever arrive —
        // this is the honest bound, not a heuristic.
        //
        // A target with no storage at all (`capacity() == None`) stays exempt:
        // there is nothing to overflow.
        let needed = crate::region::policy_units::<P>().unwrap_or_else(|| {
            region_units(
                self.viewport.width,
                self.viewport.height,
                T::PIXELS_PER_UNIT,
            )
        });
        if T::UNITS.is_none()
            && let Some(available) = blitter.capacity()
            && available < needed
        {
            return Err(AttachError {
                renderer: self,
                blitter,
                needed,
                available,
            });
        }

        Ok(RasterRenderer {
            rasterizer: self.rasterizer,
            blitter,
            clips: self.clips,
            viewport: self.viewport,
            policy: PhantomData,
        })
    }
}

impl<R, T, P> RasterRenderer<R, T, P, Attached>
where
    T: crate::blitter::Blitter,
    P: crate::region::FramePolicy,
{
    /// The renderer before it has a target — the two-step form of
    /// [`with_blitter`](Self::with_blitter), for targets that arrive later.
    ///
    /// Spelled on the attached type, which is the one an application names, so
    /// reaching the parked state needs no `Detached` turbofish.
    pub fn parked(
        rasterizer: R,
        viewport: Size,
    ) -> RasterRenderer<R, T, P, Detached> {
        RasterRenderer::new(rasterizer, viewport)
    }

    /// [`parked`](Self::parked) + [`attach`](RasterRenderer::attach), for a
    /// caller that already holds its target.
    pub fn with_blitter(
        rasterizer: R,
        viewport: Size,
        blitter: T,
    ) -> Result<Self, AttachError<R, T, P>> {
        RasterRenderer::<R, T, P, Detached>::new(rasterizer, viewport)
            .attach(blitter)
    }

    /// Take the target back, [`Detached`].
    ///
    /// The blitter owns what was lent, so ownership moves out with it — DMA
    /// needs that, a borrow the core could still write through being UB. Unwrap
    /// it with `FramebufBlitter::into_storage` or `PixmapBlitter::into_pixmap`,
    /// or read it in place and re-attach it whole.
    pub fn detach(self) -> (RasterRenderer<R, T, P, Detached>, T) {
        (
            RasterRenderer {
                rasterizer: self.rasterizer,
                blitter: (),
                clips: self.clips,
                viewport: self.viewport,
                policy: PhantomData,
            },
            self.blitter,
        )
    }

    /// The attached target, to read without giving it back.
    pub fn blitter(&self) -> &T {
        &self.blitter
    }

    /// The absolute rect the target currently covers — what to flush, and where.
    pub fn covers(&self) -> Rect {
        self.blitter.bounds()
    }

    /// The rasterizer, for a caller that needs to configure it.
    pub fn rasterizer(&mut self) -> &mut R {
        &mut self.rasterizer
    }

    /// The effective clip: the top of the stack.
    fn clip(&self) -> Rect {
        // Never empty — the root is never popped. `unwrap_or` rather than
        // `expect` because nothing on the render path may panic.
        self.clips.last().copied().unwrap_or(Rect::zero())
    }

    /// Split into the rasterizer and a clip-gated view of the blitter.
    ///
    /// **Hot path** — one call per glyph pixel, each paying the
    /// `Rect::intersection` in `RasterCtx::new`. That is redundant here, since
    /// `clip ⊆ bounds()` already holds, but it is what guarantees the clip for
    /// every other caller.
    fn split(&mut self) -> (&mut R, crate::raster::RasterCtx<'_, T>) {
        let clip = self.clip();
        (
            &mut self.rasterizer,
            crate::raster::RasterCtx::new(&mut self.blitter, clip),
        )
    }

    /// Whether a primitive bounded by `bounds` can be skipped.
    ///
    /// Grows `bounds` by the stroke width first:
    /// [`StrokeAlignment::Outside`](crate::style::StrokeAlignment::Outside)
    /// paints outside the geometry, so culling on the bare rect would clip a
    /// border away at a region edge.
    fn culled<C: Color>(&self, bounds: Rect, style: &DrawStyle<C>) -> bool {
        let grow = style.stroke_width as i32;
        let bounds = Rect::new(
            Point::new(
                bounds.top_left.x.saturating_sub(grow),
                bounds.top_left.y.saturating_sub(grow),
            ),
            Size::new(
                bounds.size.width.saturating_add(style.stroke_width * 2),
                bounds.size.height.saturating_add(style.stroke_width * 2),
            ),
        );
        !bounds.intersects(&self.clip())
    }
}

/// Every geometry method is the same three lines: cull, split, forward.
macro_rules! forward_to_rasterizer {
    (
        $( fn $name:ident ( $( $arg:ident : $ty:ty ),* $(,)? )
             $( bounded by $bounds:expr )? ; )*
    ) => {
        $(
            fn $name(
                &mut self,
                $( $arg : $ty, )*
                style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                $( if self.culled($bounds, style) { return Ok(()) } )?
                let (rasterizer, mut cx) = self.split();
                rasterizer.$name(&mut cx, $( $arg, )* style);
                Ok(())
            }
        )*
    };
}

impl<R, T, P> Renderer for RasterRenderer<R, T, P, Attached>
where
    R: crate::raster::Rasterizer<T>,
    T: crate::blitter::Blitter,
    P: crate::region::FramePolicy,
{
    type Color = T::Color;
    type Policy = P;

    fn size(&self) -> Size {
        self.viewport
    }

    /// Aim the blitter, then make the region the **root** of the clip stack.
    ///
    /// Neither step paints. The caller writes the region's background itself,
    /// through the normal clipped path, which is why the clip is established
    /// here and nothing needs an unclipped one — see
    /// [`Blitter::begin_region`](crate::blitter::Blitter::begin_region).
    fn begin_region(&mut self, region: Rect) -> RenderResult {
        self.blitter.begin_region(region)?;
        self.clips.clear();
        self.clips.push(region);
        Ok(())
    }

    /// Where a batching rasterizer would flush; nothing does yet.
    fn end_region(&mut self) -> RenderResult {
        Ok(())
    }

    fn push_clip(&mut self, area: Rect) {
        let nested = area.intersection(&self.clip());
        self.clips.push(nested);
    }

    fn pop_clip(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
        }
    }

    /// Always `Some`: a `Rect::MAX` sentinel could not mean "unbounded", since
    /// `u32::MAX as i32 == -1` makes it intersect to `Rect::zero()`.
    fn clip_bounds(&self) -> Option<Rect> {
        Some(self.clip())
    }

    fn fill_solid(&mut self, rect: Rect, color: Self::Color) -> RenderResult {
        if !rect.intersects(&self.clip()) {
            return Ok(());
        }
        let (rasterizer, mut cx) = self.split();
        rasterizer.fill(&mut cx, rect, color);
        Ok(())
    }

    fn pixel(&mut self, point: Point, color: Self::Color) -> RenderResult {
        let (rasterizer, mut cx) = self.split();
        rasterizer.pixel(&mut cx, point, color);
        Ok(())
    }

    forward_to_rasterizer! {
        // Only where the bound is exact and cheap. For `line`, `polygon` and
        // `path` computing it costs about what testing it saves.
        fn line(from: Point, to: Point);
        fn rect(rect: Rect) bounded by rect;
        fn rounded_rect(rect: Rect, corners: CornerRadii) bounded by rect;
        fn circle(top_left: Point, diameter: u32)
            bounded by Rect::new(top_left, Size::new_equal(diameter));
        fn arc(top_left: Point, diameter: u32, start: Angle, sweep: Angle)
            bounded by Rect::new(top_left, Size::new_equal(diameter));
        fn sector(top_left: Point, diameter: u32, start: Angle, sweep: Angle)
            bounded by Rect::new(top_left, Size::new_equal(diameter));
        fn ellipse(bounding_box: Rect) bounded by bounding_box;
        fn polygon(points: &[Point]);
        fn path(path: &Path);
    }

    fn image<'a>(&mut self, image: DrawImage<'a, Self::Color>) -> RenderResult {
        if !image.bounding_box().intersects(&self.clip()) {
            return Ok(());
        }
        let (rasterizer, mut cx) = self.split();
        rasterizer.image(&mut cx, image);
        Ok(())
    }
}

#[cfg(all(test, feature = "embedded-graphics"))]
mod raster_renderer_tests {
    use super::*;
    use crate::{
        blitter::FramebufBlitter,
        eg::rasterizer::EgRasterizer,
        framebuf::PackedColor,
        region::{FramePolicy, Tiles, Unbounded},
        style::DrawStyle,
    };
    use embedded_graphics::pixelcolor::Rgb888;

    /// Named once: several colors share `Storage = u32`, so the buffer type
    /// cannot imply the color.
    type Fb = FramebufBlitter<Rgb888, &'static mut [u32]>;
    type Full = RasterRenderer<EgRasterizer, Fb, Unbounded>;
    type TiledBy<const W: u32, const H: u32> =
        RasterRenderer<EgRasterizer, Fb, Tiles<W, H>>;

    /// Both steps in one: a renderer, and a blitter over the caller's storage.
    fn build<P: FramePolicy>(
        viewport: Size,
        storage: &'static mut [u32],
    ) -> RasterRenderer<EgRasterizer, Fb, P> {
        RasterRenderer::<EgRasterizer, Fb, P>::parked(EgRasterizer, viewport)
            .attach(FramebufBlitter::new(storage))
            .unwrap()
    }

    /// Build, aim at the whole frame, and paint its background — the three
    /// things `Page::paint_region` does, in its order.
    ///
    /// A blitter starts aimed at nothing, so a test that skips the aim writes
    /// into a zero-sized target; and `begin_region` no longer primes, so a test
    /// that skips the fill compares against whatever the storage held.
    fn build_full(
        viewport: Size,
        storage: &'static mut [u32],
    ) -> RasterRenderer<EgRasterizer, Fb, Unbounded> {
        let mut r = build::<Unbounded>(viewport, storage);
        let frame = Rect::new(Point::zero(), viewport);
        r.begin_region(frame)
            .expect("a full-frame buffer holds the full frame");
        Renderer::fill_solid(
            &mut r,
            frame,
            <Rgb888 as Color>::default_background(),
        )
        .expect("the background fill is the caller's, not the blitter's");
        r
    }

    /// Detach and unwrap in one step: the parked renderer, the storage, and the
    /// rect it covers.
    fn take<P: FramePolicy>(
        r: RasterRenderer<EgRasterizer, Fb, P>,
    ) -> (RasterRenderer<EgRasterizer, Fb, P, Detached>, &'static mut [u32], Rect)
    {
        let (parked, blitter) = r.detach();
        let (storage, at) = blitter.into_storage();
        (parked, storage, at)
    }

    /// A `&'static mut` loan — `Vec::leak` here, a `StaticCell` on a device.
    fn surface_units<C: Color + PackedColor>(
        units: usize,
    ) -> &'static mut [<C as PackedColor>::Storage] {
        alloc::vec![C::default_background().into_storage(); units].leak()
    }

    fn surface<C: Color + PackedColor>(
        size: Size,
    ) -> &'static mut [<C as PackedColor>::Storage] {
        surface_units::<C>(crate::framebuf::units_for::<C>(
            size.width,
            size.height,
        ))
    }

    /// Reaches every path that can differ: the whole-word `fill_solid`, a
    /// stroked `rect`, a diagonal `line`, and bare `pixel` writes.
    fn content<R: Renderer<Color = Rgb888>>(r: &mut R) {
        Renderer::fill_solid(
            r,
            Rect::new(Point::new(6, 10), Size::new(50, 30)),
            Rgb888::new(200, 30, 30),
        )
        .unwrap();
        Renderer::rect(
            r,
            Rect::new(Point::new(2, 2), Size::new(60, 60)),
            &DrawStyle::default()
                .stroke(Rgb888::new(20, 220, 40))
                .stroke_width(2),
        )
        .unwrap();
        Renderer::line(
            r,
            Point::new(0, 0),
            Point::new(63, 63),
            &DrawStyle::default()
                .stroke(Rgb888::new(10, 40, 250))
                .stroke_width(1),
        )
        .unwrap();
        for i in 0..40i32 {
            Renderer::pixel(
                r,
                Point::new(i, 63 - i),
                Rgb888::new(250, 250, 10),
            )
            .unwrap();
        }
    }

    /// A surface a fraction of the frame's size paints the same pixels as a
    /// full one.
    #[test]
    fn a_tiled_layered_renderer_paints_what_a_full_one_does() {
        const W: u32 = 64;
        const H: u32 = 64;
        const BAND: u32 = 8;
        let viewport = Size::new(W, H);

        struct Map {
            px: alloc::vec::Vec<Option<Rgb888>>,
        }
        fn blit(map: &mut Map, units: &[u32], region: Rect) {
            let w = region.size.width as usize;
            for row in 0..region.size.height as usize {
                for col in 0..w {
                    let x = region.top_left.x + col as i32;
                    let y = region.top_left.y + row as i32;
                    if x < 0 || y < 0 || x as u32 >= W || y as u32 >= H {
                        continue;
                    }
                    map.px[y as usize * W as usize + x as usize] =
                        Some(<Rgb888 as PackedColor>::as_color(
                            &units[row * w + col],
                            0,
                        ));
                }
            }
        }
        let blank = || Map { px: alloc::vec![None; (W * H) as usize] };

        let mut full = build_full(viewport, surface::<Rgb888>(viewport));
        content(&mut full);
        let mut full_map = blank();
        let (_, full_units, full_at) = take(full);
        blit(&mut full_map, &full_units, full_at);

        const TILE_UNITS: usize = (W * BAND) as usize;
        let mut tiled = build::<Tiles<W, BAND>>(
            viewport,
            surface_units::<Rgb888>(TILE_UNITS),
        );
        let mut tiled_map = blank();
        let mut spare = surface_units::<Rgb888>(TILE_UNITS);

        for band in 0..(H / BAND) as i32 {
            let region = Rect::new(
                Point::new(0, band * BAND as i32),
                Size::new(W, BAND),
            );
            tiled.begin_region(region).unwrap();
            tiled.push_clip(region);
            // The caller's background fill. Load-bearing from the second band
            // on: buffers are recycled below, so a region arrives holding the
            // previous one's pixels and `begin_region` no longer clears them.
            Renderer::fill_solid(
                &mut tiled,
                region,
                <Rgb888 as Color>::default_background(),
            )
            .unwrap();
            content(&mut tiled);
            tiled.pop_clip();
            tiled.end_region().unwrap();
            // Publish, then acquire — the ordering the loan API exists for.
            let (parked, units, at) = take(tiled);
            blit(&mut tiled_map, &units, at);
            tiled = parked.attach(FramebufBlitter::new(spare)).unwrap();
            spare = units;
        }

        let painted = |m: &Map| m.px.iter().filter(|p| p.is_some()).count();
        assert!(
            painted(&full_map) > (W * H) as usize / 3,
            "the reference frame painted only {} pixels",
            painted(&full_map)
        );
        let mismatches: alloc::vec::Vec<usize> = (0..(W * H) as usize)
            .filter(|&i| full_map.px[i] != tiled_map.px[i])
            .collect();
        assert!(
            mismatches.is_empty(),
            "{} of {} pixels differ between a full surface and a tiled one; \
             first at ({}, {})",
            mismatches.len(),
            W * H,
            mismatches[0] % W as usize,
            mismatches[0] / W as usize,
        );
    }

    /// A region is the **root** of the clip stack, so nothing can escape it —
    /// including an unbalanced `pop_clip`, which must degrade rather than empty
    /// the stack.
    #[test]
    fn a_region_is_the_root_of_the_clip_stack() {
        let viewport = Size::new(64, 64);
        let mut r =
            build::<Tiles<64, 8>>(viewport, surface_units::<Rgb888>(64 * 8));
        let region = Rect::new(Point::new(0, 16), Size::new(64, 8));
        r.begin_region(region).unwrap();
        assert_eq!(r.clip_bounds(), Some(region));

        // A nested clip narrows and never widens.
        r.push_clip(Rect::new(Point::new(0, 0), Size::new(64, 64)));
        assert_eq!(
            r.clip_bounds(),
            Some(region),
            "a clip wider than the region widened the effective clip"
        );
        r.push_clip(Rect::new(Point::new(8, 16), Size::new(16, 4)));
        assert_eq!(
            r.clip_bounds(),
            Some(Rect::new(Point::new(8, 16), Size::new(16, 4)))
        );

        // Unbalanced pops degrade to the region, never below it.
        r.pop_clip();
        r.pop_clip();
        r.pop_clip();
        r.pop_clip();
        assert_eq!(
            r.clip_bounds(),
            Some(region),
            "the region must survive an unbalanced pop — it is the root"
        );
        Renderer::pixel(&mut r, Point::new(1, 17), Rgb888::WHITE).unwrap();
    }

    /// The whole-word `fill_solid` must land the same pixels as the per-pixel
    /// path, through four hops that can each clip or shift the rect.
    #[test]
    fn fill_solid_matches_per_pixel() {
        let size = Size::new(20, 16);
        let rect = Rect::new(Point::new(3, 2), Size::new(9, 7));
        let color = Rgb888::new(10, 200, 30);

        let mut fast = build_full(size, surface::<Rgb888>(size));
        Renderer::fill_solid(&mut fast, rect, color).unwrap();

        let mut slow = build_full(size, surface::<Rgb888>(size));
        for p in rect.points() {
            Renderer::pixel(&mut slow, p, color).unwrap();
        }

        let (_, fast_units, _) = take(fast);
        let (_, slow_units, _) = take(slow);
        assert_eq!(fast_units, slow_units, "fill_solid != per-pixel fill");
    }

    /// The renderer never allocates a surface and always gives it back.
    #[test]
    fn the_renderer_gives_the_surface_back() {
        let viewport = Size::new(16, 16);
        let mut r = build_full(viewport, surface::<Rgb888>(viewport));

        let ink = Rgb888::new(9, 9, 9);
        Renderer::fill_solid(&mut r, Rect::new(Point::zero(), viewport), ink)
            .unwrap();
        let (parked, buffer, dirty) = take(r);
        assert_eq!(buffer.len(), 16 * 16);
        assert_eq!(
            dirty,
            Rect::new(Point::zero(), viewport),
            "a full-frame surface reports the whole frame as its dirty region"
        );
        assert!(
            buffer.iter().all(|u| *u == ink.into_storage()),
            "the owner got back a buffer that does not hold what was painted"
        );

        // Painting while parked has nothing to assert: no drawing methods.
        let _ =
            parked.attach(FramebufBlitter::new(surface::<Rgb888>(viewport)));
    }

    /// After `begin_region` a buffer covers exactly the region it was asked to
    /// paint — for a full-frame surface too, which a test using one surface kind
    /// at a time cannot see.
    #[test]
    fn what_a_buffer_covers_is_the_region_it_painted() {
        let viewport = Size::new(64, 64);
        let region = Rect::new(Point::new(8, 24), Size::new(16, 8));

        let mut full =
            build::<Unbounded>(viewport, surface::<Rgb888>(viewport));
        full.begin_region(region).unwrap();
        let (_, _, covers) = take(full);
        assert_eq!(
            covers, region,
            "a frame-sized buffer must report the region, not the frame"
        );

        let mut tile =
            build::<Tiles<16, 8>>(viewport, surface_units::<Rgb888>(16 * 8));
        tile.begin_region(region).unwrap();
        let (_, _, covers) = take(tile);
        assert_eq!(covers, region);
    }

    /// A region's units are strided at **its own width**.
    ///
    /// A differential test cannot reach this: chunking preserves width, so both
    /// sides of any comparison use the wrong stride identically. The region is
    /// narrow and tall so a frame-width stride overruns rather than skews.
    #[test]
    fn a_narrow_region_is_laid_out_at_its_own_width() {
        let viewport = Size::new(64, 64);
        let region = Rect::new(Point::new(40, 8), Size::new(5, 9));

        let mut r =
            build::<Tiles<8, 16>>(viewport, surface_units::<Rgb888>(8 * 16));
        r.begin_region(region).unwrap();

        let color_at = |x: i32, y: i32| {
            Rgb888::new(
                (x as u8).wrapping_mul(7),
                (y as u8).wrapping_mul(11),
                3,
            )
        };
        for p in region.points() {
            Renderer::pixel(&mut r, p, color_at(p.x, p.y)).unwrap();
        }

        let (_, units, at) = take(r);
        assert_eq!(at, region);

        let stride = region.size.width as usize;
        for row in 0..region.size.height as usize {
            for col in 0..stride {
                let want = color_at(
                    region.top_left.x + col as i32,
                    region.top_left.y + row as i32,
                );
                let got = <Rgb888 as PackedColor>::as_color(
                    &units[row * stride + col],
                    0,
                );
                assert_eq!(
                    got, want,
                    "unit at row {row}, col {col} — rows must be strided at \
                     the REGION's width, not the frame's",
                );
            }
        }
    }

    /// **`begin_region` is the only thing that aims a target.** A reattached
    /// one covers nothing until a region is begun, and exactly that after.
    #[test]
    fn a_target_is_aimed_by_begin_region_and_by_nothing_else() {
        let viewport = Size::new(16, 16);
        let region = Rect::new(Point::new(4, 4), Size::new(8, 8));

        let r = build::<Unbounded>(viewport, surface::<Rgb888>(viewport));
        let (parked, buffer, before) = take(r);
        assert_eq!(
            before,
            Rect::zero(),
            "a freshly wrapped target covers nothing until a region is begun"
        );

        let mut r = parked.attach(FramebufBlitter::new(buffer)).unwrap();
        r.begin_region(region).unwrap();
        let (_, _, after) = take(r);
        assert_eq!(after, region, "and exactly the region after");
    }

    /// **Aiming is all `begin_region` does.** It used to prime the surface with
    /// `Color::default_background()`, which put a style decision in this crate:
    /// a page whose background is not the colour default could not paint it,
    /// because the prime had already run and the page's own fill was the one
    /// that got discarded. rsact-ui paints the region background now
    /// (`Page::paint_region`), so what remains here is pure addressing.
    ///
    /// The invariant moves with it: **the caller must write every pixel of a
    /// region before flushing it.** Nothing here can enforce that — a colour to
    /// enforce it with is exactly what does not belong in a renderer.
    #[test]
    fn beginning_a_region_writes_no_pixels() {
        let viewport = Size::new(16, 16);
        // Zeroed, not background-filled: `surface` pre-fills with the colour
        // default, which is what a prime would have written — indistinguishable.
        let buffer: &'static mut [u32] = alloc::vec![0u32; 16 * 16].leak();

        let mut r = build::<Unbounded>(viewport, buffer);
        r.begin_region(Rect::new(Point::zero(), viewport)).unwrap();
        let (_, units, at) = take(r);

        assert_eq!(at, Rect::new(Point::zero(), viewport), "aimed");
        assert!(
            units.iter().all(|&u| u == 0),
            "and wrote nothing: {} of {} units were touched",
            units.iter().filter(|&&u| u != 0).count(),
            units.len()
        );
    }

    /// **`Unbounded` still bounds the surface — by the viewport.**
    ///
    /// `policy_units::<Unbounded>()` is `None` ("any region fits"), so neither
    /// half of the policy check runs and a 4-unit slice used to be accepted for
    /// a 64x64 frame. `plan_regions` then emitted a 64x64 region,
    /// `begin_region` refused it, and `Frame::render` had already advanced its
    /// cursor — so the damage was dropped for good and nothing repainted that
    /// rectangle until whatever is underneath happened to change.
    ///
    /// The viewport is the right bound and an exact one, not a guess:
    /// `plan_regions` clamps every region to it
    /// (`damage_outside_the_viewport_is_clipped_away`, and the fuzz assertion
    /// `planned.iter().all(|r| r.intersection(&viewport) == *r)`), so the
    /// viewport-sized region is the largest that can ever arrive.
    #[test]
    fn an_unbounded_policy_still_needs_a_surface_the_viewport_fits() {
        let viewport = Size::new_equal(64);
        let tiny: &'static mut [u32] = alloc::vec![0u32; 4].leak();

        let parked = Full::parked(EgRasterizer, viewport);
        // `Result::expect_err` would need the Ok side to be `Debug`, and
        // `RasterRenderer` deliberately is not (`R`/`T` need not be).
        let Err(err) = parked.attach(FramebufBlitter::new(tiny)) else {
            panic!("4 units cannot hold a 64x64 frame");
        };
        assert_eq!(err.needed, 64 * 64);
        assert_eq!(err.available, 4);

        // Exactly enough is enough — the bound is the viewport, not more.
        let exact: &'static mut [u32] = alloc::vec![0u32; 64 * 64].leak();
        assert!(
            Full::parked(EgRasterizer, viewport)
                .attach(FramebufBlitter::new(exact))
                .is_ok()
        );
    }

    /// A target with no storage bound at all is still exempt: there is nothing
    /// to overflow, and a direct-to-panel blitter has no capacity to state.
    #[test]
    fn a_storage_free_target_is_not_measured_against_the_viewport() {
        struct Panel(Rect);
        impl crate::blitter::Blitter for Panel {
            type Color = Rgb888;
            fn bounds(&self) -> Rect {
                self.0
            }
            fn capacity(&self) -> Option<usize> {
                None
            }
            fn fill_span(&mut self, _span: crate::blitter::Span, _c: Rgb888) {}
            fn begin_region(&mut self, region: Rect) -> RenderResult {
                self.0 = region;
                Ok(())
            }
        }

        let r = RasterRenderer::<EgRasterizer, Panel, Unbounded>::parked(
            EgRasterizer,
            Size::new_equal(240),
        )
        .attach(Panel(Rect::zero()));
        assert!(r.is_ok(), "a panel with no storage cannot be too small");
    }

    /// A surface is checked against the policy the renderer declares. Spelling
    /// an unstatable capacity `usize::MAX` instead of `None` would exempt every
    /// heap surface — an empty boxed slice satisfying a full-frame policy.
    #[test]
    fn a_renderer_declares_the_regions_it_accepts() {
        use crate::framebuf::FramebufStorage;

        assert_eq!(
            <<Full as Renderer>::Policy as FramePolicy>::MAX_REGION,
            None,
            "a full-frame renderer accepts anything, which is honest"
        );

        assert_eq!(
            <<TiledBy<240, 24> as Renderer>::Policy as FramePolicy>::MAX_REGION,
            Some(Size::new(240, 24))
        );
        assert_eq!(crate::region::policy_units::<Tiles<240, 24>>(), Some(5760));
        assert_eq!(crate::region::policy_units::<Unbounded>(), None);

        assert_eq!(
            <&mut [u32] as FramebufStorage<Rgb888>>::UNITS,
            None,
            "a runtime-sized surface must not state a compile-time capacity"
        );
        assert_eq!(
            <&mut [u32; 5760] as FramebufStorage<Rgb888>>::UNITS,
            Some(5760)
        );
    }

    /// A nested clip is narrowed by its parent, never replaces it. Asserted on
    /// the framebuffer, not the stack: a pixel inside the inner clip but outside
    /// the outer one must not land.
    #[test]
    fn a_nested_clip_narrows_and_never_widens() {
        let size = Size::new(40, 40);
        let mut r = build_full(size, surface::<Rgb888>(size));

        r.push_clip(Rect::new(Point::new(0, 0), Size::new(20, 20)));
        // Overlaps the parent over (10,10)..(20,20) and reaches BEYOND it.
        r.push_clip(Rect::new(Point::new(10, 10), Size::new(20, 20)));
        assert_eq!(
            r.clip_bounds(),
            Some(Rect::new(Point::new(10, 10), Size::new(10, 10))),
            "the effective clip is the intersection, not the inner rect"
        );

        // Must differ from the untouched framebuffer (WHITE for RGB), or the
        // assertions below pass vacuously.
        let bg = <Rgb888 as Color>::default_background();
        let ink = <Rgb888 as Color>::default_foreground();
        assert_ne!(ink, bg, "the probe color must be visible");

        Renderer::pixel(&mut r, Point::new(25, 15), ink).unwrap(); // outside parent
        Renderer::pixel(&mut r, Point::new(15, 15), ink).unwrap(); // inside both

        // Popping restores the parent, not the raw inner rect.
        r.pop_clip();
        assert_eq!(
            r.clip_bounds(),
            Some(Rect::new(Point::new(0, 0), Size::new(20, 20)))
        );
        r.pop_clip();
        assert_eq!(
            r.clip_bounds(),
            Some(Rect::new(Point::zero(), size)),
            "the root reports the surface rect"
        );
        // Unmatched pops degrade rather than emptying the stack.
        r.pop_clip();
        r.pop_clip();
        assert_eq!(r.clip_bounds(), Some(Rect::new(Point::zero(), size)));

        let (_, units, _) = take(r);
        let at = |x: usize, y: usize| {
            <Rgb888 as PackedColor>::as_color(&units[y * 40 + x], 0)
        };
        assert_eq!(
            at(25, 15),
            bg,
            "a write outside the PARENT clip escaped the nested clip"
        );
        assert_eq!(at(15, 15), ink);
    }

    /// A target too small for the policy is refused when lent, as an `Err` with
    /// both halves handed back. Reachable only for a runtime-length slice; a
    /// `&mut [u32; 240]` would not compile.
    #[test]
    fn a_target_too_small_for_the_policy_is_refused_with_an_err() {
        use crate::blitter::Blitter as _;

        let err = RasterRenderer::<_, Fb, Tiles<240, 24>>::parked(
            EgRasterizer,
            Size::new_equal(240),
        )
        .attach(FramebufBlitter::new(surface_units::<Rgb888>(240)))
        .err()
        .expect("240 units cannot hold a 240x24 tile");

        assert_eq!(err.needed, 5760);
        assert_eq!(err.available, 240);
        assert_eq!(err.blitter.capacity(), Some(240));
        let _ = err.renderer;
    }
}
