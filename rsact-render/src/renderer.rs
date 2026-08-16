use crate::{
    color::{Color, Rgba},
    geometry::*,
    image::DrawImage,
    path::Path,
    style::DrawStyle,
};

use core::marker::PhantomData;

pub type RenderResult = Result<(), ()>;

/// Storage units a `w × h` region needs on a surface packing
/// `pixels_per_unit` pixels per unit — **rows padded**, never area.
///
/// Per-row padding is what makes sub-byte packing correct: a 122-pixel 1-bpp
/// row occupies 16 bytes, not 15.25. Area arithmetic gets this wrong.
///
/// ```
/// # use rsact_render::renderer::region_units;
/// assert_eq!(region_units(240, 24, 1), 5760); // RGB565: one unit per pixel
/// assert_eq!(region_units(122, 24, 8), 384);  // 1-bpp: 16 bytes per row
/// ```
pub const fn region_units(w: u32, h: u32, pixels_per_unit: usize) -> usize {
    // Zero would divide by zero; treat it as unpacked, which over-estimates
    // the requirement and so fails safe.
    let pps = if pixels_per_unit == 0 { 1 } else { pixels_per_unit };
    // `div_ceil` written out: keeps this a plain const fn on stable.
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

/// Whether a renderer is holding its surface. You never name this yourself —
/// [`attach`](RasterRenderer::attach) and [`detach`](RasterRenderer::detach)
/// move a renderer between [`Attached`] and [`Detached`].
pub trait Attachment<S> {
    /// The surface field's type in this state: `S` attached, `()` detached.
    type Slot;
}

/// The renderer is holding a surface and can draw.
///
/// Drawing methods exist only in this state, so painting into a buffer you have
/// taken back is a compile error rather than a lost frame:
///
/// ```
/// # use rsact_render::{blitter::FramebufBlitter, geometry::Size,
/// #                    eg::rasterizer::EgRasterizer, region::Unbounded,
/// #                    renderer::{RasterRenderer, Renderer}};
/// # use embedded_graphics::pixelcolor::Rgb888;
/// # type Screen = RasterRenderer<
/// #     EgRasterizer, FramebufBlitter<Rgb888, &'static mut [u32]>, Unbounded>;
/// let size = Size::new_equal(16);
/// let buf: &'static mut [u32] = vec![0; 16 * 16].leak();
/// // `with_blitter` is sugar for `parked(..).attach(..)`; the two-step form
/// // is what an app whose targets arrive from a channel uses.
/// let r = Screen::with_blitter(EgRasterizer, size, FramebufBlitter::new(buf))
///     .unwrap();
/// // Attached: drawing is available.
/// let _ = Renderer::size(&r);
/// ```
///
/// and painting after a `detach` is not a logged no-op but a compile error:
///
/// ```compile_fail
/// # use rsact_render::{blitter::FramebufBlitter, geometry::Size,
/// #                    eg::rasterizer::EgRasterizer, region::Unbounded,
/// #                    renderer::{RasterRenderer, Renderer}};
/// # use embedded_graphics::pixelcolor::Rgb888;
/// # type Screen = RasterRenderer<
/// #     EgRasterizer, FramebufBlitter<Rgb888, &'static mut [u32]>, Unbounded>;
/// let size = Size::new_equal(16);
/// let buf: &'static mut [u32] = vec![0; 16 * 16].leak();
/// // `with_blitter` is sugar for `parked(..).attach(..)`; the two-step form
/// // is what an app whose targets arrive from a channel uses.
/// let r = Screen::with_blitter(EgRasterizer, size, FramebufBlitter::new(buf))
///     .unwrap();
/// let (parked, _blitter) = r.detach();
/// // The application is holding the buffer — there is nothing to draw into.
/// let _ = Renderer::size(&parked);
/// ```
pub struct Attached;

impl<S> Attachment<S> for Attached {
    type Slot = S;
}

/// The owner is holding the surface; the renderer keeps only its configuration.
///
/// Not an error case — it is where the buffer lives while it is being shipped
/// over SPI, encoded to a PNG, or waited on.
pub struct Detached;

impl<S> Attachment<S> for Detached {
    type Slot = ();
}

/// Primitive drawing, independent of any backend.
///
/// Clips are a stack: [`push_clip`](Renderer::push_clip) stores `area ∩ top`,
/// [`pop_clip`](Renderer::pop_clip) never pops the root, and the root is the
/// region currently being painted.
pub trait Renderer {
    type Color: Color;

    /// The largest region this renderer will accept.
    ///
    /// Answer [`Unbounded`] unless the renderer paints through a buffer smaller
    /// than the frame; a `Tiles<W, H>` answer makes the planner cut regions down
    /// to that budget. There is no default, so every renderer states it — one
    /// that silently inherited a bound it does not have would cost the planner
    /// work it need not do.
    ///
    /// [`Unbounded`]: crate::region::Unbounded
    type Policy: crate::region::FramePolicy;

    fn size(&self) -> Size;

    /// rsact is about to paint `region` (absolute screen coordinates).
    ///
    /// Override it if the renderer paints through something smaller than the
    /// frame — a tile framebuffer sets its origin here so absolute coordinates
    /// land correctly, a GPU sets a scissor rect. A renderer already covering
    /// the whole frame needs no transform, so the default is to do nothing.
    fn begin_region(&mut self, region: Rect) -> RenderResult {
        let _ = region;
        Ok(())
    }

    /// Finish the region opened by [`begin_region`]. A framebuffer backend has
    /// nothing to do (the caller takes the buffer back through its own inherent
    /// API — the surface never enters rsact); a GPU may end a render pass.
    ///
    /// [`begin_region`]: Renderer::begin_region
    fn end_region(&mut self) -> RenderResult {
        Ok(())
    }

    /// Restrict subsequent drawing to `area` until the matching [`pop_clip`].
    ///
    /// A stack rather than a `clipped(area, impl FnOnce(&mut Self))` closure:
    /// a tiled renderer re-establishes its clips once per pass, which closure
    /// nesting fights, and a closure taking `Self` by value would make this the
    /// one method keeping the trait dyn-incompatible.
    ///
    /// Calls must be balanced, but an unmatched [`pop_clip`] degrades rather
    /// than panicking.
    ///
    /// [`pop_clip`]: Renderer::pop_clip
    fn push_clip(&mut self, area: Rect);

    /// Undo the innermost [`push_clip`]. No-op if the stack is empty.
    ///
    /// [`push_clip`]: Renderer::push_clip
    fn pop_clip(&mut self);

    /// The absolute rect drawing is currently confined to, or `None` for "not
    /// confined / not reported".
    ///
    /// Callers use it to skip drawing that cannot land, so an implementation
    /// must **never report narrower** than what it really clips to. Reporting
    /// wider, or `None`, only costs redundant paint.
    ///
    /// The default `None` disables that skipping, which is the safe direction —
    /// and the right answer for a sink like [`NullRenderer`], whose `size()` is
    /// zero and would otherwise read as "clips everything away".
    ///
    /// Report the region being painted, not the surface behind it.
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

/// Minimal color type for use in NullRenderer.
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

/// A renderer that draws nothing, generic over the color it accepts.
///
/// The stub headless tests and size probes build a `Wtf` around — hence
/// `C = NullColor` by default — and what a collect pass runs widget bodies
/// against: such a pass must execute each body so dependencies re-track and
/// damage is pushed, but must not rasterize, and it has to satisfy
/// `Renderer<Color = W::Color>` for the *application's* color.
///
/// Stateless, so `NullRenderer::<C>::default()` is free.
pub struct NullRenderer<C = NullColor> {
    _color: PhantomData<C>,
}

// Hand-written rather than derived: `#[derive(Default)]` would demand
// `C: Default`, which no color needs to satisfy for an empty struct.
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

/// A [`Renderer`] that draws with a [`Rasterizer`] into a [`Blitter`].
///
/// Pick the pair for your hardware: `EgRasterizer` for embedded-graphics'
/// algorithms, `TinySkiaRasterizer` for anti-aliased output;
/// [`FramebufBlitter`](crate::blitter::FramebufBlitter) over your own buffer,
/// `PixmapBlitter` over a tiny-skia `Pixmap`. `P` is the
/// [`FramePolicy`](crate::region::FramePolicy) — how large a region you will
/// ask it to paint.
///
/// # Build it once; lend it a target per frame
///
/// A renderer holds state worth keeping — the clip stack, and a rasterizer's
/// caches — so build it at startup and keep it. The paint target is what comes
/// and goes: [`attach`] takes a blitter, [`detach`] gives it back, and only the
/// attached renderer has drawing methods, so nothing can paint into a buffer you
/// are holding.
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
    /// The lent target: `T` when [`Attached`], `()` when [`Detached`] — not an
    /// absent blitter but no field at all, so there is nothing to unwrap.
    blitter: A::Slot,
    /// The clip stack, seeded with the surface rect. Three load-bearing
    /// properties:
    ///
    /// - **push stores `area ∩ top`**, so the top *is* the effective clip —
    ///   which makes reading it for culling exact, and stops a widget clip
    ///   nested inside a region clip from letting drawing escape its tile;
    /// - **pop never pops the root**, so an unbalanced pop degrades rather than
    ///   leaving the renderer unclipped;
    /// - **a region is the root**, so no clip can escape it.
    ///
    /// It survives a detach, being the renderer's and not the target's.
    clips: alloc::vec::Vec<Rect>,
    /// The display's size — what rsact lays out and culls against, unchanged by
    /// how small the blitter's storage is.
    viewport: Size,
    /// Marker only: `fn() -> P` rather than `P` so the policy contributes no
    /// dropck obligation and no auto-trait leakage.
    policy: PhantomData<fn() -> P>,
}

impl<R, T, P> RasterRenderer<R, T, P, Detached> {
    /// A renderer with no target yet — what an application builds at boot, and
    /// what an app whose tiles arrive from a channel waits in.
    ///
    /// Unbounded, so a renderer can be built before its blitter type is known to
    /// satisfy [`Blitter`](crate::blitter::Blitter).
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

/// Why a target could not be lent — with both halves handed back, so a caller
/// that recovers loses nothing.
///
/// Only reachable for a target whose *type* could not state its capacity: a
/// `&'static mut [u16; N]` is proved at compile time and never arrives here.
pub struct AttachError<R, T, P> {
    /// The renderer, still parked and still holding its clip stack.
    pub renderer: RasterRenderer<R, T, P, Detached>,
    /// The target, untouched.
    pub blitter: T,
    /// Units the frame policy's largest region needs.
    pub needed: usize,
    /// Units the target holds.
    pub available: usize,
}

// Hand-written: `R` and `T` need not be `Debug` for the numbers to be printable,
// and the numbers are the whole message.
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
    /// Lend the renderer a target. Consumes the parked renderer and returns an
    /// [`Attached`] one — the only state that can draw.
    ///
    /// One implementation for every blitter, because this is the one place a
    /// policy and a target meet. The check runs **bottom-up**: the blitter
    /// states what it can hold, the policy states what will be asked of it.
    ///
    /// | the target | when it is checked |
    /// |---|---|
    /// | `&'static mut [u16; 5760]` | **compile time** — `UNITS` is `Some` |
    /// | `&'static mut [u16]`, a `Pixmap` | here, as an `Err` |
    /// | a direct-to-panel blitter | never — `capacity()` is `None`, meaning *unbounded*: there is no storage to overflow |
    ///
    /// A **packing** disagreement is always a compile error, for every blitter,
    /// since both `P::PIXELS_PER_UNIT` and `T::PIXELS_PER_UNIT` are consts and
    /// comparing a budget against a capacity is meaningless unless they count
    /// the same thing:
    ///
    /// ```compile_fail
    /// # use rsact_render::{blitter::FramebufBlitter, geometry::Size,
    /// #                    eg::rasterizer::EgRasterizer, region::Tiles,
    /// #                    renderer::RasterRenderer};
    /// # use embedded_graphics::pixelcolor::BinaryColor;
    /// // 1-bpp storage under a policy counting one pixel per unit. The buffer
    /// // is far from too small — the refusal is about the packing.
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
    /// [`AttachError`] when the target's *value* is too small, with both halves
    /// handed back. Never panics: whether a memory-plan mistake should abort is
    /// the application's call, made by an `unwrap` at the call site.
    pub fn attach(
        self,
        blitter: T,
    ) -> Result<RasterRenderer<R, T, P, Attached>, AttachError<R, T, P>> {
        // ── static half ───────────────────────────────────────────────────
        //
        // Fires once per instantiation, at monomorphization. Only codegen
        // evaluates a `const` block, so this is a build failure and not an
        // editor diagnostic — `cargo check` and rust-analyzer will not show it.
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

        // ── dynamic half ──────────────────────────────────────────────────
        //
        // Only for a target whose TYPE could not answer. `capacity() == None`
        // means *unbounded* — a direct-to-panel blitter stores nothing, so no
        // region can fail to fit — and is a different `None` from `UNITS`, which
        // means "ask the value".
        if T::UNITS.is_none()
            && let (Some(available), Some(needed)) =
                (blitter.capacity(), crate::region::policy_units::<P>())
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
    /// [`with_blitter`](Self::with_blitter), for an application whose targets
    /// arrive from a channel.
    ///
    /// Spelled on the **attached** type because that is the type an application
    /// names, so a caller reaches the parked state without writing `Detached`
    /// into a turbofish:
    ///
    /// ```ignore
    /// let renderer = Screen::parked(EgRasterizer, viewport);
    /// let renderer = renderer.attach(FramebufBlitter::new(pool.recv()))?;
    /// ```
    pub fn parked(
        rasterizer: R,
        viewport: Size,
    ) -> RasterRenderer<R, T, P, Detached> {
        RasterRenderer::new(rasterizer, viewport)
    }

    /// Build and attach in one step — sugar over
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

    /// Take the target back. Consumes the renderer and returns it [`Detached`],
    /// which has no blitter field at all, so nothing can paint into a target the
    /// caller is holding.
    ///
    /// **The blitter is the loan token**: it owns what was lent, so ownership
    /// moves out with it — a borrow the core could still write through would be
    /// UB once DMA owns the buffer. Unwrap it with
    /// `FramebufBlitter::into_storage` or `PixmapBlitter::into_pixmap`, or read
    /// it in place and re-attach it whole.
    ///
    /// Where it was painted comes back as
    /// [`Blitter::bounds`](crate::blitter::Blitter::bounds).
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

    /// The effective clip — the top of the stack, which `push_clip` keeps
    /// intersected with its parent.
    fn clip(&self) -> Rect {
        // The root is never popped, so the stack is never empty. `unwrap_or`
        // rather than `expect`: nothing on the render path may panic, and a
        // zero rect degrades to drawing nothing.
        self.clips.last().copied().unwrap_or(Rect::zero())
    }

    /// Split into the rasterizer and a clip-gated view of the blitter. The two
    /// fields are disjoint, so one `&mut self` yields both.
    ///
    /// **Hot path**: `Renderer::pixel` arrives here once per glyph pixel, so a
    /// text-heavy frame is O(10⁴) calls, each paying the `Rect::intersection`
    /// inside `RasterCtx::new`. It is redundant here — `begin_region` seeds the
    /// stack with the region and `push_clip` intersects, so `clip ⊆ bounds()`
    /// already holds — but removing it would need a second `RasterCtx`
    /// constructor, since that intersection is what guarantees the clip for
    /// every other caller.
    fn split(&mut self) -> (&mut R, crate::raster::RasterCtx<'_, T>) {
        let clip = self.clip();
        (
            &mut self.rasterizer,
            crate::raster::RasterCtx::new(&mut self.blitter, clip),
        )
    }

    /// Whether a primitive bounded by `bounds` can be skipped entirely.
    ///
    /// `bounds` is grown by the stroke width first, because
    /// [`StrokeAlignment::Outside`](crate::style::StrokeAlignment::Outside)
    /// paints *outside* the geometry — culling on the bare rect would clip a
    /// border away at a tile edge. Over-approximating costs redundant paint;
    /// under-approximating drops it, so this only ever errs outward.
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

/// Every geometry method is the same three lines — cull, split, forward. The
/// macro expands only the impl, so nothing is hidden from a reader of the
/// public API.
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
    /// The order matters and is the reason `begin_region` is a blitter method
    /// too: the blitter retargets *and primes* atomically, and the priming fill
    /// must not go through the clipped path — the region clip does not exist
    /// until the line after.
    fn begin_region(&mut self, region: Rect) -> RenderResult {
        self.blitter.begin_region(region)?;
        self.clips.clear();
        self.clips.push(region);
        Ok(())
    }

    /// Kept, unlike on `Blitter`: this is where a batching rasterizer would
    /// flush and where a GPU would end its pass. Nothing does either yet.
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

    /// `Option`, not `Rect`, and it is always `Some` here.
    ///
    /// There is no expressible "unbounded": `Rect::intersection` uses
    /// non-saturating `+` and `u32::MAX as i32 == -1`, so a `Rect::MAX` sentinel
    /// intersects to `Rect::zero()` — "unbounded" would read as "clips
    /// everything".
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
        // Culled where the bound is exact and cheap. `line`, `polygon` and
        // `path` are not: computing their bound costs about what testing it
        // saves, and `RasterCtx` clips them anyway — `scan::polygon` even bounds
        // its scan by the clip.
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

    /// The concrete stack under test, named once so no construction has to
    /// annotate a color the buffer type cannot imply (several colors share
    /// `Storage = u32`).
    type Fb = FramebufBlitter<Rgb888, &'static mut [u32]>;
    type Full = RasterRenderer<EgRasterizer, Fb, Unbounded>;
    type TiledBy<const W: u32, const H: u32> =
        RasterRenderer<EgRasterizer, Fb, Tiles<W, H>>;

    /// The two steps the API takes, in one line: a long-lived renderer, and a
    /// blitter over the caller's storage lent to it.
    fn build<P: FramePolicy>(
        viewport: Size,
        storage: &'static mut [u32],
    ) -> RasterRenderer<EgRasterizer, Fb, P> {
        RasterRenderer::<EgRasterizer, Fb, P>::parked(EgRasterizer, viewport)
            .attach(FramebufBlitter::new(storage))
            .unwrap()
    }

    /// Build and aim at the whole frame. A blitter starts aimed at **nothing**,
    /// so a test that paints without planning regions must aim it or it writes
    /// into a zero-sized target and asserts nothing.
    fn build_full(
        viewport: Size,
        storage: &'static mut [u32],
    ) -> RasterRenderer<EgRasterizer, Fb, Unbounded> {
        let mut r = build::<Unbounded>(viewport, storage);
        r.begin_region(Rect::new(Point::zero(), viewport))
            .expect("a full-frame buffer holds the full frame");
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

    /// A `&'static mut` loan, which is the shape both `FramebufStorage` impls
    /// describe and the only one a renderer reachable through `WidgetCtx`
    /// (`: 'static`) can hold. `Vec::leak` in a test is a `StaticCell` on a
    /// device.
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

    /// Content chosen to reach every path that differs between the two
    /// renderers: `fill_solid` (the whole-word framebuffer fill), a stroked
    /// `rect` (embedded-graphics' `draw_styled` through a different receiver), a
    /// diagonal `line`, and bare `pixel` writes (the text path).
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
    /// full one — the retarget-and-prime in `Blitter::begin_region`, with the
    /// renderer resetting its clip stack around it.
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

    /// The fast `fill_solid` (whole-word writes in the framebuffer) must land
    /// the same pixels as the per-pixel path.
    ///
    /// Its route is long — `Renderer::fill_solid` → `Rasterizer::fill` →
    /// `RasterCtx::rect` → `Blitter::fill_rect` → `Framebuf::fill_solid` — and
    /// every hop can clip, shift or split the rect differently.
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

    /// The renderer **borrows** its surface: it never allocates one and always
    /// gives it back.
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

        // Nothing to assert about painting while parked: that state has no
        // drawing methods, so there is nothing to call.
        let _ =
            parked.attach(FramebufBlitter::new(surface::<Rgb888>(viewport)));
    }

    /// After `begin_region`, the rect a buffer covers **is** the region it was
    /// asked to paint — for every surface, not only for tiles, which is what
    /// lets a caller carry one rectangle instead of two.
    ///
    /// Exempting full-frame surfaces from the retarget breaks this asymmetry
    /// invisibly, since a test using one surface kind at a time cannot see it.
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

    /// A region's units are laid out at **its own width**, so a region narrower
    /// than the frame is contiguous rows of `region.width`.
    ///
    /// Asserted directly because a differential test cannot reach it: chunking
    /// preserves width, so tiling a full frame yields full-width bands where
    /// "region width" and "frame width" are the same number, and a caller's
    /// helper using the wrong one is wrong the same way on both sides of any
    /// comparison — the difference cancels.
    ///
    /// The region is deliberately narrow AND tall, so a frame-width stride runs
    /// off the end of the region's data instead of merely landing askew.
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

    /// **`begin_region` is the only thing that ever aims a target.** A
    /// reattached one covers nothing until a region is begun, and exactly that
    /// region after.
    ///
    /// Aiming at construction instead makes "aimed wrongly" a reachable state:
    /// a full-frame surface exempted from the retarget is never re-aimed, so the
    /// flush loop reports a zero dirty rect from the second frame on and sends
    /// nothing — and un-exempting it requires the blitter to know a viewport,
    /// which is not its business.
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

    /// A renderer **declares** the largest region it will accept, and its
    /// surface is checked against that declaration — not the other way round.
    ///
    /// Spelling an unstatable capacity as `usize::MAX` rather than `None` reads
    /// as "my surface always covers the frame" and exempts every heap surface:
    /// an *empty* boxed slice would satisfy a full-frame policy at compile time.
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

    /// A nested clip must be **narrowed by** its parent, not replace it.
    ///
    /// Asserted on the framebuffer rather than on the stack, because that is
    /// where it matters: a pixel inside the inner clip but outside the outer one
    /// must not land. Storing the raw area instead lets an inner clip reaching
    /// beyond its parent *widen* the effective clip — under tiling, that is
    /// drawing escaping its tile.
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

        // The probe color must differ from the untouched framebuffer, or the
        // assertions below hold whatever the clip does — `default_background()`
        // is WHITE for RGB, so a white probe would pass vacuously.
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

    /// A target too small for the declared policy is refused when it is lent,
    /// before anything paints into it — **as an `Err`, not a panic**. Whether a
    /// memory-plan mistake should abort is the application's call, and both
    /// halves come back so a caller that recovers loses nothing.
    ///
    /// This is the *runtime* half, reachable only because the storage is a
    /// runtime-length slice. The same mistake with a `&mut [u32; 240]` does not
    /// compile.
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
