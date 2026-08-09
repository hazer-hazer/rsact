use crate::{
    color::{Color, Rgba},
    geometry::*,
    image::DrawImage,
    output::RenderTarget,
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
/// [`units_for`](crate::eg::framebuf::units_for) is the colour-typed wrapper
/// the embedded-graphics backend uses — they must not be allowed to drift,
/// because a capacity check that disagrees with the buffer's real layout is
/// worse than no check at all.
///
/// Padding per row is what makes sub-byte packing correct: a 122-pixel 1-bpp
/// row occupies 16 bytes, not 15.25. Area-based arithmetic gets this wrong, and
/// is exactly the bug roadmap 6.5 has to undo in `PackedFramebuf::new`.
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

pub trait AntiAliasing {}

pub struct AntiAliasingEnabled;
impl AntiAliasing for AntiAliasingEnabled {}

pub struct AntiAliasingDisabled;
impl AntiAliasing for AntiAliasingDisabled {}

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
    /// surface — no `Surface` trait, no capacity number, no buffer type
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

/// A renderer that draws nothing, generic over the colour it accepts.
///
/// Two jobs. It is the stub every headless test and size/metrics probe builds a
/// `Wtf` around — hence `C = NullColor` by default, so `Wtf<NullRenderer, ..>`
/// and `&mut NullRenderer` keep working unannotated. And (WS6.4.0(ii-4)) it is
/// what 6.4c's **collect pass** runs widget bodies against: that pass must
/// genuinely execute each body so reactive dependencies re-track and damage
/// rects are pushed, but must not rasterise, and it has to satisfy
/// `Renderer<Color = W::Color>` for the *application's* colour — which the
/// previous `type Color = NullColor` hard-wiring could not express.
///
/// It carries no state, so `NullRenderer::<C>::default()` is free.
pub struct NullRenderer<C = NullColor> {
    _color: PhantomData<C>,
}

// Hand-written rather than derived: `#[derive(Default)]` would demand
// `C: Default`, which no colour needs to satisfy for an empty struct.
impl<C> Default for NullRenderer<C> {
    fn default() -> Self {
        Self { _color: PhantomData }
    }
}

impl<C: Color> RenderTarget for NullRenderer<C> {
    type Color = C;

    fn draw(
        &mut self,
        _pixels: impl Iterator<Item = crate::output::pixel::Pixel<Self::Color>>,
    ) {
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
