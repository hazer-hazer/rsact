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
        Renderer, Viewport, ViewportKind,
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
    // 9a.2: sorted `Vec` keyed by layer index instead of a `BTreeMap`
    // (dynamic layering disabled → N == 1; kept sorted for compositing order).
    layers: Vec<(usize, Layer<C>)>,
    main_viewport: Size,
    aa: PhantomData<AA>,
}

impl<C: Color + PackedColor> EGRenderer<C, AntiAliasingDisabled> {
    pub fn new(viewport: Size) -> Self {
        Self {
            viewport_stack: vec![Viewport::root()],
            layers: vec![(0, Layer::fullscreen(viewport))],
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
        let pos = self
            .layers
            .binary_search_by_key(&layer_index, |(k, _)| *k)
            .unwrap();
        &mut self.layers[pos].1.canvas
    }

    /// Obtain the raw framebuffer data from layer 0 for hardware output.
    pub fn draw_buffer(&self, f: impl FnOnce(&[<C as PackedColor>::Storage])) {
        let pos = self.layers.binary_search_by_key(&0, |(k, _)| *k).unwrap();
        self.layers[pos].1.canvas.draw_buffer(f);
    }

    /// Map a point from the active viewport's coordinate space into the layer
    /// canvas's own space — the transform the *write* paths ([`draw_pixels`],
    /// `fill_solid`) get for free by dispatching through embedded-graphics'
    /// `DrawTargetExt`. Any path that touches the canvas **directly** must apply
    /// it by hand or it addresses a different pixel than the matching write.
    ///
    /// [`ViewportKind::Fullscreen`] is the identity, and [`ViewportKind::Clipped`]
    /// is too — eg's `clipped` only *filters* pixels outside the area and never
    /// rebases the origin. [`ViewportKind::Cropped`] does rebase (eg's `cropped`
    /// puts the origin at `area.top_left`).
    ///
    /// [`draw_pixels`]: Self::draw_pixels
    fn viewport_to_canvas(&self, point: Point) -> Point {
        match self.current_viewport().kind {
            ViewportKind::Fullscreen | ViewportKind::Clipped(_) => point,
            ViewportKind::Cropped(area) => point + area.top_left,
        }
    }

    /// Blend `pixel`'s colour into whatever the canvas already holds there.
    ///
    /// WS6.4.0(i-1): the read goes through [`viewport_to_canvas`] so it lands on
    /// the pixel `draw_pixels` will write. It previously read `pixel.0` raw,
    /// which is only correct while the viewport is `Fullscreen`/`Clipped` — under
    /// `Cropped` the write is rebased and the read was not, so the blend mixed
    /// against an unrelated pixel. Latent today (nothing constructs a `Cropped`
    /// viewport — see the commented-out producer at `layer.rs:88`), but painting
    /// into a tile *is* a rebased coordinate space, so 6.4d would have activated
    /// it. Note this is a read-modify-write per pixel: it defeats
    /// write-combining, and it is why a tile buffer must be pre-filled with the
    /// true background before painting (roadmap 6.4 constraint (b)).
    ///
    /// [`viewport_to_canvas`]: Self::viewport_to_canvas
    pub fn pixel_alpha(&mut self, pixel: Pixel<C>, blend: f32) -> RenderResult {
        let read_at = self.viewport_to_canvas(pixel.0);
        let canvas = self.current_canvas();
        // NOTE: an out-of-bounds read still degrades to the unblended colour
        // rather than an error, so a mis-addressed read yields a *plausible*
        // pixel, not a failure. Preserved as-is (a behaviour change is out of
        // scope here); it is why 6.4a's tile-invariance op-log check is the real
        // defence for this area.
        let color = canvas
            .pixel(read_at)
            .map(|current| current.mix(blend, pixel.1))
            .unwrap_or(pixel.1);
        self.draw_pixels(core::iter::once(Pixel(pixel.0, color)))
    }

    pub fn draw_pixels(
        &mut self,
        pixels: impl IntoIterator<Item = Pixel<C>>,
    ) -> Result<(), ()> {
        let viewport = self.current_viewport();
        let pos = self
            .layers
            .binary_search_by_key(&viewport.layer, |(k, _)| *k)
            .unwrap();
        let canvas = &mut self.layers[pos].1.canvas;
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
        self.layers
            .iter()
            .for_each(|(_, layer)| layer.canvas.output(target))
    }

    /// WS6.3: flush only `regions` (each clamped to the viewport) across all
    /// layers. Layer order is preserved (a region is streamed layer-by-layer,
    /// same as the full flush), so overlapping upper layers still land last.
    fn renderer_output_regions<TC>(
        &self,
        target: &mut impl RenderTarget<Color = TC>,
        regions: &[Rect],
    ) where
        C: MapColor<TC>,
    {
        self.layers.iter().for_each(|(_, layer)| {
            for &region in regions {
                layer.canvas.output_region(target, region)
            }
        })
    }

    fn renderer_push_clip(&mut self, area: Rect) {
        self.viewport_stack
            .push(self.sub_viewport(ViewportKind::Clipped(area)));
    }

    // Never pops the root viewport: an unbalanced `pop_clip` must degrade, not
    // leave the renderer with no viewport at all (`current_viewport` unwraps).
    fn renderer_pop_clip(&mut self) {
        if self.viewport_stack.len() > 1 {
            self.viewport_stack.pop();
        }
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
        let pos = self
            .layers
            .binary_search_by_key(&viewport.layer, |(k, _)| *k)
            .unwrap();
        let canvas = &mut self.layers[pos].1.canvas;
        match viewport.kind {
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

    fn push_clip(&mut self, area: Rect) {
        self.renderer_push_clip(area)
    }

    fn pop_clip(&mut self) {
        self.renderer_pop_clip()
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

    fn push_clip(&mut self, area: Rect) {
        self.renderer_push_clip(area)
    }

    fn pop_clip(&mut self) {
        self.renderer_pop_clip()
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

    /// WS6.4.0(ii-1): the clip stack must balance, and an unmatched `pop_clip`
    /// must degrade rather than pop the root viewport — `current_viewport()`
    /// unwraps the top of the stack, so emptying it would turn a caller's
    /// bookkeeping slip into a panic on the render path (WS1.8: the UI logs and
    /// degrades, it does not abort).
    #[test]
    fn clip_stack_balances_and_never_pops_the_root() {
        let mut r =
            EGRenderer::<Rgb888, AntiAliasingDisabled>::new(Size::new(20, 16));
        let root = r.viewport_stack.len();
        assert_eq!(root, 1, "a fresh renderer holds exactly the root viewport");

        r.push_clip(Rect::new(Point::new(2, 2), Size::new(8, 8)));
        assert_eq!(r.viewport_stack.len(), root + 1);
        r.push_clip(Rect::new(Point::new(3, 3), Size::new(4, 4)));
        assert_eq!(r.viewport_stack.len(), root + 2);

        r.pop_clip();
        r.pop_clip();
        assert_eq!(r.viewport_stack.len(), root, "push/pop must balance");

        // Unmatched pop: no panic, no lost root.
        r.pop_clip();
        r.pop_clip();
        assert_eq!(r.viewport_stack.len(), root, "root viewport must survive");
        // Still usable afterwards — the real point of not emptying the stack.
        Renderer::pixel(&mut r, Point::new(1, 1), Rgb888::WHITE).unwrap();
    }

    /// WS6.4.0(i-1): `pixel_alpha` must read the destination through the SAME
    /// viewport transform its write goes through. Under `ViewportKind::Cropped`
    /// the write is rebased to the crop origin (eg's `cropped`) while the read
    /// was raw, so the blend mixed against a different pixel than it wrote.
    ///
    /// Expressed as an invariance: `Cropped(crop)` + a viewport-local point must
    /// produce the same framebuffer as `Fullscreen` + the absolute point. That
    /// equivalence is exactly what painting into a tile relies on, which is why
    /// this latent bug would have gone live with 6.4d.
    #[test]
    fn pixel_alpha_reads_through_the_viewport_transform() {
        let size = Size::new(20, 16);
        let crop = Rect::new(Point::new(5, 4), Size::new(10, 8));
        let local = Point::new(2, 3);
        let abs = local + crop.top_left;

        // The backdrop must differ from the cleared background, or reading the
        // wrong pixel would coincidentally produce the right colour.
        let backdrop = Rgb888::new(200, 0, 0);
        let ink = Rgb888::new(0, 0, 200);
        assert_ne!(backdrop, <Rgb888 as Color>::default_background());

        // Cropped: seed the backdrop at the ABSOLUTE pixel, blend at the LOCAL
        // point. Pre-fix, the read landed on `local` (still background).
        let mut cropped = EGRenderer::<Rgb888, AntiAliasingDisabled>::new(size);
        Renderer::pixel(&mut cropped, abs, backdrop).unwrap();
        cropped
            .viewport_stack
            .push(Viewport { layer: 0, kind: ViewportKind::Cropped(crop) });
        cropped.pixel_alpha(Pixel(local, ink), 0.5).unwrap();
        cropped.viewport_stack.pop();

        // Reference: the same blend written in absolute coordinates.
        let mut absolute =
            EGRenderer::<Rgb888, AntiAliasingDisabled>::new(size);
        Renderer::pixel(&mut absolute, abs, backdrop).unwrap();
        absolute.pixel_alpha(Pixel(abs, ink), 0.5).unwrap();

        cropped.draw_buffer(|c| {
            absolute.draw_buffer(|a| {
                assert_eq!(
                    c, a,
                    "pixel_alpha blended against the untranslated destination"
                );
            })
        });
    }
}
