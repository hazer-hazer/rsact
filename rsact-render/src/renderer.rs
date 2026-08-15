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
/// The invariant it removes was real. The pre-split renderer logged a warning and
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
/// # use rsact_render::{blitter::framebuf::FramebufBlitter, geometry::Size,
/// #                    raster::eg::EgRasterizer, region::Unbounded,
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
/// # use rsact_render::{blitter::framebuf::FramebufBlitter, geometry::Size,
/// #                    raster::eg::EgRasterizer, region::Unbounded,
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
/// let (parked, _blitter) = take(r);
/// // The application is holding the buffer — there is nothing to draw into.
/// let _ = Renderer::size(&parked);
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

// NOTE (layer split, PR C): `enum ViewportKind` lived here — `Fullscreen`,
// `Clipped(Rect)` and `Cropped(Rect)` — with `root()`, `clip_bounds()` and
// `nested_in()`. Every backend held a `Vec<ViewportKind>` as its clip stack.
//
// **`Cropped` is deleted (maintainer decision D4) and the other two collapse.**
// `Cropped` re-based coordinates rather than narrowing them; it had no live
// constructor and no planned one, because absolute positioning is a *bounds*
// extension (`paint_bounds`/`ext_draw`, WS6.4c(G)) rather than a coordinate
// space, and offscreen layers rebase at L3 where `local()` already lives. That
// left `Fullscreen` and `Clipped(Rect)` — and `Fullscreen` already behaved as
// `Clipped(surface_rect)` everywhere, because `clip_bounds` substituted the
// surface rect for it deliberately (WS6.4b needed culling to pay on an ordinary
// full-frame render, not only under tiles).
//
// So a clip stack is a plain `Vec<Rect>` seeded with the surface rect, and
// `nested_in` becomes one `Rect::intersection`. Three properties survive as
// documented, tested behaviour on every holder — see `RasterRenderer::clips`:
// push stores `area ∩ top`, pop never pops the root, and a region IS the root.
//
// No `ClipStack` type replaced it (D9): the invariant it would have shared is
// one line, and the two remaining holders — `RasterRenderer` and
// `RecordingRenderer`, the latter necessarily L1 because it logs *primitives* —
// do different things on mutation anyway.

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
    /// and a full-frame `RasterRenderer` are already right with no code.
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
/// # The renderer is long-lived; the TARGET is what comes and goes
///
/// `A` is the attachment type-state, and it is on **this** type rather than on
/// the blitter — which is the correction that matters most about this struct.
///
/// A renderer retains state a caller pays to build: the clip stack, and a
/// rasterizer's caches (`TinySkiaRasterizer` holds a coverage `Mask` and a
/// `PathStroker`; `RsactRasterizer` will hold a scanline). So it is created once
/// and lives for the application. What is *lent* is the caller's paint target —
/// a framebuffer, a pixmap, a `DrawTarget`, a GPU attachment — and a
/// [`Blitter`] is exactly the thing that wraps one. So [`attach`] takes a
/// blitter and [`detach`] gives it back, and a blitter is always in the one
/// state where it has its target.
///
/// **The first shape of this put the type-state on the blitter**
/// (`FramebufBlitter<C, B, Attached>`), which forced a *specialized* attach and
/// detach pair on `RasterRenderer` per blitter kind — four impl blocks for two
/// blitters, each re-stating the capacity proof, and one of them (the framebuf
/// constructor) was written without the runtime half, so a 240-unit slice
/// satisfied `Tiles<240, 24>` in silence. That duplication was structural: a
/// `DirectBlitter` and a DMA2D blitter would each have added another pair and
/// another chance to forget. It also could not express a target that is not
/// storage at all — a direct-to-`DrawTarget` blitter has no buffer to hand
/// back, only itself.
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
    /// The lent target — and **only** in the [`Attached`] state, where its type
    /// is `T`. In [`Detached`] it is `()`: not an absent blitter but no field at
    /// all, so there is nothing to unwrap and no "drawing without a target" case
    /// for any method to handle.
    blitter: A::Slot,
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
    ///
    /// It survives a detach, because it is the renderer's and not the target's.
    clips: alloc::vec::Vec<Rect>,
    /// The display's size — what rsact lays out and culls against, unchanged by
    /// how small the blitter's storage is.
    viewport: Size,
    /// Marker only: `fn() -> P` rather than `P` so the policy contributes no
    /// dropck obligation and no auto-trait leakage.
    policy: PhantomData<fn() -> P>,
}

