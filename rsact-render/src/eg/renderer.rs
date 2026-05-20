use crate::{
    color::Color,
    eg::{
        alpha::{AlphaDrawTarget, StyledAlphaDrawable as _},
        framebuf::{Framebuf as _, PackedColor, PackedFramebuf},
    },
    geometry::*,
    path::{Path, PathSegment},
    primitives::{
        arc::Arc, circle::Circle, ellipse::Ellipse, line::Line,
        rounded_rect::RoundedRect, sector::Sector,
    },
    renderer::{
        AntiAliasing, LayerRenderer, RenderResult, Renderer, RendererOptions,
        Viewport, ViewportKind,
    },
    style::{DrawStyle, StrokeAlignment},
};
use alloc::{collections::BTreeMap, vec::Vec};
use embedded_graphics::{
    Drawable, Pixel,
    draw_target::DrawTargetExt,
    prelude::{Dimensions, DrawTarget},
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, StyledDrawable},
};

impl Into<embedded_graphics::primitives::StrokeAlignment> for StrokeAlignment {
    fn into(self) -> embedded_graphics::primitives::StrokeAlignment {
        match self {
            Self::Inside => {
                embedded_graphics::primitives::StrokeAlignment::Inside
            },
            Self::Center => {
                embedded_graphics::primitives::StrokeAlignment::Center
            },
            Self::Outside => {
                embedded_graphics::primitives::StrokeAlignment::Outside
            },
        }
    }
}

impl<C: Color + embedded_graphics::prelude::PixelColor> DrawStyle<C> {
    pub fn into_primitive_style(self) -> PrimitiveStyle<C> {
        let mut builder = PrimitiveStyleBuilder::new()
            .stroke_width(self.stroke_width)
            .stroke_alignment(self.stroke_alignment.into());
        if let Some(fill) = self.fill {
            builder = builder.fill_color(fill);
        }
        if let Some(stroke) = self.stroke {
            builder = builder.stroke_color(stroke);
        }
        builder.build()
    }
}

// ── Layer ──────────────────────────────────────────────────────────────────

// Note: Real alpha channel is not supported. Now, alpha channel is more like
// blending parameter for drawing on a single layer, so each layer is not
// transparent and alpha parameter only affects blending on current layer.
// TODO: Real alpha-channel
struct Layer<C: Color + PackedColor + embedded_graphics::prelude::PixelColor> {
    canvas: PackedFramebuf<C>,
}

impl<C: Color + PackedColor + embedded_graphics::prelude::PixelColor> Layer<C> {
    fn fullscreen(size: Size) -> Self {
        Self { canvas: PackedFramebuf::new(size, C::default_background()) }
    }
}

/// Renderer backed by embedded_graphics. Combines buffering and layering into
/// one structure.
///
/// Preserves PackedColor framebuffer optimization, alpha channel blending,
/// anti-aliasing, and layering support.
pub struct EGRenderer<
    C: Color + PackedColor + embedded_graphics::prelude::PixelColor,
> {
    viewport_stack: Vec<Viewport>,
    layers: BTreeMap<usize, Layer<C>>,
    main_viewport: Size,
    options: RendererOptions,
}

impl<C: Color + PackedColor + embedded_graphics::prelude::PixelColor>
    EGRenderer<C>
{
    pub fn new(viewport: Size) -> Self {
        Self {
            viewport_stack: vec![Viewport::root()],
            layers: BTreeMap::from([(0, Layer::fullscreen(viewport))]),
            main_viewport: viewport,
            options: RendererOptions::default(),
        }
    }

    fn current_viewport(&self) -> Viewport {
        self.viewport_stack.last().copied().unwrap()
    }

    fn layer_index(&self) -> usize {
        self.current_viewport().layer
    }

    fn sub_viewport(&self, kind: ViewportKind) -> Viewport {
        Viewport { layer: self.layer_index(), kind }
    }

    fn current_canvas(&mut self) -> &mut PackedFramebuf<C> {
        let layer_index = self.layer_index();
        &mut self.layers.get_mut(&layer_index).unwrap().canvas
    }

    /// Obtain the raw framebuffer data from layer 0 for hardware output.
    pub fn draw_buffer(&self, f: impl FnOnce(&[<C as PackedColor>::Storage])) {
        self.layers.get(&0).unwrap().canvas.draw_buffer(f);
    }

    fn aa_enabled(&self) -> bool {
        matches!(self.options.anti_aliasing, Some(AntiAliasing::Enabled))
    }

    fn draw_pixels(
        &mut self,
        viewport: Viewport,
        pixels: impl IntoIterator<Item = Pixel<C>>,
    ) -> Result<(), ()> {
        let canvas = &mut self.layers.get_mut(&viewport.layer).unwrap().canvas;
        match viewport.kind {
            ViewportKind::Fullscreen => canvas.draw_iter(pixels),
            ViewportKind::Clipped(area) => {
                canvas.clipped(&area.into()).draw_iter(pixels)
            },
            ViewportKind::Cropped(area) => {
                canvas.cropped(&area.into()).draw_iter(pixels)
            },
        }
        .unwrap();
        Ok(())
    }
}

