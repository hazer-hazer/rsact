use crate::{
    color::{Color, RgbColor},
    eg::{
        framebuf::{
            Framebuf as _, PackedColor, PackedFramebuf, PixelBuf, Surface,
        },
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
        // WS6.4b(ii), the cheap half: drop pixels the renderer would reject
        // anyway, BEFORE paying `Renderer::pixel` for each one.
        //
        // This is the only path text takes — `embedded-text` / u8g2 rasterise
        // glyphs and hand them here one pixel at a time — and it is where the
        // per-pixel cost that survives WS6.4b's part-level cull lives: a label
        // straddling a region boundary is not culled in either region, so every
        // glyph pixel is offered twice and each one pays a colour conversion plus
        // a viewport dispatch into the framebuffer to be discarded.
        //
        // It is a cheaper *write filter*, not the loop bound (ii) ultimately
        // wants: the glyph iteration upstream still runs, because the line/glyph
        // loop belongs to `embedded-text`, not to us. Owning that loop — which is
        // also what `font/fixed.rs`'s `Clip`/`Ellipsis` TODO needs — is what makes
        // the first/last-visible-glyph arithmetic possible, and it is filed as the
        // remaining part of (ii).
        //
        // Read once, outside the loop: `clip_bounds` borrows the renderer
        // immutably and `pixel` needs it mutably.
        let clip = self.renderer.clip_bounds();
        pixels
            .into_iter()
            .filter(|p| clip.is_none_or(|clip| clip.contains(Point::from(p.0))))
            .try_for_each(|p| {
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
/// `B` is the **user's** surface. The renderer borrows it: it never allocates
/// one, and [`detach`](Self::detach) hands it back. That is what makes the
/// framebuffer the application's property while keeping `W::Renderer` free of a
/// lifetime parameter — `WidgetCtx: 'static` (`el/ctx.rs:5`) rules out a
/// `&'a mut [T]` field, so the loan is expressed by moving the buffer in and
/// out. On embedded that is exactly the shape a `StaticCell` tile pool wants:
/// `&'static mut Tile` values passed through channels.
///
/// `B` also carries the capacity ([`PixelBuf::UNITS`]), which is what
/// [`Renderer::SURFACE_UNITS`] reports and what `UI::start_frame` proves a frame
/// policy against at compile time.
pub struct EGRenderer<C: Color + PackedColor, AA: AntiAliasing, B: Surface<C>> {
    viewport_stack: Vec<ViewportKind>,
    /// `None` between a `detach` and the next `attach` — the window in which
    /// the owner holds their buffer (shipping a tile over SPI, say).
    canvas: Option<PackedFramebuf<C, B>>,
    main_viewport: Size,
    aa: PhantomData<AA>,
}

impl<C: Color + PackedColor, B: Surface<C>>
    EGRenderer<C, AntiAliasingDisabled, B>
{
    /// Full-frame: `buffer` covers the whole display.
    ///
    /// The buffer is the caller's — a `Box<[u16]>` on a host, a `[u16; N]` in
    /// flash-and-RAM on a device. Nothing here allocates.
    pub fn new(viewport: Size, buffer: B) -> Self {
        Self {
            viewport_stack: vec![ViewportKind::root()],
            canvas: Some(PackedFramebuf::new(viewport, buffer)),
            main_viewport: viewport,
            aa: PhantomData,
        }
    }

    /// **WS6.4d: tiled.** `buffer` is *smaller* than the display, and is
    /// re-aimed at each region by [`Renderer::begin_region`].
    ///
    /// The acceptance target, constructible: a 240×240 RGB565 frame needs 57600
    /// storage units (112.5 KiB), while `[u16; 5760]` is 11.25 KiB and paints
    /// the same frame. `main_viewport` stays the display's size — that is what
    /// rsact lays out and culls against; only the surface shrinks.
    ///
    /// Starts aimed at nothing; the first `begin_region` supplies a region.
    /// Pair it with a policy whose largest region fits — `Tiles<240, 24>` here
    /// — and `UI::start_frame` proves the fit at compile time from
    /// `B`'s own capacity.
    pub fn tiled(viewport: Size, buffer: B) -> Self {
        Self {
            viewport_stack: vec![ViewportKind::root()],
            canvas: Some(PackedFramebuf::tile(buffer)),
            main_viewport: viewport,
            aa: PhantomData,
        }
    }
}

impl<C: Color + PackedColor, AA: AntiAliasing, B: Surface<C>>
    EGRenderer<C, AA, B>
{
    /// Lend the renderer a surface, returning whatever it held.
    ///
    /// The other half of the ping-pong an embedded app wants: hand in the free
    /// tile, take back the painted one, ship it while the next is painted.
    pub fn attach(&mut self, buffer: B) -> Option<B> {
        let previous = self.canvas.take().map(PackedFramebuf::into_buffer);
        self.canvas = Some(PackedFramebuf::tile(buffer));
        previous
    }

    /// Take the surface back. Drawing while detached is a logged no-op, never a
    /// panic (WS1.8: the UI degrades rather than aborting the device).
    pub fn detach(&mut self) -> Option<B> {
        self.canvas.take().map(PackedFramebuf::into_buffer)
    }

    pub fn is_attached(&self) -> bool {
        self.canvas.is_some()
    }
}

impl<C: Color + PackedColor + PixelColor, AA: AntiAliasing, B: Surface<C>>
    EGRenderer<C, AA, B>
{
    fn current_viewport(&self) -> ViewportKind {
        self.viewport_stack.last().copied().unwrap()
    }

    /// The lent surface, or `None` while its owner holds it.
    ///
    /// Every drawing path goes through this and degrades to a logged no-op when
    /// detached (WS1.8: the UI logs and continues; a panic in a render loop that
    /// runs every frame is not recoverable). Detached drawing is a *scheduling*
    /// mistake — painting between `detach` and `attach` — not a broken frame,
    /// and the next attached frame repaints anyway.
    fn current_canvas(&mut self) -> Option<&mut PackedFramebuf<C, B>> {
        if self.canvas.is_none() {
            log::warn!(
                "drawing with no surface attached — the frame is discarded. \
                 Attach a buffer before painting, or paint before detaching."
            );
        }
        self.canvas.as_mut()
    }

    /// Obtain the raw framebuffer data for hardware output. No-op if detached.
    pub fn draw_buffer(&self, f: impl FnOnce(&[<C as PackedColor>::Storage])) {
        if let Some(canvas) = self.canvas.as_ref() {
            canvas.draw_buffer(f);
        }
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
        match self.current_viewport() {
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
    /// viewport since PR #31 deleted the only, commented-out, producer), but painting
    /// into a tile *is* a rebased coordinate space, so 6.4d would have activated
    /// it. Note this is a read-modify-write per pixel: it defeats
    /// write-combining, and it is why a tile buffer must be pre-filled with the
    /// true background before painting (roadmap 6.4 constraint (b)).
    ///
    /// [`viewport_to_canvas`]: Self::viewport_to_canvas
    // Note: Real alpha channel is not supported. Alpha is currently just a
    // blend parameter applied while drawing onto the (opaque) framebuffer — it
    // affects blending against existing pixels, not surface transparency.
    // TODO: Real alpha-channel
    pub fn pixel_alpha(&mut self, pixel: Pixel<C>, blend: f32) -> RenderResult {
        let read_at = self.viewport_to_canvas(pixel.0);
        let canvas = self.current_canvas();
        // NOTE: an out-of-bounds read still degrades to the unblended colour
        // rather than an error, so a mis-addressed read yields a *plausible*
        // pixel, not a failure. Preserved as-is (a behaviour change is out of
        // scope here); it is why 6.4a's tile-invariance op-log check is the real
        // defence for this area.
        let Some(canvas) = canvas else { return Ok(()) };
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
        let Some(canvas) = self.current_canvas() else { return Ok(()) };
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
    /// WS6.4d: aim the surface at `region` and pre-fill it.
    ///
    /// Two things happen, and the second is not optional. The buffer is
    /// retargeted — `region`'s own width becomes the stride, so any region
    /// fitting the capacity is addressable — and then it is **filled with the
    /// background**, because a tile arrives holding whatever the previous region
    /// left in it.
    ///
    /// That fill is what makes anti-aliasing correct rather than merely tidy
    /// (roadmap 6.4 constraint (b)): `pixel_alpha` blends against the
    /// *destination*, and on a full framebuffer the destination survives between
    /// frames, which is how AA edges compose under damage-driven repaint. A tile
    /// has no such history, so without this the first AA edge in each region
    /// would blend against the previous region's pixels — a plausible image, not
    /// an obvious failure.
    ///
    /// A full-frame surface skips both: it already covers the region, and
    /// clearing it would erase the frame the damage-driven path relies on.
    fn renderer_begin_region(&mut self, region: Rect) -> RenderResult {
        let full_frame = crate::eg::framebuf::units_for::<C>(
            self.main_viewport.width,
            self.main_viewport.height,
        );
        let Some(canvas) = self.current_canvas() else { return Ok(()) };
        if canvas.capacity_units() >= full_frame {
            return Ok(());
        }
        canvas.retarget(region);
        // Straight at the canvas, not through `Renderer::fill_solid`: the
        // region clip is pushed by the caller *after* this returns, and the
        // whole retargeted buffer is what needs priming.
        DrawTarget::fill_solid(canvas, &region.into(), C::default_background())
    }

    fn renderer_output<TC>(&self, target: &mut impl RenderTarget<Color = TC>)
    where
        C: MapColor<TC>,
    {
        if let Some(canvas) = self.canvas.as_ref() {
            canvas.output(target)
        }
    }

    /// WS6.3: flush only `regions` (each clamped to the viewport) to `target`.
    fn renderer_output_regions<TC>(
        &self,
        target: &mut impl RenderTarget<Color = TC>,
        regions: &[Rect],
    ) where
        C: MapColor<TC>,
    {
        let Some(canvas) = self.canvas.as_ref() else { return };
        for &region in regions {
            canvas.output_region(target, region);
        }
    }

    // WS6.4b: narrowed by the active viewport so the top of the stack IS the
    // effective clip (`ViewportKind::nested_in` documents why that matters).
    // `EGRenderer` still keeps its viewport stack inline instead of using the
    // shared `surface::Canvas` helper — see this file's TODO — so the same
    // one-line composition lives in both places for now.
    fn renderer_push_clip(&mut self, area: Rect) {
        let nested =
            ViewportKind::Clipped(area).nested_in(self.current_viewport());
        self.viewport_stack.push(nested);
    }

    // `Fullscreen` falls back to the SURFACE rect rather than reporting "no
    // bound": that is what makes WS6.4b's culling pay on an ordinary full-frame
    // render, not only under tiles — off-screen content (scrolled-away rows) is
    // exactly the case where the clip is currently a write filter and the paint
    // happens anyway.
    fn renderer_clip_bounds(&self) -> Option<Rect> {
        Some(
            self.current_viewport()
                .clip_bounds()
                .unwrap_or(Rect::new(Point::zero(), self.main_viewport)),
        )
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

impl<C: Color + PackedColor + PixelColor, AA: AntiAliasing, B: Surface<C>>
    DrawTarget for EGRenderer<C, AA, B>
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
        let Some(canvas) = self.current_canvas() else { return Ok(()) };
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

impl<C: Color + PackedColor + PixelColor, AA: AntiAliasing, B: Surface<C>>
    Dimensions for EGRenderer<C, AA, B>
{
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        embedded_graphics::primitives::Rectangle::new(
            embedded_graphics::geometry::Point::zero(),
            self.main_viewport.into(),
        )
    }
}

// TODO: Other colors mapping
impl<C: Color + PackedColor + PixelColor, AA: AntiAliasing, B: Surface<C>>
    FinishRender<C> for EGRenderer<C, AA, B>
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

impl<C: Color + PackedColor + PixelColor, B: Surface<C>> Renderer
    for EGRenderer<C, AntiAliasingDisabled, B>
{
    type Color = C;

    /// The surface's capacity, from the type — WS6.4.0(iii)'s compile-time
    /// proof reads this. `usize::MAX` (a full-frame surface) accepts any policy,
    /// which is correct rather than merely permissive.
    const SURFACE_UNITS: usize = <B as PixelBuf<C>>::UNITS;

    /// How this surface packs pixels, straight from the colour. Leaving the
    /// default of 1 would over-state a 1-bpp surface's capacity eightfold.
    const SURFACE_PIXELS_PER_UNIT: usize = C::PPS;

    fn size(&self) -> Size {
        self.main_viewport
    }

    fn begin_region(&mut self, region: Rect) -> RenderResult {
        self.renderer_begin_region(region)
    }

    fn push_clip(&mut self, area: Rect) {
        self.renderer_push_clip(area)
    }

    fn pop_clip(&mut self) {
        self.renderer_pop_clip()
    }

    fn clip_bounds(&self) -> Option<Rect> {
        self.renderer_clip_bounds()
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

impl<C: Color + PackedColor + PixelColor, B: Surface<C>> Renderer
    for EGRenderer<C, AntiAliasingEnabled, B>
{
    type Color = C;

    /// The surface's capacity, from the type — WS6.4.0(iii)'s compile-time
    /// proof reads this. `usize::MAX` (a full-frame surface) accepts any policy,
    /// which is correct rather than merely permissive.
    const SURFACE_UNITS: usize = <B as PixelBuf<C>>::UNITS;

    /// How this surface packs pixels, straight from the colour. Leaving the
    /// default of 1 would over-state a 1-bpp surface's capacity eightfold.
    const SURFACE_PIXELS_PER_UNIT: usize = C::PPS;

    fn size(&self) -> Size {
        self.main_viewport
    }

    fn begin_region(&mut self, region: Rect) -> RenderResult {
        self.renderer_begin_region(region)
    }

    fn push_clip(&mut self, area: Rect) {
        self.renderer_push_clip(area)
    }

    fn pop_clip(&mut self) {
        self.renderer_pop_clip()
    }

    fn clip_bounds(&self) -> Option<Rect> {
        self.renderer_clip_bounds()
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
    use crate::eg::framebuf::{heap_surface, heap_surface_units};
    use crate::{
        geometry::{Point, Rect, Size},
        renderer::{NullColor, NullRenderer, Renderer},
    };
    use embedded_graphics::pixelcolor::{BinaryColor, Rgb888};

    /// WS6.3b: EGRenderer's fast `fill_solid` (routing to the framebuffer's
    /// whole-word writes) must land the SAME pixels as the per-pixel path —
    /// proving the DrawTarget override + viewport dispatch forward correctly,
    /// not just the framebuffer method in isolation.
    #[test]
    fn eg_renderer_fill_solid_matches_per_pixel() {
        let size = Size::new(20, 16);
        let rect = Rect::new(Point::new(3, 2), Size::new(9, 7));
        let color = Rgb888::new(10, 200, 30);

        let mut fast = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::new(
            size,
            heap_surface::<Rgb888>(size),
        );
        Renderer::fill_solid(&mut fast, rect, color).unwrap();

        // Reference: fill the same rect one pixel at a time (the draw_iter path).
        let mut slow = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::new(
            size,
            heap_surface::<Rgb888>(size),
        );
        for p in rect.points() {
            Renderer::pixel(&mut slow, p, color).unwrap();
        }

        fast.draw_buffer(|f| {
            slow.draw_buffer(|s| {
                assert_eq!(f, s, "EGRenderer fill_solid != per-pixel fill");
            })
        });
    }

    /// **WS6.4d: the acceptance test for tiled rendering.** A surface a
    /// fraction of the frame's size must produce the *same pixels* as a full
    /// framebuffer.
    ///
    /// This is what 6.4 exists for, stated as an equality: 240×240 RGB565 is
    /// 112.5 KiB and does not fit a Blue Pill, while a 240×24 tile is 11.25 KiB
    /// and does. The whole design is only worth anything if the two agree
    /// exactly, and "agree" has to mean pixels — WS6.4a's op-log invariance
    /// checks that each region *issues* the right draw calls, which is a
    /// different claim and cannot see an addressing mistake. Getting the origin
    /// sign wrong, or the stride, produces a plausible image and an intact op
    /// log.
    ///
    /// Both paths flush through `FinishRender` into the same kind of pixel map,
    /// so what is compared is what would reach the panel.
    #[test]
    fn a_tiled_surface_paints_the_same_pixels_as_a_full_one() {
        use crate::{
            output::{FinishRender, RenderTarget, pixel::Pixel},
            style::DrawStyle,
        };
        use alloc::vec;

        const W: u32 = 64;
        const H: u32 = 64;
        let viewport = Size::new(W, H);

        /// A full-frame pixel map, so the two paths are compared on what the
        /// display would actually receive.
        struct Map {
            px: alloc::vec::Vec<Option<Rgb888>>,
        }
        impl RenderTarget for Map {
            type Color = Rgb888;
            fn draw(
                &mut self,
                pixels: impl Iterator<Item = Pixel<Self::Color>>,
            ) {
                for Pixel(p, c) in pixels {
                    if p.x >= 0
                        && p.y >= 0
                        && (p.x as u32) < W
                        && (p.y as u32) < H
                    {
                        self.px[p.y as usize * W as usize + p.x as usize] =
                            Some(c);
                    }
                }
            }
        }
        let blank = || Map { px: vec![None; (W * H) as usize] };

        // Content chosen to cross region boundaries and to exercise both write
        // paths: `fill_solid`'s whole-word runs and the per-pixel fan-out.
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

        // Reference: one full-size surface, one pass, one flush.
        let mut full = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::new(
            viewport,
            heap_surface::<Rgb888>(viewport),
        );
        content(&mut full);
        let mut full_map = blank();
        full.finish_frame(&mut full_map);

        // Tiled: a 64x8 surface — 512 units against the frame's 4096, an eighth
        // — repainted and flushed region by region.
        const TILE_UNITS: usize = (W * 8) as usize; // Rgb888: one unit per pixel
        let tile_units = TILE_UNITS;
        let mut tiled = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::tiled(
            viewport,
            heap_surface_units::<Rgb888>(TILE_UNITS),
        );
        assert!(
            tile_units * 8 == (W * H) as usize,
            "the point of the test is that the surface is a FRACTION of the frame"
        );

        let mut tiled_map = blank();
        for band in 0..8 {
            let region = Rect::new(Point::new(0, band * 8), Size::new(W, 8));
            tiled.begin_region(region).unwrap();
            tiled.push_clip(region);
            content(&mut tiled);
            tiled.pop_clip();
            tiled.end_region().unwrap();
            tiled.finish_frame_regions(&mut tiled_map, &[region]);
        }

        // Not vacuous: both paths must have painted a substantial frame. An
        // all-`None` comparison passes trivially, and this test's whole value
        // is that it would catch an addressing bug — which is also the kind of
        // bug that could leave a map empty.
        let painted = |m: &Map| m.px.iter().filter(|p| p.is_some()).count();
        assert!(
            painted(&full_map) > (W * H) as usize / 3,
            "the reference frame painted only {} of {} pixels",
            painted(&full_map),
            W * H
        );
        assert_eq!(
            painted(&full_map),
            painted(&tiled_map),
            "the two paths painted different numbers of pixels"
        );

        let mismatches: alloc::vec::Vec<usize> = (0..(W * H) as usize)
            .filter(|&i| full_map.px[i] != tiled_map.px[i])
            .collect();
        assert!(
            mismatches.is_empty(),
            "{} of {} pixels differ between a full framebuffer and a tiled \
             one; first at ({}, {}): full {:?} vs tiled {:?}",
            mismatches.len(),
            W * H,
            mismatches[0] % W as usize,
            mismatches[0] / W as usize,
            full_map.px[mismatches[0]],
            tiled_map.px[mismatches[0]],
        );
    }

    /// WS6.4d: the renderer **borrows** its surface — it never allocates one and
    /// always gives it back.
    ///
    /// This is the ownership rule stated as a test. On a device the buffer lives
    /// in the application's `StaticCell` pool and moves through channels; rsact
    /// holds it only for as long as it is painting. `WidgetCtx: 'static`
    /// (`el/ctx.rs:5`) rules out expressing that as a `&'a mut [T]` field, so
    /// the loan is a move in and a move out — which is also exactly the shape
    /// DMA wants, since a borrow the core could still write through is UB
    /// (roadmap 6.7).
    #[test]
    fn the_renderer_gives_the_surface_back() {
        let viewport = Size::new(16, 16);
        let mut r = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::new(
            viewport,
            heap_surface::<Rgb888>(viewport),
        );
        assert!(r.is_attached());

        // Paint something, then take the buffer back and inspect it — the owner
        // can read what was painted, which is what "ship this tile" means.
        let ink = Rgb888::new(9, 9, 9);
        Renderer::fill_solid(&mut r, Rect::new(Point::zero(), viewport), ink)
            .unwrap();
        let buffer = r.detach().expect("the surface was attached");
        assert_eq!(buffer.len(), (16 * 16) as usize);
        assert!(
            buffer.iter().all(|u| *u == ink.into_storage()),
            "the owner got back a buffer that does not hold what was painted"
        );
        assert!(!r.is_attached());

        // Painting while detached degrades: no panic, and nothing is lost that
        // the next attached frame will not repaint.
        Renderer::fill_solid(
            &mut r,
            Rect::new(Point::zero(), viewport),
            Rgb888::new(1, 2, 3),
        )
        .expect("drawing detached must not be an error");

        // Hand a different buffer in; the renderer takes it and reports the old
        // one (here: none, since we detached).
        assert!(r.attach(heap_surface::<Rgb888>(viewport)).is_none());
        assert!(r.is_attached());
        // ...and now a swap returns the buffer that was in place.
        let swapped = r
            .attach(heap_surface::<Rgb888>(viewport))
            .expect("attach over an attached surface returns the old one");
        assert_eq!(swapped.len(), (16 * 16) as usize);
    }

    /// WS6.4d: a tiled renderer must **declare** its capacity, so the
    /// compile-time frame-policy proof can see it.
    ///
    /// This closes a hole the tiled constructor opened. `Renderer::
    /// SURFACE_UNITS` defaults to `usize::MAX` — "my surface always covers the
    /// frame" — which was simply true of `EGRenderer` while every instance was
    /// a full framebuffer. The moment a 5760-unit surface could exist, that
    /// default became a lie, and `UI::start_frame::<Tiles<240, 240>>()` against
    /// it would have passed the const assert and failed at runtime on
    /// `retarget`'s `debug_assert` — in debug builds only, after the plan was
    /// already made.
    ///
    /// Hence the capacity is a const *parameter*: it reaches the trait, and the
    /// proof is against the buffer that actually exists.
    #[test]
    fn a_tiled_renderer_declares_its_capacity() {
        // Full-frame: unbounded, which is honest — it does cover the frame.
        assert_eq!(
            <EGRenderer<
                Rgb888,
                AntiAliasingDisabled,
                alloc::boxed::Box<[<Rgb888 as PackedColor>::Storage]>,
            > as Renderer>::SURFACE_UNITS,
            usize::MAX
        );
        // Tiled: exactly what was allocated.
        assert_eq!(
            <EGRenderer<Rgb888, AntiAliasingDisabled, [u32; 5760]> as Renderer>::SURFACE_UNITS,
            5760
        );

        // And the packing, which the capacity arithmetic needs. Leaving this at
        // the default of 1 would over-state a 1-bpp surface eightfold — a
        // policy needing 384 bytes would "fit" a 48-byte buffer.
        assert_eq!(
            <EGRenderer<
                Rgb888,
                AntiAliasingDisabled,
                alloc::boxed::Box<[<Rgb888 as PackedColor>::Storage]>,
            > as Renderer>::SURFACE_PIXELS_PER_UNIT,
            1
        );
        assert_eq!(
            <EGRenderer<
                BinaryColor,
                AntiAliasingDisabled,
                alloc::boxed::Box<[<BinaryColor as PackedColor>::Storage]>,
            > as Renderer>::SURFACE_PIXELS_PER_UNIT,
            8
        );
    }

    /// WS6.4.0(ii-4): `NullRenderer` must be a no-op renderer for the
    /// *application's* colour, not only for `NullColor`.
    ///
    /// This is what 6.4c's collect pass runs widget bodies against: it has to
    /// satisfy `Renderer<Color = W::Color>` while rasterising nothing, which the
    /// old `type Color = NullColor` hard-wiring could not express. Lives in this
    /// module because a second real `Color` impl (`Rgb888`) is in scope here.
    #[test]
    fn null_renderer_is_generic_over_colour() {
        fn accepts_renderer_for<C: Color, R: Renderer<Color = C>>(
            r: &mut R,
            c: C,
        ) {
            // Every primitive is a no-op that still reports success, so a
            // collect pass never sees a spurious `Err` from the null backend.
            Renderer::pixel(r, Point::zero(), c).unwrap();
            r.push_clip(Rect::new(Point::zero(), Size::new_equal(4)));
            r.pop_clip();
            // Defaulted in ii-3; correct as a no-op for a full-frame surface.
            r.begin_region(Rect::new(Point::zero(), Size::new_equal(4)))
                .unwrap();
            r.end_region().unwrap();
        }

        let mut app = NullRenderer::<Rgb888>::default();
        accepts_renderer_for(&mut app, Rgb888::WHITE);

        // The `C = NullColor` default keeps every existing `Wtf<NullRenderer, ..>`
        // and `&mut NullRenderer` spelling compiling unannotated.
        let mut legacy = NullRenderer::default();
        accepts_renderer_for(&mut legacy, NullColor);
    }

    /// WS6.4.0(ii-1): the clip stack must balance, and an unmatched `pop_clip`
    /// must degrade rather than pop the root viewport — `current_viewport()`
    /// unwraps the top of the stack, so emptying it would turn a caller's
    /// bookkeeping slip into a panic on the render path (WS1.8: the UI logs and
    /// degrades, it does not abort).
    #[test]
    fn clip_stack_balances_and_never_pops_the_root() {
        let mut r = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::new(
            Size::new(20, 16),
            heap_surface::<Rgb888>(Size::new(20, 16)),
        );
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

    /// WS6.4b(ii): the proxy must drop pixels outside the renderer's clip before
    /// paying `Renderer::pixel` for them, and must drop **only** those.
    ///
    /// This is the path all text takes (`embedded-text` / u8g2 hand glyphs over
    /// one pixel at a time), so it is where the per-pixel cost that survives the
    /// part-level cull lives — a label straddling a region boundary is culled in
    /// neither region. Asserted on op counts because the failure modes are
    /// symmetric and both silent: filter too little and tiling pays N× per-pixel
    /// work; filter too much and glyphs lose columns.
    #[test]
    fn the_proxy_filters_pixels_the_renderer_would_reject() {
        use crate::record::{DrawOp, RecordingRenderer};

        let mut rec = RecordingRenderer::<Rgb888>::new(Size::new(64, 64));
        rec.push_clip(Rect::new(Point::new(10, 10), Size::new(10, 10)));

        let ink = Rgb888::new(255, 255, 255);
        let at = |x, y| {
            embedded_graphics::prelude::Pixel(
                embedded_graphics::prelude::Point::new(x, y),
                ink,
            )
        };
        DrawTargetProxy::new(&mut rec)
            .draw_iter([
                at(15, 15), // inside
                at(19, 19), // inside, last pixel of the clip
                at(20, 20), // outside: the clip's edge is exclusive
                at(5, 5),   // outside
            ])
            .unwrap();

        let drawn: Vec<_> = rec
            .ops()
            .into_iter()
            .filter_map(|op| match op {
                DrawOp::Pixel(point) => Some(point),
                _ => None,
            })
            .collect();
        assert_eq!(drawn, [Point::new(15, 15), Point::new(19, 19)]);
    }

    /// WS6.4b: a nested clip must be **narrowed by** its parent, not replace it.
    ///
    /// Asserted where it actually matters — on the framebuffer, not on the stack:
    /// a pixel inside the inner clip but outside the outer one must not land. It
    /// used to, because `push_clip` stored the raw area and the write filter
    /// consults only the top of the stack, so an inner clip reaching beyond its
    /// parent *widened* the effective clip. Under WS6.4d that is drawing escaping
    /// its tile; here it is the precondition for `clip_bounds` being an exact cull
    /// rect rather than a guess.
    #[test]
    fn a_nested_clip_narrows_and_never_widens() {
        let mut r = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::new(
            Size::new(40, 40),
            heap_surface::<Rgb888>(Size::new(40, 40)),
        );

        r.push_clip(Rect::new(Point::new(0, 0), Size::new(20, 20)));
        // Overlaps the parent over (10,10)..(20,20) and reaches BEYOND it.
        r.push_clip(Rect::new(Point::new(10, 10), Size::new(20, 20)));

        assert_eq!(
            r.clip_bounds(),
            Some(Rect::new(Point::new(10, 10), Size::new(10, 10))),
            "the effective clip is the intersection, not the inner rect"
        );

        // The probe colour must DIFFER from the untouched framebuffer, or the
        // assertions below hold whatever the clip does: `default_background()`
        // for RGB is WHITE, so a white probe pixel proves nothing (this test was
        // written that way first and passed its "rejected" case vacuously).
        let bg = <Rgb888 as Color>::default_background();
        let ink = <Rgb888 as Color>::default_foreground();
        assert_ne!(ink, bg, "the probe colour must be visible");

        // Inside the inner rect but outside the parent: must be rejected.
        Renderer::pixel(&mut r, Point::new(25, 15), ink).unwrap();
        // Inside both: must land.
        Renderer::pixel(&mut r, Point::new(15, 15), ink).unwrap();

        assert_eq!(
            r.canvas.as_ref().unwrap().pixel(Point::new(25, 15)),
            Some(bg),
            "a write outside the PARENT clip escaped the nested clip"
        );
        assert_eq!(
            r.canvas.as_ref().unwrap().pixel(Point::new(15, 15)),
            Some(ink)
        );

        // Popping restores the parent, not the raw inner rect.
        r.pop_clip();
        assert_eq!(
            r.clip_bounds(),
            Some(Rect::new(Point::new(0, 0), Size::new(20, 20)))
        );
        r.pop_clip();
        assert_eq!(
            r.clip_bounds(),
            Some(Rect::new(Point::zero(), Size::new(40, 40))),
            "the root viewport reports the surface rect"
        );
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
        let mut cropped = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::new(
            size,
            heap_surface::<Rgb888>(size),
        );
        Renderer::pixel(&mut cropped, abs, backdrop).unwrap();
        // PR #31 collapsed `Viewport { layer, kind }` to a bare `ViewportKind`
        // when the layer dimension was deleted.
        cropped.viewport_stack.push(ViewportKind::Cropped(crop));
        cropped.pixel_alpha(Pixel(local, ink), 0.5).unwrap();
        cropped.viewport_stack.pop();

        // Reference: the same blend written in absolute coordinates.
        let mut absolute = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::new(
            size,
            heap_surface::<Rgb888>(size),
        );
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
