use crate::{
    color::Color,
    eg::{
        framebuf::{Framebuf as _, PackedColor, PackedFramebuf},
        primitives::EgPrimitive,
    },
    geometry::*,
    output::{FinishRender, MapColor, RenderTarget, pixel::Pixel},
    path::{Path, PathSegment},
    primitives::{
        arc::Arc, circle::Circle, ellipse::Ellipse, line::Line,
        rounded_rect::RoundedRect, sector::Sector,
    },
    renderer::{
        AntiAliasing, AntiAliasingDisabled, AntiAliasingEnabled, RenderResult,
        Renderer, Viewport, ViewportKind,
    },
    style::{DrawStyle, StrokeAlignment},
};
use alloc::{collections::BTreeMap, vec::Vec};
use core::marker::PhantomData;
use embedded_graphics::{
    draw_target::DrawTargetExt,
    prelude::{Dimensions, DrawTarget, PixelColor},
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

impl<C: Color + PixelColor> DrawStyle<C> {
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

// Note: Real alpha channel is not supported. Now, alpha channel is more like
// blending parameter for drawing on a single layer, so each layer is not
// transparent and alpha parameter only affects blending on current layer.
// TODO: Real alpha-channel
// TODO: Use common [`Layering`]
struct Layer<C: Color + PackedColor> {
    canvas: PackedFramebuf<C>,
}

impl<C: Color + PackedColor> Layer<C> {
    fn fullscreen(size: Size) -> Self {
        Self { canvas: PackedFramebuf::new(size, C::default_background()) }
    }
}

/// Renderer backed by embedded_graphics. Combines buffering and layering into
/// one structure.
///
/// Preserves PackedColor framebuffer optimization, alpha channel blending,
/// anti-aliasing, and layering support.
pub struct EGRenderer<C: Color + PackedColor, AA: AntiAliasing> {
    viewport_stack: Vec<Viewport>,
    layers: BTreeMap<usize, Layer<C>>,
    main_viewport: Size,
    aa: PhantomData<AA>,
}

impl<C: Color + PackedColor> EGRenderer<C, AntiAliasingDisabled> {
    pub fn new(viewport: Size) -> Self {
        Self {
            viewport_stack: vec![Viewport::root()],
            layers: BTreeMap::from([(0, Layer::fullscreen(viewport))]),
            main_viewport: viewport,
            aa: PhantomData,
        }
    }
}

impl<C: Color + PackedColor + PixelColor, AA: AntiAliasing> EGRenderer<C, AA> {
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

    pub fn pixel_alpha(&mut self, pixel: Pixel<C>, blend: f32) -> RenderResult {
        let canvas = self.current_canvas();
        let color = canvas
            .pixel(pixel.0)
            .map(|current| current.mix(blend, pixel.1))
            .unwrap_or(pixel.1);
        self.draw_pixels(core::iter::once(Pixel(pixel.0, color)))
    }

    pub fn draw_pixels(
        &mut self,
        pixels: impl IntoIterator<Item = Pixel<C>>,
    ) -> Result<(), ()> {
        let viewport = self.current_viewport();
        let canvas = &mut self.layers.get_mut(&viewport.layer).unwrap().canvas;
        let eg_pixels = pixels
            .into_iter()
            .map(|p| embedded_graphics::prelude::Pixel(p.0.into(), p.1));
        match viewport.kind {
            ViewportKind::Fullscreen => canvas.draw_iter(eg_pixels),
            ViewportKind::Clipped(area) => {
                canvas.clipped(&area.into()).draw_iter(eg_pixels)
            },
            ViewportKind::Cropped(area) => {
                canvas.cropped(&area.into()).draw_iter(eg_pixels)
            },
        }
        .unwrap();
        Ok(())
    }

    // Renderer common implementations
    fn renderer_output<TC>(&self, target: &mut impl RenderTarget<Color = TC>)
    where
        C: MapColor<TC>,
    {
        self.layers.values().for_each(|layer| layer.canvas.output(target))
    }

    fn renderer_clipped(
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
}

// impl<C: Color + PackedColor + embedded_graphics::prelude::PixelColor>
//     LayerRenderer for EGRenderer<C>
// {
//     fn on_layer(
//         &mut self,
//         index: usize,
//         f: impl FnOnce(&mut Self) -> RenderResult,
//     ) -> RenderResult {
//         self.layers.insert(index, Layer::fullscreen(self.main_viewport));
//         self.viewport_stack
//             .push(Viewport { layer: index, kind: ViewportKind::Fullscreen });
//         let result = f(self);
//         self.viewport_stack.pop();
//         result
//     }
// }

impl<C: Color + PackedColor + PixelColor, AA: AntiAliasing> DrawTarget
    for EGRenderer<C, AA>
{
    type Color = C;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::prelude::Pixel<Self::Color>>,
    {
        self.draw_pixels(pixels.into_iter().map(|p| Pixel(p.0.into(), p.1)))
    }
}

impl<C: Color + PackedColor + PixelColor, AA: AntiAliasing> Dimensions
    for EGRenderer<C, AA>
{
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        embedded_graphics::primitives::Rectangle::new(
            embedded_graphics::geometry::Point::zero(),
            self.main_viewport.into(),
        )
    }
}

// TODO: Other colors mapping
impl<C: Color + PackedColor + PixelColor, AA: AntiAliasing> FinishRender<C>
    for EGRenderer<C, AA>
{
    fn finish_frame(&mut self, target: &mut impl RenderTarget<Color = C>) {
        self.renderer_output(target);
    }
}

// TODO: Generalize AA and non-AA Renderer implementations

impl<C: Color + PackedColor + PixelColor> Renderer
    for EGRenderer<C, AntiAliasingDisabled>
{
    type Color = C;
    type Options = ();

    fn set_options(&mut self, _options: Self::Options) {}

    fn size(&self) -> Size {
        self.main_viewport
    }

    fn clipped(
        &mut self,
        area: Rect,
        f: impl FnOnce(&mut Self) -> RenderResult,
    ) -> RenderResult {
        self.renderer_clipped(area, f)
    }

    fn fill_solid(&mut self, rect: Rect, color: Self::Color) -> RenderResult {
        self.rect(
            rect,
            &DrawStyle {
                fill: Some(color),
                stroke: None,
                stroke_width: 0,
                stroke_alignment: StrokeAlignment::Inside,
            },
        )
    }

    fn line(
        &mut self,
        from: Point,
        to: Point,
        style: &DrawStyle<C>,
    ) -> RenderResult {
        Line::new(from, to).draw(self, *style)
    }

    fn rect(
        &mut self,
        rect: Rect,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let eg_rect: embedded_graphics::primitives::Rectangle = rect.into();
        eg_rect.draw_styled(&style.into_primitive_style(), self).ok().unwrap();
        Ok(())
    }

    fn rounded_rect(
        &mut self,
        rect: Rect,
        corners: CornerRadii,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        RoundedRect::new(rect, corners).draw(self, *style)
    }

    fn circle(
        &mut self,
        top_left: Point,
        diameter: u32,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Circle::new(top_left, diameter).draw(self, *style)
    }

    fn arc(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Arc::new(top_left, diameter, start, sweep).draw(self, *style)
    }

    fn ellipse(
        &mut self,
        bounding_box: Rect,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ellipse::new(bounding_box.top_left, bounding_box.size)
            .draw(self, *style)
    }

    fn sector(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Sector::new(top_left, diameter, start, sweep).draw(self, *style)
    }

    fn polygon(
        &mut self,
        points: &[Point],
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // TODO: I don't want to allocate a vector for conversion between my Point and EG Point, so better use custom primitive Polygon and implement AA and non-AA rendering for it.
        todo!()
    }

    fn path(
        &mut self,
        path: &Path,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let mut current_pos = Point::zero();
        for segment in path.segments() {
            match segment {
                PathSegment::MoveTo(p) => {
                    current_pos = *p;
                },
                PathSegment::LineTo(p) => {
                    self.line(current_pos, *p, style)?;
                    current_pos = *p;
                },
                PathSegment::ArcTo { center: _, radius, start, sweep } => {
                    let diameter = radius * 2;
                    let top_left = Point::new(
                        current_pos.x - *radius as i32,
                        current_pos.y - *radius as i32,
                    );
                    self.arc(top_left, diameter, *start, *sweep, style)?;
                },
                PathSegment::Close => {},
            }
        }
        Ok(())
    }
}

impl<C: Color + PackedColor + PixelColor> Renderer
    for EGRenderer<C, AntiAliasingEnabled>
{
    type Color = C;
    type Options = ();

    fn set_options(&mut self, _options: Self::Options) {}

    fn size(&self) -> Size {
        self.main_viewport
    }

    fn clipped(
        &mut self,
        area: Rect,
        f: impl FnOnce(&mut Self) -> RenderResult,
    ) -> RenderResult {
        self.renderer_clipped(area, f)
    }

    fn fill_solid(&mut self, rect: Rect, color: Self::Color) -> RenderResult {
        self.rect(
            rect,
            &DrawStyle {
                fill: Some(color),
                stroke: None,
                stroke_width: 0,
                stroke_alignment: StrokeAlignment::Inside,
            },
        )
    }

    fn line(
        &mut self,
        from: Point,
        to: Point,
        style: &DrawStyle<C>,
    ) -> RenderResult {
        Line::new(from, to).draw_aa(self, *style)
    }

    fn rect(
        &mut self,
        rect: Rect,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let eg_rect: embedded_graphics::primitives::Rectangle = rect.into();
        eg_rect.draw_styled(&style.into_primitive_style(), self).ok().unwrap();
        Ok(())
    }

    fn rounded_rect(
        &mut self,
        rect: Rect,
        corners: CornerRadii,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        RoundedRect::new(rect, corners).draw_aa(self, *style)
    }

    fn circle(
        &mut self,
        top_left: Point,
        diameter: u32,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Circle::new(top_left, diameter).draw_aa(self, *style)
    }

    fn arc(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Arc::new(top_left, diameter, start, sweep).draw_aa(self, *style)
    }

    fn ellipse(
        &mut self,
        bounding_box: Rect,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Ellipse::new(bounding_box.top_left, bounding_box.size)
            .draw_aa(self, *style)
    }

    fn sector(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        Sector::new(top_left, diameter, start, sweep).draw_aa(self, *style)
    }

    fn polygon(
        &mut self,
        points: &[Point],
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // TODO: I don't want to allocate a vector for conversion between my Point and EG Point, so better use custom primitive Polygon and implement AA and non-AA rendering for it.
        todo!()
    }

    fn path(
        &mut self,
        path: &Path,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let mut current_pos = Point::zero();
        for segment in path.segments() {
            match segment {
                PathSegment::MoveTo(p) => {
                    current_pos = *p;
                },
                PathSegment::LineTo(p) => {
                    self.line(current_pos, *p, style)?;
                    current_pos = *p;
                },
                PathSegment::ArcTo { center: _, radius, start, sweep } => {
                    let diameter = radius * 2;
                    let top_left = Point::new(
                        current_pos.x - *radius as i32,
                        current_pos.y - *radius as i32,
                    );
                    Arc::new(top_left, diameter, *start, *sweep)
                        .draw_aa(self, *style)?;
                },
                PathSegment::Close => {},
            }
        }
        Ok(())
    }
}
