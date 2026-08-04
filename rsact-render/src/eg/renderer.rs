use crate::{
    color::{Color, RgbColor},
    eg::{
        framebuf::{Framebuf as _, PackedColor, PackedFramebuf},
        primitives::EgPrimitive,
    },
    geometry::*,
    image::DrawImage,
    output::{FinishRender, MapColor, RenderTarget, pixel::Pixel},
    path::{Path, PathSegment},
    primitives::{
        arc::Arc, circle::Circle, ellipse::Ellipse, line::Line,
        rounded_rect::RoundedRect, sector::Sector,
    },
    renderer::{
        AntiAliasing, AntiAliasingDisabled, AntiAliasingEnabled, RenderResult,
        Renderer, ViewportKind,
    },
    style::{DrawStyle, StrokeAlignment},
};
use alloc::vec::Vec;
use core::marker::PhantomData;
use embedded_graphics::{
    Drawable,
    draw_target::DrawTargetExt,
    geometry::OriginDimensions,
    pixelcolor::Rgb888,
    prelude::{Dimensions, DrawTarget, PixelColor},
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, StyledDrawable},
};

/// Proxy to draw on Renderer as on embedded_graphics DrawTarget, works by
/// mapping any color into embedded_graphics Rgb888.
pub struct DrawTargetProxy<'a, R: Renderer> {
    renderer: &'a mut R,
}

impl<'a, R: Renderer> DrawTargetProxy<'a, R> {
    pub fn new(renderer: &'a mut R) -> Self {
        Self { renderer }
    }
}

impl<'a, R: Renderer> OriginDimensions for DrawTargetProxy<'a, R> {
    fn size(&self) -> embedded_graphics::prelude::Size {
        self.renderer.size().into()
    }
}

impl<'a, C: Color, R: Renderer<Color = C>> DrawTarget
    for DrawTargetProxy<'a, R>
{
    type Color = Rgb888;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::prelude::Pixel<Self::Color>>,
    {
        pixels.into_iter().try_for_each(|p| {
            self.renderer.pixel(
                p.0.into(),
                C::from_rgba(crate::color::Rgba {
                    r: p.1.r(),
                    g: p.1.g(),
                    b: p.1.b(),
                    a: 255,
                }),
            )
        })
    }
}

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

/// Renderer backed by embedded_graphics, drawing into a single owned
/// `PackedFramebuf` under a clip/crop viewport stack.
///
/// Preserves the PackedColor framebuffer optimization, alpha-channel blending,
/// and anti-aliasing. Layer compositing was removed (see [`crate::surface`]).
// TODO: Use the common [`crate::surface::Canvas`] surface + viewport helper
// instead of holding `canvas` + `viewport_stack` inline here.
pub struct EGRenderer<C: Color + PackedColor, AA: AntiAliasing> {
    viewport_stack: Vec<ViewportKind>,
    canvas: PackedFramebuf<C>,
    main_viewport: Size,
    aa: PhantomData<AA>,
}

impl<C: Color + PackedColor> EGRenderer<C, AntiAliasingDisabled> {
    pub fn new(viewport: Size) -> Self {
        Self {
            viewport_stack: vec![ViewportKind::root()],
            canvas: PackedFramebuf::new(viewport, C::default_background()),
            main_viewport: viewport,
            aa: PhantomData,
        }
    }
}

impl<C: Color + PackedColor + PixelColor, AA: AntiAliasing> EGRenderer<C, AA> {
    fn current_viewport(&self) -> ViewportKind {
        self.viewport_stack.last().copied().unwrap()
    }

    fn current_canvas(&mut self) -> &mut PackedFramebuf<C> {
        &mut self.canvas
    }

    /// Obtain the raw framebuffer data for hardware output.
    pub fn draw_buffer(&self, f: impl FnOnce(&[<C as PackedColor>::Storage])) {
        self.canvas.draw_buffer(f);
    }

    // Note: Real alpha channel is not supported. Alpha is currently just a
    // blend parameter applied while drawing onto the (opaque) framebuffer — it
    // affects blending against existing pixels, not surface transparency.
    // TODO: Real alpha-channel
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
        let canvas = &mut self.canvas;
        let eg_pixels = pixels
            .into_iter()
            .map(|p| embedded_graphics::prelude::Pixel(p.0.into(), p.1));
        match viewport {
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
        self.canvas.output(target)
    }

    /// WS6.3: flush only `regions` (each clamped to the viewport) to `target`.
    fn renderer_output_regions<TC>(
        &self,
        target: &mut impl RenderTarget<Color = TC>,
        regions: &[Rect],
    ) where
        C: MapColor<TC>,
    {
        for &region in regions {
            self.canvas.output_region(target, region);
        }
    }

    fn renderer_clipped(
        &mut self,
        area: Rect,
        f: impl FnOnce(&mut Self) -> RenderResult,
    ) -> RenderResult {
        self.viewport_stack.push(ViewportKind::Clipped(area));
        let result = f(self);
        self.viewport_stack.pop();
        result
    }

