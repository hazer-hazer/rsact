use crate::{
    geometry::*,
    layout::{block_model::BlockModel, padding::Padding},
    render::color::Color,
    style::block::{BlockStyle, BorderRadius, BorderStyle},
    widget::RenderResult,
};
use path::Path;
use rsact_reactive::prelude::IntoMaybeReactive;

pub mod color;
pub mod path;
pub mod primitives;

#[cfg(feature = "tiny-skia")]
pub mod tiny_skia;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeAlignment {
    Inside,
    Center,
    Outside,
}

impl Default for StrokeAlignment {
    fn default() -> Self {
        Self::Inside
    }
}

/// Unified style for drawing filled/stroked primitives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawStyle<C: Color> {
    pub fill: Option<C>,
    pub stroke: Option<C>,
    pub stroke_width: u32,
    pub stroke_alignment: StrokeAlignment,
}

impl<C: Color> Default for DrawStyle<C> {
    fn default() -> Self {
        Self {
            fill: None,
            stroke: None,
            stroke_width: 0,
            stroke_alignment: StrokeAlignment::Inside,
        }
    }
}

impl<C: Color> DrawStyle<C> {
    pub fn filled(color: C) -> Self {
        Self { fill: Some(color), ..Default::default() }
    }

    pub fn stroked(color: C, width: u32) -> Self {
        Self { stroke: Some(color), stroke_width: width, ..Default::default() }
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

#[derive(Debug, Clone, Copy)]
pub struct Border<C: Color> {
    pub color: Option<C>,
    pub width: u32,
    pub radius: BorderRadius,
}

impl<C: Color> Border<C> {
    pub fn new(block_style: BlockStyle<C>, block_model: BlockModel) -> Self {
        Self {
            color: block_style.border.color.get(),
            width: block_model.border_width,
            radius: block_style.border.radius,
        }
    }

    pub fn zero() -> Self {
        Self { color: None, width: 0, radius: 0.into() }
    }

    pub fn color(mut self, color: Option<C>) -> Self {
        self.color = color;
        self
    }

    pub fn width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    pub fn radius(mut self, radius: impl Into<BorderRadius>) -> Self {
        self.radius = radius.into();
        self
    }

    /// Make Block for border used as outline. Background color is always
    /// removed to avoid drawing above element.
    pub fn into_outline(self, bounds: Rect) -> Block<C> {
        Block { rect: bounds, background: None, border: self }
    }

    pub fn into_block(self, bounds: Rect, background: Option<C>) -> Block<C> {
        Block { rect: bounds, background, border: self }
    }
}

impl<C: Color> Into<Padding> for Border<C> {
    fn into(self) -> Padding {
        self.width.into()
    }
}

#[derive(Clone, Copy)]
pub struct Block<C: Color> {
    pub border: Border<C>,
    pub rect: Rect,
    pub background: Option<C>,
}

impl<C: Color> Block<C> {
    /// Render this block using the renderer's primitive drawing methods.
    pub fn render<R: Renderer<Color = C>>(
        &self,
        renderer: &mut R,
    ) -> RenderResult {
        renderer.draw_rounded_rect(
            self.rect,
            self.border.radius.into_corner_radii(self.rect.size),
            DrawStyle {
                fill: self.background,
                stroke: self.border.color,
                stroke_width: self.border.width,
                stroke_alignment: StrokeAlignment::Inside,
            },
        )
    }

    // TODO: Find better way to construct Block. border width inside layout
    // makes it complex
    #[inline]
    pub fn from_layout_style(
        outer: Rect,
        BlockModel { border_width, padding: _ }: BlockModel,
        BlockStyle {
            background_color,
            border: BorderStyle { color: border_color, radius },
        }: BlockStyle<C>,
    ) -> Self {
        Self {
            border: Border {
                color: border_color.get(),
                width: border_width,
                radius,
            },
            rect: outer,
            background: background_color.get(),
        }
    }
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
pub(crate) struct NullRenderer;

#[cfg(feature = "embedded-graphics")]
impl embedded_graphics::prelude::PixelColor for NullColor {
    type Raw = embedded_graphics_core::pixelcolor::raw::RawU8;
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