impl<R, T, P> RasterRenderer<R, T, P, Detached> {
    /// A renderer with no target yet — the state a blitter is attached *to*, and
    /// the one an application builds at boot.
    ///
    /// Useful on its own rather than a formality: an app whose tiles arrive from
    /// a channel constructs this once and waits for the first one. There is no
    /// bound here at all, so a renderer can be built before its blitter type is
    /// even known to satisfy [`Blitter`](crate::blitter::Blitter).
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
    /// **One implementation, every blitter**, and the whole capacity proof lives
    /// here because this is the one place a policy and a target meet. The
    /// constraint is checked **bottom-up**: the blitter states what it can hold,
    /// the policy states what the application wants asked of it, and the two are
    /// compared — statically where the blitter's type can answer, at run time
    /// where only its value can, and not at all where there is nothing to
    /// overflow.
    ///
    /// | the target | when it is checked |
    /// |---|---|
    /// | `&'static mut [u16; 5760]` | **compile time** — `UNITS` is `Some` |
    /// | `&'static mut [u16]`, a `Pixmap` | here, as an `Err` |
    /// | a direct-to-panel blitter | never — `capacity()` is `None`, meaning *unbounded*, and there is no storage to overflow |
    ///
    /// The **packing agreement** is always a compile error, for every blitter:
    /// both `P::PIXELS_PER_UNIT` and `T::PIXELS_PER_UNIT` are consts, and
    /// comparing a budget against a capacity is meaningless unless they count
    /// the same thing.
    ///
    /// # Errors
    ///
    /// [`AttachError`] when the target's *value* is too small, with the renderer
    /// and the target handed back. **Never panics** — whether a memory-plan
    /// mistake should abort is the application's call, and `unwrap` at the call
    /// site is that call, written where a reader can see it.
    pub fn attach(
        self,
        blitter: T,
    ) -> Result<RasterRenderer<R, T, P, Attached>, AttachError<R, T, P>> {
        // ── static half ───────────────────────────────────────────────────
        //
        // Fires once per instantiation, at monomorphization. A `const` block is
        // invisible to `cargo check` and rust-analyzer — only codegen evaluates
        // one — so this is a build failure rather than an editor diagnostic, and
        // `region.rs`'s eager `const _: () = …` doctests remain the only
        // check-time-visible form.
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
    /// arrive from a channel and which builds its renderer at boot.
    ///
    /// It is spelled on the **attached** type on purpose. That is the type an
    /// application names (`W::Renderer` is the attached one), so a caller gets a
    /// parked renderer without having to write `Detached` into a turbofish:
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

    /// Take the target back. Consumes the renderer and returns it [`Detached`] —
    /// the state with no blitter *field*, so nothing can paint into a target the
    /// caller is holding.
    ///
    /// **The blitter is the loan token.** It owns whatever the caller lent it,
    /// so ownership moves out with it — which is WS6.7's DMA-soundness
    /// requirement, since a borrow the core can still write through is UB. Where
    /// the raw buffer is needed, the blitter hands it over
    /// (`FramebufBlitter::into_storage`, `PixmapBlitter::into_pixmap`); where it
    /// is not, the blitter can be read in place and re-attached whole.
    ///
    /// The rect that was painted comes back with it, as
    /// [`Blitter::bounds`](crate::blitter::Blitter::bounds) — one rect, because
    /// `begin_region` retargets unconditionally, so a tile and a full-frame
    /// buffer report the same thing.
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

// NOTE (attachment rework): four impl blocks lived here and below —
// `with_framebuf`/`detach`/`attach` for `FramebufBlitter` and
// `with_pixmap`/`detach`/`attach` for `PixmapBlitter` — plus an
// `assert_static_capacity` const block. They are replaced by ONE generic
// `attach`/`detach` pair, because the type-state moved from the blitter to the
// renderer (see the struct docs for why).
//
// What went with them, deliberately: the **compile-time** half of the capacity
// proof. It needed `C`, `B` and `P` together, and a generic
// `attach<T: Blitter>` hides the first two inside `T`. Rather than reintroduce
// specialization to keep it, `Blitter` gained a defaulted, dyn-safe
// `pixels_per_unit()` so BOTH halves run at `attach`, for every blitter — which
// is strictly more coverage than the const block had, since it only ever
// guarded the framebuf path while the pixmap path compared units against a
// policy without checking they counted the same thing. The const block was also
// invisible to `cargo check` and rust-analyzer (only codegen evaluates one), so
// what it actually bought was a build failure rather than an editor diagnostic.

#[cfg(all(test, feature = "embedded-graphics"))]
mod raster_renderer_tests {
    use super::*;
    use crate::{
        blitter::framebuf::FramebufBlitter,
        framebuf::PackedColor,
        raster::eg::EgRasterizer,
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

    /// Build and aim at the whole frame.
    ///
    /// A blitter starts aimed at **nothing** — `begin_region` is the only thing
    /// that ever aims one, and on the real path `Frame` always calls it. A test
    /// that paints without planning regions has to do the same, or it paints
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

    /// Detach and unwrap: the parked renderer, the storage, and the rect it
    /// covers. The blitter is the loan token, so taking the buffer out of it is
    /// a second, explicit step — this is that pair, for tests that want the raw
    /// units.
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

    // NOTE (layer split, PR C): `the_layered_renderer_paints_what_eg_renderer_paints`
    // lived here — it ran the same content through `EGRenderer` and through
    // `RasterRenderer<EgRasterizer, FramebufBlitter<..>>` and compared raw
    // storage units, pixel for pixel. It passed, which is what licensed this PR
    // to delete `EGRenderer`; with the reference gone there is nothing left to
    // compare against, so the test goes with it rather than degenerating into a
    // renderer compared with itself.
    //
    // What survives of its guarantee: the tiling equality below (the same claim
    // against a *different* configuration of the same renderer), and the fact
    // that the tile/schedule goldens came out byte-identical across all three
    // PRs. The differential itself is in the PR B commit, and re-creatable by
    // checking that commit out.

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

    /// WS6.3b, through the new stack: the fast `fill_solid` (whole-word writes
    /// in the framebuffer) must land the SAME pixels as the per-pixel path.
    ///
    /// The route is longer than it was — `Renderer::fill_solid` →
    /// `Rasterizer::fill` → `RasterCtx::rect` → `Blitter::fill_rect` →
    /// `Framebuf::fill_solid` — and every hop is a place the rect could be
    /// clipped, shifted or split differently from the per-pixel one.
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

    /// WS6.4d: the renderer **borrows** its surface — it never allocates one and
    /// always gives it back.
    ///
    /// On a device the buffer lives in the application's `StaticCell` pool and
    /// moves through channels; rsact holds it only while painting.
    /// `WidgetCtx: 'static` rules out expressing that as a `&'a mut [T]` field,
    /// so the loan is a move in and a move out — which is also exactly the shape
    /// DMA wants, since a borrow the core could still write through is UB
    /// (roadmap 6.7).
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

        // `parked` has no drawing methods AT ALL — painting between a detach and
        // the next attach does not compile. There is nothing to assert here
        // because there is nothing to call.
        let _ =
            parked.attach(FramebufBlitter::new(surface::<Rgb888>(viewport)));
    }

    /// WS6.4d: after `begin_region`, the rect a buffer covers **is** the region
    /// it was asked to paint — for every surface, not only for tiles.
    ///
    /// This is what lets a caller carry one rectangle instead of two. It held
    /// only for tiles until `begin_region` stopped exempting full-frame
    /// surfaces from retargeting, and the asymmetry was invisible in every test
    /// because each used one surface kind at a time.
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

    /// WS6.4d: a region's units are laid out at **its own width**, so a region
    /// narrower than the frame is contiguous rows of `region.width`.
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

    /// **`begin_region` is the only thing that ever aims a target**, and that
    /// is what retires a whole bug class rather than fixing one instance.
    ///
    /// WS6.4d had a silent failure here: `attach` wrapped every buffer as a tile
    /// (aimed at `Rect::zero()`) while `begin_region` returned early for a
    /// full-frame surface — so nothing re-aimed it, and the ordinary flush loop
    /// reported a zero dirty rect from the second frame onward and sent nothing.
    /// The fix at the time was to make `attach` aim a full-frame buffer at the
    /// frame, which needed a `viewport` the blitter had no business knowing.
    ///
    /// Now a blitter is aimed by `begin_region` and by nothing else, so "aimed
    /// wrongly at construction" is not a state that exists. A reattached target
    /// covers nothing until a region is begun, and exactly the region after.
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

    /// WS6.4d: a renderer **declares** the largest region it will accept, and
    /// its surface is checked against that declaration — not the other way
    /// round.
    ///
    /// This closed a hole that went through two shapes: a `usize::MAX` default
    /// that read as "my surface always covers the frame", and a `PixelBuf for
    /// Box<[S]>` impl claiming the same, which made **every heap surface**
    /// exempt — an *empty* boxed slice satisfied a full-frame policy at compile
    /// time. Capacity a type cannot state is now `None` ("ask the value").
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

    /// WS6.4b: a nested clip must be **narrowed by** its parent, not replace it.
    ///
    /// Asserted where it actually matters — on the framebuffer, not on the
    /// stack: a pixel inside the inner clip but outside the outer one must not
    /// land. It used to, because `push_clip` stored the raw area and the write
    /// filter consulted only the top of the stack, so an inner clip reaching
    /// beyond its parent *widened* the effective clip. Under tiling that is
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

        // The probe color must DIFFER from the untouched framebuffer, or the
        // assertions below hold whatever the clip does: `default_background()`
        // for RGB is WHITE, so a white probe proves nothing (this test was
        // written that way first and passed its "rejected" case vacuously).
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

    /// **New coverage the per-blitter constructors never had.** A policy's unit
    /// budget and a target's capacity are only comparable if they count the same
    /// thing; a 1-bpp target under a `PIXELS_PER_UNIT = 1` policy appears to need
    /// eight times the storage it does. That agreement used to be asserted only
    /// in a `const` block on the framebuf path, so the pixmap path compared the
    /// two without ever checking — now `attach` checks it for every blitter,
    /// because `Blitter::pixels_per_unit` makes it a value.
    /// A packing disagreement is a **compile error**, for every blitter — both
    /// sides are consts. It cannot be asserted at runtime because the code does
    /// not build, which is the point; this records what the message says.
    ///
    /// ```compile_fail
    /// # use rsact_render::{blitter::framebuf::FramebufBlitter, geometry::Size,
    /// #                    raster::eg::EgRasterizer, region::Tiles,
    /// #                    renderer::RasterRenderer};
    /// # use embedded_graphics::pixelcolor::BinaryColor;
    /// // 1-bpp storage under a policy that counts one pixel per unit. The
    /// // buffer is far from too small — the refusal is about the packing.
    /// let buf: &'static mut [u8] = vec![0u8; 4096].leak();
    /// let _ = RasterRenderer::<
    ///     EgRasterizer,
    ///     FramebufBlitter<BinaryColor, &'static mut [u8]>,
    ///     Tiles<240, 24>,
    /// >::with_blitter(
    ///     EgRasterizer, Size::new_equal(240), FramebufBlitter::new(buf),
    /// );
    /// ```
    fn _packing_disagreement_is_a_build_error() {}

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
