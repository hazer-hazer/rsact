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
/// The one place this arithmetic is written.
/// [`assert_policy_fits`](crate::region::assert_policy_fits) compares a
/// surface against it for WS6.4.0(iii)'s capacity proof, and
/// [`units_for`](crate::framebuf::units_for) is the color-typed wrapper
/// the embedded-graphics backend uses — they must not be allowed to drift,
/// because a capacity check that disagrees with the buffer's real layout is
/// worse than no check at all.
///
/// Padding per row is what makes sub-byte packing correct: a 122-pixel 1-bpp
/// row occupies 16 bytes, not 15.25. Area-based arithmetic gets this wrong, and
/// is exactly the bug roadmap 6.5 has to undo in `Framebuf::new`.
///
/// ```
/// # use rsact_render::renderer::region_units;
/// assert_eq!(region_units(240, 24, 1), 5760); // RGB565: one unit per pixel
/// assert_eq!(region_units(122, 24, 8), 384);  // 1-bpp: 16 bytes per row
/// ```
pub const fn region_units(w: u32, h: u32, pixels_per_unit: usize) -> usize {
    // A renderer that reports zero would divide by zero; treat it as unpacked,
    // which over-estimates the requirement and so fails safe.
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

/// Whether a renderer is holding its surface — a **type-state**, not a flag.
///
/// The same shape as `UI<W, HasPages>`: a marker parameter that decides which
/// methods exist. The twist is [`Slot`](Attachment::Slot), and it is what makes
/// this worth doing rather than decorative — a plain marker could only *guard*
/// an `Option<B>` field, leaving the `unwrap` inside. An associated type lets
/// the field itself change shape: the surface when attached, `()` when not. So
/// there is no `Option`, no `unwrap`, and no "drawing while detached" branch on
/// any hot path — that state is simply not a value a drawing method can be
/// called on.
///
/// The invariant it removes was real. `EGRenderer` used to log a warning and
/// discard the frame when something painted between a `detach` and the next
/// `attach`; that is a scheduling mistake the caller could make silently, once
/// per frame, forever. Now it does not compile.
pub trait Attachment<S> {
    /// The surface field's type in this state: `S` attached, `()` detached.
    type Slot;
}

/// The renderer is holding a surface and can draw.
///
/// Every drawing impl — [`Renderer`], `DrawTarget` — is written for this state
/// and no other, so the guarantee is structural:
///
/// ```
/// # use rsact_render::{eg::renderer::EGRenderer, geometry::Size,
/// #                    renderer::Renderer};
/// # use embedded_graphics::pixelcolor::Rgb888;
/// let buf: &'static mut [u32] = vec![0; 16 * 16].leak();
/// let r = EGRenderer::<Rgb888, _>::new(Size::new_equal(16), buf);
/// // Attached: drawing is available.
/// let _ = r.size();
/// ```
///
/// and painting after a `detach` is not a logged no-op but a compile error:
///
/// ```compile_fail
/// # use rsact_render::{eg::renderer::EGRenderer, geometry::Size,
/// #                    renderer::Renderer};
/// # use embedded_graphics::pixelcolor::Rgb888;
/// let buf: &'static mut [u32] = vec![0; 16 * 16].leak();
/// let r = EGRenderer::<Rgb888, _>::new(Size::new_equal(16), buf);
/// let (parked, _buf, _at) = r.detach();
/// // The application is holding the buffer — there is nothing to draw into.
/// let _ = parked.size();
/// ```
pub struct Attached;

impl<S> Attachment<S> for Attached {
    type Slot = S;
}

/// The owner is holding the surface; the renderer keeps only its configuration.
///
/// A real, useful state rather than an error case: it is where an app's buffer
/// lives while it is being shipped over SPI, encoded to a PNG, or waited on.
pub struct Detached;

impl<S> Attachment<S> for Detached {
    type Slot = ();
}

// NOTE (layer split, PR A): `trait AntiAliasing` with the `AntiAliasingEnabled`
// / `AntiAliasingDisabled` witnesses lived here. It was a **type-level** switch:
// `EGRenderer<C, AA, ..>` had two `Renderer` impls, and the witness is what kept
// their call graphs from resolving into each other (`Renderer::line` and the
// primitives' mutual `draw_aa` calls both dispatch on it).
//
// Both impls and every `draw_aa` are deleted with it — maintainer decision D1:
// `EgRasterizer` is embedded-graphics **as-is**, and anti-aliasing belongs to
// rsact's own `RsactRasterizer`, which will produce coverage and spans instead
// of blending `f32` per pixel through the renderer.
//
// The *runtime* option this shadowed is a different question and still open —
// see the commented-out `RendererOptions` block at the top of this file. That
// sketch mentions an `AntiAliasing` enum; it means a value, not this type.

#[derive(Clone, Copy, Debug)]
pub enum ViewportKind {
    Fullscreen,
    /// Clipped part of the parent viewport with absolute positions relative to
    /// the screen top-left point
    Clipped(Rect),
    /// Part of the parent viewport with positions relative to this viewport's
    /// top-left point
    Cropped(Rect),
}

impl ViewportKind {
    pub fn root() -> Self {
        Self::Fullscreen
    }

    /// The absolute rect this viewport confines drawing to, or `None` for
    /// [`Self::Fullscreen`] (confined only by the surface itself).
    ///
    /// WS6.4b reads this as the **cull rect**: a widget whose bounds miss it
    /// cannot affect the output, so it need not be drawn at all. That is only
    /// sound if the stack composes — see [`Self::nested_in`].
    ///
    /// The rect is in **the caller's coordinate space**, i.e. absolute, for every
    /// variant — including [`Self::Cropped`], whose stored rect is absolute and
    /// whose *rebasing* is the renderer's private business (WS6.4.0(ii-3): rsact
    /// paints in absolute coordinates and a region-backed renderer offsets
    /// internally). That is what lets both consumers compare against it directly:
    /// `render_part`'s cull, which holds an absolute `layout.outer`, and
    /// `DrawTargetProxy`'s per-pixel filter, which sees the coordinates the
    /// drawing code emitted. A variant reporting a viewport-local rect here would
    /// silently invert both tests the moment WS6.4d starts constructing `Cropped`.
    pub fn clip_bounds(&self) -> Option<Rect> {
        match *self {
            ViewportKind::Fullscreen => None,
            ViewportKind::Clipped(area) | ViewportKind::Cropped(area) => {
                Some(area)
            },
        }
    }

    /// This viewport as it must be recorded *inside* `parent` — narrowed by it.
    ///
    /// **WS6.4b bug fix.** Nested clips did not compose: `push_clip` stored the
    /// raw area and the write filter consulted only the top of the stack, so a
    /// clip *wider* than its parent widened the effective clip. Unreachable today
    /// (nothing sets `ElState::clip_path`, so a frame's own push is the only one)
    /// and live the moment either WS6.4d pushes a region clip with a widget clip
    /// inside it — where drawing would escape the tile — or `Scrollable`'s
    /// commented-out clip is implemented. Intersecting on push also makes the top
    /// of the stack *be* the effective clip, which is what makes reading it for
    /// culling exact rather than approximate.
    ///
    /// `Cropped` passes through on either side: it re-bases coordinates, so
    /// narrowing it is not a rect intersection. Nothing constructs it today, and
    /// the coordinate-space rule is WS6.4d's to define when tile origins arrive.
    pub fn nested_in(self, parent: ViewportKind) -> Self {
        match (self, parent) {
            (ViewportKind::Clipped(area), ViewportKind::Clipped(parent)) => {
                ViewportKind::Clipped(area.intersection(&parent))
            },
            _ => self,
        }
    }
}

/// Core renderer trait: defines primitive drawing methods independent of
/// embedded_graphics.
pub trait Renderer {
    type Color: Color;

    /// The largest region this renderer will accept, as a **type**.
    ///
    /// This is the whole of what rsact knows about a renderer's storage, and it
    /// is deliberately not a fact about storage at all: it says *how big a
    /// rectangle you may ask me to paint*, which a GPU streaming commands and a
    /// renderer holding an 11 KiB tile can both answer. rsact never sees a
    /// surface — no `FramebufStorage` trait, no capacity number, no buffer type
    /// parameter reaches this trait — because a renderer is free to have no
    /// surface at all.
    ///
    /// [`Unbounded`] is the answer for every renderer that never needed tiling,
    /// and there is no default because associated *type* defaults are still
    /// unstable. Having to write it out is a feature: a renderer that silently
    /// inherited a bound it does not have would mislead the planner in the
    /// expensive direction.
    ///
    /// **Where the surface is checked against this: not here.** A backend owns
    /// both facts — the policy it declares and the buffer it was handed — so it
    /// makes the comparison itself via
    /// [`assert_policy_fits`](crate::region::assert_policy_fits): in a `const`
    /// block when its surface is a fixed-size array, at `attach` when it is a
    /// runtime-length slice. Hoisting the check up here would force every
    /// renderer to describe a surface just so the ones that have one could be
    /// checked.
    ///
    /// [`Unbounded`]: crate::region::Unbounded
    type Policy: crate::region::FramePolicy;

    // NOTE (WS6.4.0(ii-2)): `type Options` + `fn set_options` lived here and
    // were removed as dead — all seven implementors were `type Options = ()`
    // with an empty body, and nothing ever called the setter. The associated
    // type also had to be named in any `dyn Renderer<Color = _, Options = _>`,
    // so it cost something despite carrying nothing.
    //
    // The design it was a placeholder for is NOT dropped: see the commented-out
    // `RendererOptions` / `AntiAliasing` block at the top of this file, which is
    // still the sketch for runtime renderer options. Reintroduce the hook there
    // when something actually configures a renderer at runtime — by then the
    // shape will be known, instead of an empty slot guessing at it.

    fn size(&self) -> Size;

    /// rsact is about to paint `region` (absolute screen coordinates).
    ///
    /// WS6.4.0(ii-3). What a backend does with it is its own business: a tile
    /// framebuffer sets its origin offset so absolute coordinates land in a
    /// surface smaller than the frame; a GPU sets a scissor rect; a renderer
    /// whose surface already covers the whole frame ignores it.
    ///
    /// **The default is _correct_, not merely permissive** — a full-frame
    /// surface receives absolute coordinates and needs no transform at all, so
    /// [`NullRenderer`], [`RecordingRenderer`](crate::record::RecordingRenderer)
    /// and a full-frame `EGRenderer` are already right with no code.
    ///
    /// Deliberately part of `Renderer` rather than a separate `TileAware` trait:
    /// it states _where_ you are drawing, the same category as [`size`] and
    /// [`push_clip`], so no `where` clause leaks into the render entry point.
    /// Equally deliberately it says nothing about *tiles* — the number and shape
    /// of regions is the frame policy's business (roadmap 6.4d), and this trait
    /// stays "how to draw a primitive".
    ///
    /// No caller yet: the multi-region driver is 6.4d. Landed with the rest of
    /// the trait shape so 6.4d adds a strategy rather than reopening the trait.
    ///
    /// [`size`]: Renderer::size
    /// [`push_clip`]: Renderer::push_clip
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
    /// WS6.4.0(ii-1): a **stack** rather than the previous
    /// `clipped(area, impl FnOnce(&mut Self))` closure form. Every backend
    /// already kept a stack internally and merely wrapped it in a closure, so
    /// this exposes what was already there. Two reasons to prefer it: a
    /// multi-pass (tiled) renderer re-establishes clips once per pass, which
    /// closure nesting fights; and the closure form takes `Self` by value in a
    /// generic parameter, so it is the one method keeping this trait
    /// dyn-incompatible. Closure sugar survives where it reads better — see
    /// `RenderCtx::clip_inner` in rsact-ui, which pairs push/pop for its caller.
    ///
    /// Calls must be balanced. An unmatched [`pop_clip`] is a no-op, never a
    /// panic (WS1.8: the UI logs and degrades, it does not abort).
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
    /// WS6.4b: this is the **cull rect**, and the contract runs one way —
    /// *anything whose bounds miss it cannot affect the output*, so a caller may
    /// skip drawing it. It must therefore never report *narrower* than what the
    /// renderer actually clips to; reporting wider (or `None`) only costs
    /// redundant paint. Same asymmetry as
    /// [`DrawOp::bounds`](crate::record::DrawOp::bounds), and deliberately the
    /// same predicate: WS6.4a's tile-invariance check is written against it.
    ///
    /// The default is `None`, which disables culling for a backend that does not
    /// report — the safe direction, and correct for a no-op sink like
    /// [`NullRenderer`], whose `size()` is zero and would otherwise read as
    /// "clips everything away".
    ///
    /// Note what this is *not*: a way to ask "what is my surface". A tile-backed
    /// renderer under WS6.4d reports its **region**, not its buffer — the whole
    /// point being that the region is what bounds the frame's useful work.
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
/// Two jobs. It is the stub every headless test and size/metrics probe builds a
/// `Wtf` around — hence `C = NullColor` by default, so `Wtf<NullRenderer, ..>`
/// and `&mut NullRenderer` keep working unannotated. And (WS6.4.0(ii-4)) it is
/// what 6.4c's **collect pass** runs widget bodies against: that pass must
/// genuinely execute each body so reactive dependencies re-track and damage
/// rects are pushed, but must not rasterise, and it has to satisfy
/// `Renderer<Color = W::Color>` for the *application's* color — which the
/// previous `type Color = NullColor` hard-wiring could not express.
///
/// It carries no state, so `NullRenderer::<C>::default()` is free.
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

// ===========================================================================
// The layered renderer — L1 over a `Rasterizer` and a `Blitter`
// ===========================================================================

/// The [`Renderer`] built around a [`Rasterizer`], with the [`Blitter`] as the
/// interchangeable sink.
///
/// This is what `EGRenderer` and `TinySkiaRenderer` both become. L1's whole job
/// is here and it is small: hold the clip stack, reset it per region, cull, and
/// forward to L2 with a [`RasterCtx`] built from the current clip.
///
/// **No `where` clause on the struct.** With the bounds here, the detached type
/// `RasterRenderer<R, FramebufBlitter<C, B, Detached>, P>` is ill-formed
/// (`E0277`, since a detached blitter is not a `Blitter`), and adding the bound
/// to fix it would restore exactly the drawing-while-detached case the
/// type-state exists to delete. The bounds live on the impls that draw.
///
/// [`Rasterizer`]: crate::raster::Rasterizer
/// [`Blitter`]: crate::blitter::Blitter
/// [`RasterCtx`]: crate::raster::RasterCtx
pub struct RasterRenderer<R, T, P = crate::region::Unbounded> {
    rasterizer: R,
    blitter: T,
    /// The clip stack. A plain `Vec<Rect>`, seeded with the surface rect.
    ///
    /// Three properties, and each is load-bearing rather than tidy:
    ///
    /// - **push stores `area ∩ top`**, so the top IS the effective clip — which
    ///   is what makes reading it for culling exact rather than approximate, and
    ///   what stops a widget clip inside a region clip letting drawing escape
    ///   its tile;
    /// - **pop never pops the root**, so an unbalanced pop degrades rather than
    ///   leaving the renderer with no clip at all (WS1.8: the UI logs and
    ///   degrades, it does not abort);
    /// - **a region is the root**, so no clip can escape it.
    clips: alloc::vec::Vec<Rect>,
    /// The display's size — what rsact lays out and culls against, unchanged by
    /// how small the blitter's storage is.
    viewport: Size,
    /// Marker only: `fn() -> P` rather than `P` so the policy contributes no
    /// dropck obligation and no auto-trait leakage.
    policy: PhantomData<fn() -> P>,
}

impl<R, T, P> RasterRenderer<R, T, P>
where
    R: crate::raster::Rasterizer<T>,
    T: crate::blitter::Blitter,
    P: crate::region::FramePolicy,
{
    /// The **runtime** half of the capacity proof, and it is genuinely generic:
    /// `policy_units::<P>()` needs only `P`, because `PIXELS_PER_UNIT` lives on
    /// the policy. A blitter with no storage bound reports `None` and there is
    /// nothing to check.
    ///
    /// # Panics
    ///
    /// If the blitter cannot hold policy `P`'s largest region.
    pub fn new(rasterizer: R, blitter: T, viewport: Size) -> Self {
        if let Some(units) = blitter.capacity() {
            crate::region::assert_policy_fits::<P>(units);
        }
        Self {
            rasterizer,
            blitter,
            clips: alloc::vec![Rect::new(Point::zero(), viewport)],
            viewport,
            policy: PhantomData,
        }
    }

    /// The effective clip — the top of the stack, which `push_clip` keeps
    /// intersected with its parent.
    fn clip(&self) -> Rect {
        // The root is never popped, so the stack is never empty; `unwrap_or`
        // rather than `expect` because a panic on the render path is exactly
        // what WS1.8 forbids, and a zero rect degrades to drawing nothing.
        self.clips.last().copied().unwrap_or(Rect::zero())
    }

    /// Split into the rasterizer and a clip-gated view of the blitter.
    ///
    /// `&mut self.rasterizer` and `&mut self.blitter` are disjoint fields, so
    /// one `&mut self` yields both; elision gives both borrows the same lifetime
    /// and the tuple return is accepted.
    ///
    /// **Hot path.** `Renderer::pixel` comes through here once *per glyph pixel*
    /// — `DrawTargetProxy::draw_iter` is the only route text takes, and a
    /// text-heavy frame is O(10⁴) calls. Against the pre-split renderer that
    /// adds one `Rect::intersection`, inside `RasterCtx::new`. If it ever shows
    /// up in a profile, note that the intersection is *provably redundant here*:
    /// `begin_region` seeds the stack with the region and `push_clip`
    /// intersects, so `clip ⊆ bounds()` already holds for this renderer. It
    /// could become a `debug_assert!` plus a plain assignment — but only behind
    /// a second constructor, because the intersection is what makes the
    /// guarantee structural for any *other* L1 that builds a `RasterCtx`.
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

    /// The rasterizer, for a caller that needs to configure it.
    pub fn rasterizer(&mut self) -> &mut R {
        &mut self.rasterizer
    }
}

/// Every geometry method is the same three lines — cull, split, forward — and a
/// macro is where that belongs: one private expansion inside this crate, over
/// mechanically identical bodies. Unlike a macro over the *trait* definition it
/// hides nothing from a reader of the public API, which is the distinction that
/// makes it acceptable here and not there.
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

impl<R, T, P> Renderer for RasterRenderer<R, T, P>
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

/// **The compile-time half of the capacity proof lives on the framebuf path
/// only** — the one place a *static* capacity exists.
///
/// A blitter with no framebuffer (a direct-to-panel one, a GPU attachment) has
/// neither a `B::UNITS` nor a `C: PackedColor`, which is why this cannot sit on
/// the generic [`RasterRenderer::new`]. The runtime half does, and does run
/// there.
impl<R, C, B, P>
    RasterRenderer<
        R,
        crate::blitter::framebuf::FramebufBlitter<C, B, Attached>,
        P,
    >
where
    C: Color + crate::framebuf::PackedColor,
    B: crate::framebuf::FramebufStorage<C>,
    P: crate::region::FramePolicy,
{
    /// Two assertions, both unchanged in substance from the pre-split
    /// `EGRenderer::assert_static_capacity`, and fired once per instantiation at
    /// monomorphization:
    ///
    /// 1. `assert_policy_fits::<P>(units)` when `B::UNITS` is `Some` — skipped
    ///    when `None`, which means "ask the value" and is the runtime half's job.
    /// 2. `P::PIXELS_PER_UNIT == C::PPS`. The two are separate values today and
    ///    must be kept in step: a 1-bpp color under a `PIXELS_PER_UNIT = 1`
    ///    policy would demand eight times the storage it needs, and the reverse
    ///    would silently under-demand. The deferred byte/packing rework is what
    ///    eventually deletes this one, by making them one value.
    ///
    /// A `const` block is invisible to `cargo check` and rust-analyzer — only
    /// codegen evaluates it — so `region.rs`'s eager `const _: () = …` doctests
    /// remain the only check-time-visible form. That is unchanged behaviour.
    fn assert_static_capacity() {
        const {
            if P::MAX_REGION.is_some() {
                assert!(
                    P::PIXELS_PER_UNIT == C::PPS,
                    "this frame policy's pixel packing disagrees with the \
                     renderer's color — see the instantiation in this error"
                );
            }
            if let (Some(units), Some(needed)) = (
                <B as crate::framebuf::FramebufStorage<C>>::UNITS,
                crate::region::policy_units::<P>(),
            ) {
                assert!(
                    needed <= units,
                    "this surface is too small for the renderer's frame \
                     policy — see the instantiation in this error for the \
                     color, buffer type and policy"
                );
            }
        }
    }

    /// Build a renderer over a framebuffer the caller owns.
    ///
    /// **Nothing here allocates**, and nothing here chooses *where* the memory
    /// lives; that is the application's decision and not one rsact can make
    /// well. A buffer covering the whole frame gives classic full-framebuffer
    /// behaviour; a smaller one is retargeted per region.
    pub fn with_framebuf(rasterizer: R, viewport: Size, storage: B) -> Self {
        use crate::blitter::Blitter as _;
        Self::assert_static_capacity();
        let blitter = crate::blitter::framebuf::FramebufBlitter::<C, B, Detached>::parked(
            viewport,
        )
        .attach(storage);
        // **Both halves, and this line is not redundant.** The static half above
        // is skipped entirely when `B::UNITS` is `None` — i.e. for every
        // runtime-length buffer, which is every `&'static mut [T]` from a
        // `StaticCell` pool and every heap surface. Without this, a 240-unit
        // slice satisfied `Tiles<240, 24>` silently. That is the same shape of
        // hole as the `UNITS = usize::MAX` sentinel WS6.4d removed, and it was
        // reintroduced here for one commit until a test caught it.
        if let Some(units) = blitter.capacity() {
            crate::region::assert_policy_fits::<P>(units);
        }
        Self::from_parts(rasterizer, blitter, viewport)
    }

    /// Take the surface back, with the region that was painted into it.
    ///
    /// Consumes the renderer and returns it holding a [`Detached`] blitter — a
    /// state with no surface *field*, so nothing can paint into a buffer the
    /// caller is holding. The returned rect is both what to index the buffer at
    /// (rows are strided at its width) and what to send.
    pub fn detach(
        self,
    ) -> (
        RasterRenderer<
            R,
            crate::blitter::framebuf::FramebufBlitter<C, B, Detached>,
            P,
        >,
        B,
        Rect,
    ) {
        let (parked, buffer, at) = self.blitter.detach();
        (
            RasterRenderer {
                rasterizer: self.rasterizer,
                blitter: parked,
                clips: self.clips,
                viewport: self.viewport,
                policy: PhantomData,
            },
            buffer,
            at,
        )
    }

    /// The bounds-free half of construction, shared by `with_framebuf` and
    /// `attach` — both of which have already run the capacity proof, and neither
    /// of which can call [`RasterRenderer::new`] without also naming
    /// `R: Rasterizer<..>`, a bound a *constructor* has no reason to demand.
    fn from_parts(
        rasterizer: R,
        blitter: crate::blitter::framebuf::FramebufBlitter<C, B, Attached>,
        viewport: Size,
    ) -> Self {
        Self {
            rasterizer,
            blitter,
            clips: alloc::vec![Rect::new(Point::zero(), viewport)],
            viewport,
            policy: PhantomData,
        }
    }
}

impl<R, C, B, P>
    RasterRenderer<
        R,
        crate::blitter::framebuf::FramebufBlitter<C, B, Detached>,
        P,
    >
where
    C: Color + crate::framebuf::PackedColor,
    B: crate::framebuf::FramebufStorage<C>,
    P: crate::region::FramePolicy,
{
    /// Lend the renderer a surface. The one place `P` and a live buffer's
    /// capacity are both in scope, so it is where the runtime half of the proof
    /// runs — again, because a re-lent runtime-length buffer has a new extent.
    ///
    /// # Panics
    ///
    /// If the loan is smaller than `P` requires.
    pub fn attach(
        self,
        storage: B,
    ) -> RasterRenderer<
        R,
        crate::blitter::framebuf::FramebufBlitter<C, B, Attached>,
        P,
    > {
        use crate::blitter::Blitter as _;
        RasterRenderer::<
            R,
            crate::blitter::framebuf::FramebufBlitter<C, B, Attached>,
            P,
        >::assert_static_capacity();
        let blitter = self.blitter.attach(storage);
        if let Some(units) = blitter.capacity() {
            crate::region::assert_policy_fits::<P>(units);
        }
        RasterRenderer {
            rasterizer: self.rasterizer,
            blitter,
            clips: self.clips,
            viewport: self.viewport,
            policy: PhantomData,
        }
    }
}

#[cfg(all(test, feature = "embedded-graphics"))]
mod raster_renderer_tests {
    use super::*;
    use crate::{
        blitter::framebuf::FramebufBlitter, eg::renderer::EGRenderer,
        framebuf::PackedColor, raster::eg::EgRasterizer, region::Tiles,
        style::DrawStyle,
    };
    use embedded_graphics::pixelcolor::Rgb888;

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

    /// **The acceptance test for the whole refactor**, proved one PR before the
    /// substitution it licenses.
    ///
    /// `RasterRenderer<EgRasterizer, FramebufBlitter<..>>` must paint what
    /// `EGRenderer` paints — the same pixels, not merely the same draw calls.
    /// The op-log goldens cannot see this: they sit above L1 and record what the
    /// widget layer *asked* for, so an addressing or clipping mistake below them
    /// leaves them intact and produces a plausible image.
    #[test]
    fn the_layered_renderer_paints_what_eg_renderer_paints() {
        let viewport = Size::new(64, 64);

        let mut old =
            EGRenderer::<Rgb888, _>::new(viewport, surface::<Rgb888>(viewport));
        content(&mut old);

        let mut new =
            RasterRenderer::<_, _, crate::region::Unbounded>::with_framebuf(
                EgRasterizer,
                viewport,
                surface::<Rgb888>(viewport),
            );
        content(&mut new);

        let (_, old_units, old_at) = old.detach();
        let (_, new_units, new_at) = new.detach();
        assert_eq!(old_at, new_at, "the two renderers cover different rects");

        // Not vacuous: a substantial part of the frame must have been painted,
        // or two identically-blank buffers would pass.
        let bg = <Rgb888 as Color>::default_background().into_storage();
        let painted = old_units.iter().filter(|u| **u != bg).count();
        assert!(
            painted > (64 * 64) / 4,
            "the reference frame painted only {painted} of 4096 pixels"
        );

        let mismatches: alloc::vec::Vec<usize> = (0..old_units.len())
            .filter(|&i| old_units[i] != new_units[i])
            .collect();
        assert!(
            mismatches.is_empty(),
            "{} of {} pixels differ between EGRenderer and \
             RasterRenderer<EgRasterizer, FramebufBlitter>; first at ({}, {})",
            mismatches.len(),
            old_units.len(),
            mismatches[0] % 64,
            mismatches[0] / 64,
        );
    }

    /// The tiling equality, on the layered renderer: a surface a fraction of the
    /// frame's size paints the same pixels as a full one.
    ///
    /// Same claim as the pre-split `EGRenderer` test, and it has to be re-proved
    /// here because the retarget-and-prime that makes it true moved — it is now
    /// `Blitter::begin_region`, one layer down, with L1 resetting its clip stack
    /// around it.
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

        let mut full =
            RasterRenderer::<_, _, crate::region::Unbounded>::with_framebuf(
                EgRasterizer,
                viewport,
                surface::<Rgb888>(viewport),
            );
        content(&mut full);
        let mut full_map = blank();
        let (_, full_units, full_at) = full.detach();
        blit(&mut full_map, &full_units, full_at);

        const TILE_UNITS: usize = (W * BAND) as usize;
        let mut tiled = RasterRenderer::<_, _, Tiles<W, BAND>>::with_framebuf(
            EgRasterizer,
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
            let (parked, units, at) = tiled.detach();
            blit(&mut tiled_map, &units, at);
            tiled = parked.attach(spare);
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
        let mut r = RasterRenderer::<_, _, Tiles<64, 8>>::with_framebuf(
            EgRasterizer,
            viewport,
            surface_units::<Rgb888>(64 * 8),
        );
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

    /// A surface too small for the declared policy is refused at construction,
    /// before anything paints into it — the runtime half of the capacity proof,
    /// which the layered renderer runs generically rather than per backend.
    #[test]
    #[should_panic(expected = "too small for this frame policy")]
    fn a_surface_too_small_for_the_policy_is_refused() {
        let _ = RasterRenderer::<
            EgRasterizer,
            FramebufBlitter<Rgb888, _>,
            Tiles<240, 24>,
        >::with_framebuf(
            EgRasterizer,
            Size::new_equal(240),
            surface_units::<Rgb888>(240),
        );
    }
}