impl<C: Color + PackedColor + embedded_graphics::prelude::PixelColor> Dimensions
    for EGRenderer<C>
{
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        embedded_graphics::primitives::Rectangle::new(
            embedded_graphics::geometry::Point::zero(),
            self.main_viewport.into(),
        )
    }
}

impl<C: Color + PackedColor + embedded_graphics::prelude::PixelColor> DrawTarget
    for EGRenderer<C>
{
    type Color = C;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let viewport = self.current_viewport();
        self.draw_pixels(viewport, pixels)
    }
}

impl<C: Color + PackedColor + embedded_graphics::prelude::PixelColor>
    AlphaDrawTarget for EGRenderer<C>
{
    fn pixel_alpha(
        &mut self,
        pixel: Pixel<<Self as DrawTarget>::Color>,
        blend: f32,
    ) -> RenderResult {
        let viewport = self.current_viewport();
        let canvas = &mut self.layers.get_mut(&viewport.layer).unwrap().canvas;
        let color = canvas
            .pixel(pixel.0)
            .map(|current| current.mix(blend, pixel.1))
            .unwrap_or(pixel.1);
        match viewport.kind {
            ViewportKind::Fullscreen => {
                canvas.draw_iter(core::iter::once(Pixel(pixel.0, color)))
            },
            ViewportKind::Clipped(area) => canvas
                .clipped(&area.into())
                .draw_iter(core::iter::once(Pixel(pixel.0, color))),
            ViewportKind::Cropped(area) => canvas
                .cropped(&area.into())
                .draw_iter(core::iter::once(Pixel(pixel.0, color))),
        }
        .unwrap();
        Ok(())
    }
}

impl<C: Color + PackedColor + embedded_graphics::prelude::PixelColor> Drawable
    for EGRenderer<C>
{
    type Color = C;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.layers.values().try_for_each(|layer| layer.canvas.draw(target))
    }
}

impl<C: Color + PackedColor + embedded_graphics::prelude::PixelColor>
    LayerRenderer for EGRenderer<C>
{
    fn on_layer(
        &mut self,
        index: usize,
        f: impl FnOnce(&mut Self) -> RenderResult,
    ) -> RenderResult {
        self.layers.insert(index, Layer::fullscreen(self.main_viewport));
        self.viewport_stack
            .push(Viewport { layer: index, kind: ViewportKind::Fullscreen });
        let result = f(self);
        self.viewport_stack.pop();
        result
    }
}

impl<C: Color + PackedColor + embedded_graphics::prelude::PixelColor> Renderer
    for EGRenderer<C>