    fn renderer_image<'a>(&mut self, image: DrawImage<'a, C>) -> RenderResult {
        embedded_graphics::image::Image::new(
            image.image(),
            image.position().into(),
        )
        .draw(self)?;
        Ok(())
    }
}

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

    /// WS6.3b: route a solid rect fill to the framebuffer's fast `fill_solid`
    /// (whole-word writes) instead of the default fan-out to per-pixel
    /// `draw_iter`. Without this, `EGRenderer::fill_solid` → styled `Rectangle` →
    /// this DrawTarget's default `fill_solid` → `draw_iter`, and the framebuffer
    /// fast path is never reached. Mirrors `draw_pixels`' viewport dispatch; the
    /// eg `clipped`/`cropped` adapters forward `fill_solid` to the canvas (with
    /// clip / translation), so those paths stay correct and also get the speedup.
    fn fill_solid(
        &mut self,
        area: &embedded_graphics::primitives::Rectangle,
        color: Self::Color,
    ) -> Result<(), Self::Error> {
        let viewport = self.current_viewport();
        let canvas = &mut self.canvas;
        match viewport {
            ViewportKind::Fullscreen => canvas.fill_solid(area, color),
            ViewportKind::Clipped(clip) => {
                canvas.clipped(&clip.into()).fill_solid(area, color)
            },
            ViewportKind::Cropped(crop) => {
                canvas.cropped(&crop.into()).fill_solid(area, color)
            },
        }
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

    fn finish_frame_regions(
        &mut self,
        target: &mut impl RenderTarget<Color = C>,
        regions: &[Rect],
    ) {
        self.renderer_output_regions(target, regions);
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

    fn pixel(&mut self, point: Point, color: Self::Color) -> RenderResult {
        embedded_graphics::Pixel(point.into(), color).draw(self)
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
        eg_rect
            .draw_styled(&style.into_primitive_style(), self)
            .ok()
            .unwrap();
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
        _points: &[Point],
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // TODO: I don't want to allocate a vector for conversion between my
        // Point and EG Point, so better use custom primitive Polygon and
        // implement AA and non-AA rendering for it.
        //
        // TODO(unimplemented): polygon rendering for the embedded-graphics
        // backend. Skip (logged) instead of `todo!()` so drawing a polygon
        // degrades to nothing rather than aborting the device.
        log::warn!(
            "polygon() is not implemented for the embedded-graphics renderer; \
             skipping"
        );
        Ok(())
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

    fn image<'a>(&mut self, image: DrawImage<'a, Self::Color>) -> RenderResult {
        self.renderer_image(image)
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

    fn pixel(&mut self, point: Point, color: Self::Color) -> RenderResult {
        embedded_graphics::Pixel(point.into(), color).draw(self)
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
        eg_rect
            .draw_styled(&style.into_primitive_style(), self)
            .ok()
            .unwrap();
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
        _points: &[Point],
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // TODO: I don't want to allocate a vector for conversion between my
        // Point and EG Point, so better use custom primitive Polygon and
        // implement AA and non-AA rendering for it.
        //
        // TODO(unimplemented): polygon rendering for the embedded-graphics
        // backend. Skip (logged) instead of `todo!()` so drawing a polygon
        // degrades to nothing rather than aborting the device.
        log::warn!(
            "polygon() is not implemented for the embedded-graphics renderer; \
             skipping"
        );
        Ok(())
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

    fn image<'a>(&mut self, image: DrawImage<'a, Self::Color>) -> RenderResult {
        self.renderer_image(image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        geometry::{Point, Rect, Size},
        renderer::Renderer,
    };
    use embedded_graphics::pixelcolor::Rgb888;

    /// WS6.3b: EGRenderer's fast `fill_solid` (routing to the framebuffer's
    /// whole-word writes) must land the SAME pixels as the per-pixel path —
    /// proving the DrawTarget override + viewport dispatch forward correctly,
    /// not just the framebuffer method in isolation.
    #[test]
    fn eg_renderer_fill_solid_matches_per_pixel() {
        let size = Size::new(20, 16);
        let rect = Rect::new(Point::new(3, 2), Size::new(9, 7));
        let color = Rgb888::new(10, 200, 30);

        let mut fast = EGRenderer::<Rgb888, AntiAliasingDisabled>::new(size);
        Renderer::fill_solid(&mut fast, rect, color).unwrap();

        // Reference: fill the same rect one pixel at a time (the draw_iter path).
        let mut slow = EGRenderer::<Rgb888, AntiAliasingDisabled>::new(size);
        for p in rect.points() {
            Renderer::pixel(&mut slow, p, color).unwrap();
        }

        fast.draw_buffer(|f| {
            slow.draw_buffer(|s| {
                assert_eq!(f, s, "EGRenderer fill_solid != per-pixel fill");
            })
        });
    }
}
