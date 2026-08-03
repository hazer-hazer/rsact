use crate::{
    color::{Color, Rgba},
    geometry::*,
    image::DrawImage,
    output::{FinishRender, RenderTarget},
    path::Path,
    style::DrawStyle,
};

use core::marker::PhantomData;

pub type RenderResult = Result<(), ()>;

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
    /// Clipped part of parent layer with absolute positions relative to screen
    /// top-left point
    Clipped(Rect),
    /// Part of parent layer with positions relative to this layer top-left
    /// point
    Cropped(Rect),
}

#[derive(Clone, Copy)]
pub struct Viewport {
    /// It's okay to have multiple Layers pointing to the same Canvas as it can
    /// be Clipped or Cropped but not for overlaying
    pub layer: usize,
    pub kind: ViewportKind,
}

impl Viewport {
    pub fn root() -> Self {
        Self { layer: 0, kind: ViewportKind::Fullscreen }
    }
}

/// Core renderer trait: defines primitive drawing methods independent of
/// embedded_graphics.
pub trait Renderer {
    type Color: Color;

    /// How many storage units this renderer's surface holds — the capacity side
    /// of WS6.4.0(iii)'s compile-time tile check.
    ///
    /// `usize::MAX` means "my surface always covers the frame": a GPU, a host
    /// renderer owning a resizable buffer, `NullRenderer`. Such a renderer
    /// accepts any frame policy. A tile-backed renderer overrides this with its
    /// buffer's `PixelBuf::UNITS`, after which a policy asking for a region
    /// larger than the buffer fails to compile — see
    /// [`assert_region_fits`](crate::eg::framebuf::assert_region_fits).
    ///
    /// The default sits at the permissive end on purpose: a renderer that has
    /// not opted into tiling is one that never needed the check.
    const SURFACE_UNITS: usize = usize::MAX;

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

impl<C, D> FinishRender<C> for NullRenderer<D> {
    fn finish_frame(&mut self, target: &mut impl RenderTarget<Color = C>) {
        let _ = target;
    }
}

impl<C: Color> Renderer for NullRenderer<C> {
    type Color = C;

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