where
    C: From<<C as embedded_graphics::prelude::PixelColor>::Raw>,
{
    type Color = C;
    type Options = RendererOptions;

    fn set_options(&mut self, options: Self::Options) {
        self.options = options;
    }

    fn clipped(
        &mut self,
        area: Rect,
        f: impl FnOnce(&mut Self) -> RenderResult,
    ) -> RenderResult {
        self.viewport_stack
            .push(self.sub_viewport(ViewportKind::Clipped(area)));
        let result = f(self);
        self.viewport_stack.pop();
        result
    }

    fn fill_solid(&mut self, rect: &Rect, color: Self::Color) -> RenderResult {
        let rect: embedded_graphics::primitives::Rectangle = (*rect).into();
        DrawTarget::fill_solid(self, &rect, color).ok().unwrap();
        Ok(())
    }

    fn draw_line(
        &mut self,
        from: Point,
        to: Point,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        let style = style.into_primitive_style();
        if self.aa_enabled() {
            Line::new(from.into(), to.into()).draw_styled_alpha(&style, self)
        } else {
            Line::new(from, to).draw_styled(&style, self).ok().unwrap();
            Ok(())
        }
    }

    fn draw_rect(
        &mut self,
        rect: Rect,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        let eg_rect: embedded_graphics::primitives::Rectangle = rect.into();
        let eg_style = style.into_primitive_style();
        eg_rect.draw_styled(&eg_style, self).ok().unwrap();
        Ok(())
    }

    fn draw_rounded_rect(
        &mut self,
        rect: Rect,
        corners: CornerRadii,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        let style = style.into_primitive_style();
        if self.aa_enabled() {
            RoundedRect::new(rect, corners).draw_styled_alpha(&style, self)
        } else {
            RoundedRect::new(rect, corners)
                .draw_styled(&style, self)
                .ok()
                .unwrap();
            Ok(())
        }
    }

    fn draw_circle(
        &mut self,
        top_left: Point,
        diameter: u32,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        let style = style.into_primitive_style();
        if self.aa_enabled() {
            Circle::new(top_left, diameter).draw_styled_alpha(&style, self)
        } else {
            Circle::new(top_left, diameter)
                .draw_styled(&style, self)
                .ok()
                .unwrap();
            Ok(())
        }
    }

    fn draw_arc(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        let style = style.into_primitive_style();
        if self.aa_enabled() {
            Arc::new(top_left, diameter, start, sweep)
                .draw_styled_alpha(&style, self)
        } else {
            Arc::new(top_left, diameter, start, sweep)
                .draw_styled(&style, self)
                .ok()
                .unwrap();
            Ok(())
        }
    }

    fn draw_ellipse(
        &mut self,
        bounding_box: Rect,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        let top_left = bounding_box.top_left;
        let size = bounding_box.size;
        let style = style.into_primitive_style();
        if self.aa_enabled() {
            Ellipse::new(top_left, size).draw_styled_alpha(&style, self)
        } else {
            Ellipse::new(top_left, size)
                .draw_styled(&style, self)
                .ok()
                .unwrap();
            Ok(())
        }
    }

    fn draw_sector(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        let style = style.into_primitive_style();
        if self.aa_enabled() {
            Sector::new(top_left, diameter, start, sweep)
                .draw_styled_alpha(&style, self)
        } else {
            Sector::new(top_left, diameter, start, sweep)
                .draw_styled(&style, self)
                .ok()
                .unwrap();
            Ok(())
        }
    }

    fn draw_polygon(
        &mut self,
        points: &[Point],
        style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        // TODO: I don't want to allocate a vector for conversion between my Point and EG Point, so better use custom primitive Polygon and implement AA and non-AA rendering for it.
        todo!()
        // let style = style.into_primitive_style();
        // if self.aa_enabled() {
        //     Polyline::new(points.iter())
        //         .draw_styled_alpha(&style, self)
        // } else {
        //     super::primitives::polygon::Polygon::new(points.iter().copied())
        //         .draw_styled(&style, self)
        //         .ok()
        //         .unwrap();
        //     Ok(())
        // }
    }

    fn draw_path(
        &mut self,
        path: &Path,
        style: DrawStyle<Self::Color>,
    ) -> RenderResult {
        let style = style.into_primitive_style();
        let mut current_pos = Point::zero();
        for segment in path.segments() {
            match segment {
                PathSegment::MoveTo(p) => {
                    current_pos = *p;
                },
                PathSegment::LineTo(p) => {
                    Line::new(current_pos.into(), (*p).into())
                        .draw_styled(&style, self)
                        .ok()
                        .unwrap();
                    current_pos = *p;
                },
                PathSegment::ArcTo { center: _, radius, start, sweep } => {
                    let diameter = radius * 2;
                    let top_left = Point::new(
                        current_pos.x - *radius as i32,
                        current_pos.y - *radius as i32,
                    );
                    Arc::new(
                        top_left.into(),
                        diameter,
                        (*start).into(),
                        (*sweep).into(),
                    )
                    .draw_styled(&style, self)
                    .ok()
                    .unwrap();
                },
                PathSegment::Close => {},
            }
        }
        Ok(())
    }
}
