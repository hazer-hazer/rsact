use crate::{
    color::Color,
    geometry::{block_model::BlockModel, border::Border, *},
    path::Path,
    style::{
        DrawStyle, StrokeAlignment,
        block::{BlockStyle, BorderRadius, BorderStyle},
    },
};
use rsact_reactive::prelude::IntoMaybeReactive;

pub type RenderResult = Result<(), ()>;

#[derive(PartialEq, Clone)]
pub enum AntiAliasing {
    Disabled,
    Enabled,
}

#[derive(Default, Clone, PartialEq, IntoMaybeReactive)]
pub struct RendererOptions {
    pub anti_aliasing: Option<AntiAliasing>,
}

impl RendererOptions {
    pub fn new() -> Self {
        Self { anti_aliasing: None }
    }

    // TODO: Simple `with_anti_aliasing` method shortcut
    pub fn anti_aliasing(mut self, aa: AntiAliasing) -> Self {
        self.anti_aliasing = Some(aa);
        self
    }
}

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
    type Options: PartialEq + Clone + Default;

    fn set_options(&mut self, options: Self::Options);

    fn clipped(
        &mut self,
        area: Rect,
        f: impl FnOnce(&mut Self) -> RenderResult,
    ) -> RenderResult;

    fn fill_solid(&mut self, rect: &Rect, color: Self::Color) -> RenderResult;

    fn draw_line(
        &mut self,
        from: Point,
        to: Point,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn draw_rect(
        &mut self,
        rect: Rect,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn draw_rounded_rect(
        &mut self,
        rect: Rect,
        corners: CornerRadii,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn draw_circle(
        &mut self,
        top_left: Point,
        diameter: u32,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn draw_arc(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn draw_ellipse(
        &mut self,
        bounding_box: Rect,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn draw_sector(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn draw_polygon(
        &mut self,
        points: &[Point],
        style: DrawStyle<Self::Color>,
    ) -> RenderResult;

    fn draw_path(
        &mut self,
        path: &Path,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult;
}

pub trait LayerRenderer {
    fn on_layer(
        &mut self,
        index: usize,
        f: impl FnOnce(&mut Self) -> RenderResult,
    ) -> RenderResult;
}

/// Minimal color type for use in NullRenderer (no embedded_graphics needed).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NullColor;

impl Color for NullColor {
    fn default_foreground() -> Self {
        NullColor
    }

    fn default_background() -> Self {
        NullColor
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

/// Stub renderer for tests.
#[derive(Default)]
pub struct NullRenderer;

#[cfg(feature = "embedded-graphics")]
impl embedded_graphics::prelude::PixelColor for NullColor {
    type Raw = embedded_graphics::pixelcolor::raw::RawU8;
}

#[cfg(feature = "embedded-graphics")]
impl embedded_graphics::prelude::Dimensions for NullRenderer {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        embedded_graphics::primitives::Rectangle::zero()
    }
}

#[cfg(feature = "embedded-graphics")]
impl embedded_graphics::prelude::DrawTarget for NullRenderer {
    type Color = NullColor;
    type Error = ();

    fn draw_iter<I>(&mut self, _pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        Ok(())
    }
}

impl Renderer for NullRenderer {
    type Color = NullColor;
    type Options = ();

    fn set_options(&mut self, _options: Self::Options) {}

    fn clipped(
        &mut self,
        _area: Rect,
        f: impl FnOnce(&mut Self) -> RenderResult,
    ) -> RenderResult {
        f(self)
    }

    fn fill_solid(
        &mut self,
        _rect: &Rect,
        _color: Self::Color,
    ) -> RenderResult {
        Ok(())
    }

    fn draw_line(
        &mut self,
        _from: Point,
        _to: Point,
        _style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn draw_rect(
        &mut self,
        _rect: Rect,
        _style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn draw_rounded_rect(
        &mut self,
        _rect: Rect,
        _corners: CornerRadii,
        _style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn draw_circle(
        &mut self,
        _top_left: Point,
        _diameter: u32,
        _style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn draw_arc(
        &mut self,
        _top_left: Point,
        _diameter: u32,
        _start: Angle,
        _sweep: Angle,
        _style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn draw_ellipse(
        &mut self,
        _bounding_box: Rect,
        _style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn draw_sector(
        &mut self,
        _top_left: Point,
        _diameter: u32,
        _start: Angle,
        _sweep: Angle,
        _style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn draw_polygon(
        &mut self,
        _points: &[Point],
        _style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }

    fn draw_path(
        &mut self,
        _path: &Path,
        _style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ok(())
    }
}
