#[allow(unused)]
use crate::FloatExt as _;
use crate::renderer::{Attached, Attachment, Detached};
use crate::{
    prelude::{Angle, DrawStyle, Path, Point, Rect, RenderResult, Size, *},
    tiny_skia::path::PathBuilderExt,
};
use alloc::vec::Vec;
use core::marker::PhantomData;
use tiny_skia::{
    IntSize, Mask, Paint, PathBuilder, Pixmap, PixmapMut, PixmapPaint,
    PixmapRef, Transform,
};

pub mod color;
pub mod geometry;
pub mod path;

/// A renderer that draws into a [`Pixmap`] the **caller** owns.
///
/// The mirror of [`EGRenderer`]'s design, with one deliberate difference: the
/// surface is a `tiny_skia::Pixmap`, not an embedded-friendly colour buffer, and
/// that is the point rather than an oversight. A detached pixmap is
/// `encode_png`-able, which makes this the renderer for showcase renders and
/// golden images. Bringing it down to an embedded colour is a separate future
/// step (a `PixmapExt::map_to_framebuffer` over [`MapColor`]), not something
/// this type should do on the way out.
///
/// # The bound is a CAPACITY, exactly as for the framebuffer backend
///
/// A `Pixmap` has a fixed `width()` and indexes `y * width + x`, so it looks at
/// first like it must be bounded by *shape* — a 240×24 pixmap could not hold a
/// 16×38 region, even though both fit the same byte budget and the planner is
/// entitled to emit either under `Tiles<240, 24>`.
///
/// It is not, because a pixmap **can** be re-strided: `Pixmap::take()` yields
/// its `Vec<u8>` and `Pixmap::from_vec` rebuilds one at any size the vector's
/// length matches. So [`begin_region`] reshapes the storage to the region —
/// `take` · `resize` · `from_vec` — and since a shrink keeps the vector's
/// capacity and a regrow stays inside it, **no reallocation happens** as long as
/// the attached pixmap was big enough for the policy in the first place, which is
/// precisely what [`attach`](TinySkiaRenderer::attach) checks.
///
/// The consequence is that this backend takes a fixed pixmap pool under a
/// `Tiles<W, H>` policy on the same terms as [`EGRenderer`], and the two
/// backends differ only in what the storage *is*.
///
/// [`EGRenderer`]: crate::eg::renderer::EGRenderer
/// [`begin_region`]: Renderer::begin_region
pub struct TinySkiaRenderer<
    C,
    P = crate::region::Unbounded,
    A: Attachment<Vec<u8>> = Attached,
> {
    /// The lent pixmap's **bytes** — and only in the [`Attached`] state, where
    /// the type is `Vec<u8>`. In [`Detached`] it is `()`: no field, nothing to
    /// unwrap, and no "drawing with nothing attached" case to handle.
    ///
    /// Bytes rather than a `Pixmap` because a `Pixmap` cannot be re-strided in
    /// place — reshaping it to each region means owning the vector. The caller
    /// still hands in and gets back a real `Pixmap`; this is the form it is held
    /// in while lent. `capacity` is what bounds reshaping, and is checked once,
    /// at `attach`.
    pixels: A::Slot,
    /// Bytes the attached vector was allocated with — the reshaping ceiling.
    ///
    /// `Vec::capacity` is not a contract (it may exceed what was asked for), so
    /// the figure the policy was checked against is remembered explicitly rather
    /// than re-read from the vector.
    capacity: usize,
    /// The region the storage is currently shaped for.
    region: Size,
    /// Where the attached pixmap's `(0, 0)` sits in absolute frame coordinates.
    ///
    /// rsact paints in absolute coordinates (WS6.4.0(ii-3)); rebasing them into
    /// a surface smaller than the frame is the renderer's private business. For
    /// tiny-skia that rebase is a `Transform`, applied to every draw call, plus
    /// a subtraction on the two paths that index pixels directly.
    origin: Point,
    /// The display's size — what rsact lays out and culls against, unchanged by
    /// how small the attached pixmap is.
    size: Size,
    viewport_stack: Vec<ViewportKind>,
    /// WS6.11: the active clip, as a tiny-skia [`Mask`].
    ///
    /// tiny-skia has no scissor rect — every draw call takes an
    /// `Option<&Mask>`, so a clip has to be an actual alpha mask. Rebuilt only
    /// when the clip stack changes (a `Mask` is `w*h` bytes, which is why it is
    /// cached rather than constructed per primitive), and `None` while the
    /// viewport is `Fullscreen`, which is both cheaper and the common case.
    clip_mask: Option<Mask>,
    _color: PhantomData<C>,
    _policy: PhantomData<P>,
}

impl TinySkiaRenderer<tiny_skia::Color, crate::region::Unbounded> {
    /// `size` is the display's; `pixmap` is the caller's surface.
    ///
    /// **Nothing here allocates.** A pixmap covering the whole frame gives the
    /// classic full-framebuffer behaviour; a smaller one is painted a region at
    /// a time, with the caller sizing it from `Frame::peek_region`. Either way
    /// the policy is [`Unbounded`](crate::region::Unbounded), which is what lets
    /// this constructor need no annotation — and is honest, since a pixmap the
    /// caller sizes per region has no *fixed* bound to declare.
    ///
    /// [`tiled`](TinySkiaRenderer::tiled) is the constructor for a pixmap
    /// reused across regions under a declared policy.
    pub fn new(size: Size, pixmap: Pixmap) -> Self {
        TinySkiaRenderer::<tiny_skia::Color, _, Detached>::parked(size)
            .attach(pixmap)
    }
}

impl<P: crate::region::FramePolicy> TinySkiaRenderer<tiny_skia::Color, P> {
    /// A pixmap reused across regions, bounded by policy `P` — a **capacity**
    /// check at [`attach`](TinySkiaRenderer::attach), since the storage is
    /// reshaped per region (see the type docs).
    pub fn tiled(size: Size, pixmap: Pixmap) -> Self {
        TinySkiaRenderer::<tiny_skia::Color, P, Detached>::parked(size)
            .attach(pixmap)
    }

    /// Take the pixmap back, with the region that was painted into it.
    ///
    /// Consumes the renderer and returns it [`Detached`]: the state with no
    /// pixmap field, so nothing can paint into a surface the caller is holding.
    ///
    /// The rect comes from the renderer because `begin_region` told it where it
    /// was painting — re-pairing a surface with a rect by hand produces a
    /// plausible image rather than an obvious failure.
    pub fn detach(
        self,
    ) -> (TinySkiaRenderer<tiny_skia::Color, P, Detached>, Pixmap, Rect) {
        let at = Rect::new(self.origin, self.region);
        let parked = TinySkiaRenderer {
            pixels: (),
            capacity: self.capacity,
            region: self.region,
            origin: self.origin,
            size: self.size,
            viewport_stack: self.viewport_stack,
            clip_mask: self.clip_mask,
            _color: PhantomData,
            _policy: PhantomData,
        };
        // Sized to the region actually painted, so what the caller receives is
        // exactly the tile — `encode_png`-able as-is. The vector's *capacity*
        // survives the truncation, which is what makes reattaching it free.
        let mut pixels = self.pixels;
        pixels.truncate(Self::bytes_for(self.region));
        let pixmap = Pixmap::from_vec(
            pixels,
            IntSize::from_wh(self.region.width, self.region.height)
                .expect("a painted region is never zero-sized"),
        )
        .expect("length was just set to match the region");
        (parked, pixmap, at)
    }

    /// Exchange pixmaps in place, returning the painted one and its rect.
    ///
    /// Sugar over detach + attach, and — unlike them — it keeps `&mut self`,
    /// because the renderer is never observably without a surface. Not the
    /// primitive: see `EGRenderer::detach` for why a single-buffer pool
    /// deadlocks under swap-only.
    pub fn swap(&mut self, next: Pixmap) -> (Pixmap, Rect) {
        let capacity = Self::check_capacity(&next);
        let at = Rect::new(self.origin, self.region);
        let region = self.region;

        let mut taken = core::mem::replace(&mut self.pixels, next.take());
        self.capacity = capacity;
        self.region = Size::new(0, 0);
        taken.truncate(Self::bytes_for(region));
        let pixmap = Pixmap::from_vec(
            taken,
            IntSize::from_wh(region.width, region.height)
                .expect("a painted region is never zero-sized"),
        )
        .expect("length was just set to match the region");
        (pixmap, at)
    }

    /// Bytes a `size` region of premultiplied RGBA occupies.
    fn bytes_for(size: Size) -> usize {
        size.width as usize * size.height as usize * 4
    }

    /// The attached pixmap must hold policy `P`'s largest region — a **byte
    /// budget**, because the storage is reshaped per region rather than used at
    /// a fixed stride. Returns the capacity to remember.
    ///
    /// # Panics
    ///
    /// If it does not. Same reasoning as `EGRenderer`: a static property of the
    /// application's memory plan, discovered at the hand-off, with no degraded
    /// mode worth having.
    fn check_capacity(pixmap: &Pixmap) -> usize {
        let have = Self::bytes_for(Size::new(pixmap.width(), pixmap.height()));
        if let Some(max) = <P as crate::region::FramePolicy>::MAX_REGION {
            let needed = Self::bytes_for(max);
            assert!(
                have >= needed,
                "[rsact] pixmap {}x{} holds {have} bytes; this renderer's \
                 frame policy needs {needed} for its largest region ({}x{})",
                pixmap.width(),
                pixmap.height(),
                max.width,
                max.height,
            );
        }
        have
    }

    fn painted_size(&self) -> Size {
        self.region
    }

    /// The attached storage as a drawable pixmap over the current region.
    ///
    /// A `PixmapMut` view rather than an owned `Pixmap`, which is what lets one
    /// allocation serve every region shape: the view's stride is the region's
    /// own width, so nothing is re-addressed by hand.
    fn canvas(&mut self) -> PixmapMut<'_> {
        let (w, h) = (self.region.width, self.region.height);
        let bytes = Self::bytes_for(self.region);
        PixmapMut::from_bytes(&mut self.pixels[..bytes], w, h)
            .expect("the region was validated when it was set")
    }

    /// The canvas **and** the active clip, which every draw call needs together.
    ///
    /// They are one accessor because `PixmapMut` borrows `self.pixels` mutably
    /// while the `Mask` lives in another field: taking them separately is a
    /// borrow conflict, and taking the mask *out* and putting it back (the
    /// obvious workaround) is both noisy and easy to forget half of.
    fn canvas_and_clip(&mut self) -> (PixmapMut<'_>, Option<&Mask>) {
        let (w, h) = (self.region.width, self.region.height);
        let bytes = Self::bytes_for(self.region);
        let Self { pixels, clip_mask, .. } = self;
        let canvas = PixmapMut::from_bytes(&mut pixels[..bytes], w, h)
            .expect("the region was validated when it was set");
        (canvas, clip_mask.as_ref())
    }
}

impl<P: crate::region::FramePolicy>
    TinySkiaRenderer<tiny_skia::Color, P, Detached>
{
    /// A renderer with no pixmap yet — the state one is attached *to*.
    ///
    /// Useful on its own: an app whose surfaces arrive from a channel can build
    /// the renderer at boot and wait for the first one, which the `Option`-based
    /// shape could express only as "constructed but secretly broken".
    pub fn parked(size: Size) -> Self {
        Self {
            pixels: (),
            capacity: 0,
            region: Size::new(0, 0),
            origin: Point::zero(),
            size,
            viewport_stack: vec![ViewportKind::root()],
            clip_mask: None,
            _color: PhantomData,
            _policy: PhantomData,
        }
    }

    /// Lend the renderer a pixmap. Consumes the parked renderer and returns an
    /// [`Attached`] one — the only state that can draw.
    ///
    /// # Panics
    ///
    /// If `pixmap` is smaller than policy `P`'s largest region in either
    /// dimension — a **shape** check, not a capacity one (see the type docs).
    pub fn attach(
        self,
        pixmap: Pixmap,
    ) -> TinySkiaRenderer<tiny_skia::Color, P, Attached> {
        type Attach<P> = TinySkiaRenderer<tiny_skia::Color, P, Attached>;
        let capacity = Attach::<P>::check_capacity(&pixmap);
        let region = Size::new(pixmap.width(), pixmap.height());
        TinySkiaRenderer {
            pixels: pixmap.take(),
            capacity,
            region,
            origin: self.origin,
            size: self.size,
            viewport_stack: self.viewport_stack,
            clip_mask: self.clip_mask,
            _color: PhantomData,
            _policy: PhantomData,
        }
    }
}

impl<P: crate::region::FramePolicy> TinySkiaRenderer<tiny_skia::Color, P> {
    /// The transform that carries absolute frame coordinates into the attached
    /// pixmap's own space.
    ///
    /// Identity while the pixmap covers the frame, which is why a full-frame
    /// caller pays nothing for tiling existing.
    fn base_transform(&self) -> Transform {
        Transform::from_translate(-self.origin.x as f32, -self.origin.y as f32)
    }

    fn current_viewport(&self) -> ViewportKind {
        *self.viewport_stack.last().unwrap()
    }

    /// Rebuild [`clip_mask`] from the composed viewport (WS6.11).
    ///
    /// Called only from `push_clip`/`pop_clip`, so the per-primitive cost is a
    /// null check. `enter_viewport` already intersects nested clips
    /// (`ViewportKind::nested_in`), so the rect here is the *composed* one —
    /// the same rect `clip_bounds()` reports, which is what keeps the culling
    /// contract and the actual clipping in agreement.
    fn rebuild_clip_mask(&mut self) {
        // WS6.4d: the mask is the size of the **pixmap**, not the frame, and the
        // clip rect is rebased by the origin like every other coordinate. A
        // frame-sized mask over a tile-sized pixmap would both over-allocate and
        // mis-address, letting drawing escape the region — the exact failure
        // `ViewportKind::nested_in` exists to prevent one level up.
        let size = self.painted_size();
        let transform = self.base_transform();
        self.clip_mask = match self.current_viewport().clip_bounds() {
            None => None,
            Some(area) => {
                let mut mask = Mask::new(size.width, size.height)
                    .expect("clip mask allocation failed");
                let mut path = PathBuilder::new();
                path.push_rect(area.into());
                if let Some(path) = path.finish() {
                    mask.fill_path(
                        &path,
                        tiny_skia::FillRule::default(),
                        // No AA: a clip edge is a hard boundary. Anti-aliasing
                        // it would leak half-covered pixels outside the rect,
                        // which is exactly what a clip must not do.
                        false,
                        transform,
                    );
                }
                Some(mask)
            },
        };
    }

    fn bounding_box(&self) -> Rect {
        Rect::new(Point::zero(), self.size)
    }

    // TODO: Add `current_paint`/`current_transform` used via callbacks like
    // Renderer::with_transform(transform, |renderer| ...). But before this we
    // need to move from EG to our implementation of Path.
    fn base_paint<'a>(&self) -> Paint<'a> {
        let paint = Paint::default();
        // TODO: Renderer options: anti-aliasing, colorspace, etc.
        paint
    }

    // TODO: How do we deal with the StrokeAlignment that is supported by
    // embedded graphics but not by tiny-skia? Do we just ignore it and always
    // stroke centered on the path? Or do we implement it ourselves by stroking
    // with offset?
    fn tiny_skia_path(
        &mut self,
        path: &tiny_skia::Path,
        style: &DrawStyle<tiny_skia::Color>,
    ) {
        let transform = self.base_transform();

        if let Some(fill) = style.fill {
            let mut paint = self.base_paint();
            paint.set_color(fill);

            let (mut canvas, clip) = self.canvas_and_clip();
            canvas.fill_path(
                path,
                &paint,
                tiny_skia::FillRule::default(),
                transform,
                clip,
            );
        }

        if let Some(stroke) = style.stroke {
            // TODO: Play with LineCap, miter_limit and LineJoin

            let mut paint = self.base_paint();
            paint.set_color(stroke);
            paint.blend_mode = tiny_skia::BlendMode::SourceOver;

            let mut stroke = tiny_skia::Stroke::default();
            stroke.width = style.stroke_width as f32;
            stroke.line_cap = tiny_skia::LineCap::Round;

            let (mut canvas, clip) = self.canvas_and_clip();
            canvas.stroke_path(path, &paint, &stroke, transform, clip);
        }
    }
}

// NOTE (WS6.4d): `output` / `output_regions` lived here and went with the
// `RenderTarget` seam (see `output/mod.rs`). A detached `Pixmap` is the
// caller's, and tiny-skia's own API is a better exit than anything rsact could
// wrap: `encode_png`, `save_png`, `pixels()`, or a future
// `PixmapExt::map_to_framebuffer` over `MapColor` for embedded targets.

impl<P: crate::region::FramePolicy> Renderer
    for TinySkiaRenderer<tiny_skia::Color, P>
{
    type Color = tiny_skia::Color;

    /// Whatever policy this renderer was built with — `Unbounded` by default,
    /// which is right for a pixmap covering the frame and for the
    /// one-pixmap-per-region showcase path. See the type docs for why a bounded
    /// policy here means a *shape*, not a byte budget.
    type Policy = P;

    fn size(&self) -> Size {
        self.size
    }

    /// WS6.4d: aim at `region` and clear it.
    ///
    /// Two things, and the second is not optional. The origin moves, so absolute
    /// coordinates land in a pixmap smaller than the frame; then the pixmap is
    /// **cleared**, because it arrives holding whatever the previous region left
    /// in it and tiny-skia composites `SourceOver` against the destination.
    /// Without the clear, the first blended edge in each region would mix with
    /// an unrelated pixel — a plausible image, not an obvious failure (roadmap
    /// 6.4 constraint (b)).
    ///
    /// A pixmap already covering the frame skips both: it needs no rebase, and
    /// clearing it would erase the frame a damage-driven repaint relies on.
    fn begin_region(&mut self, region: Rect) -> RenderResult {
        let want = Self::bytes_for(region.size);
        if want > self.capacity {
            // The policy check at `attach` guarantees the planner never asks for
            // this, so it is a renderer driven outside that path. Log and skip
            // (WS1.8) rather than reallocating behind the caller's back — the
            // whole contract is that the storage is theirs and fixed.
            log::error!(
                "region {region:?} needs {want} bytes, the attached pixmap \
                 holds {}; skipping it",
                self.capacity,
            );
            return Err(());
        }

        // Re-stride the storage to this region. A shrink keeps the vector's
        // capacity and a regrow stays inside it, so no reallocation happens —
        // which is what makes a byte budget the right bound for a pixmap even
        // though a pixmap has a fixed width.
        self.pixels.resize(want, 0);
        self.region = region.size;
        self.origin = region.top_left;

        // A region arrives holding whatever the previous one left in it, and
        // tiny-skia composites `SourceOver` against the destination — so
        // without this the first blended edge would mix with an unrelated pixel
        // (roadmap 6.4 constraint (b)). A pixmap already covering the frame is
        // exempt: clearing it would erase what a damage-driven repaint relies
        // on.
        if region.size != self.size {
            self.canvas().fill(tiny_skia::Color::WHITE);
        }
        self.rebuild_clip_mask();
        Ok(())
    }

    fn push_clip(&mut self, area: Rect) {
        let nested =
            ViewportKind::Clipped(area).nested_in(self.current_viewport());
        self.viewport_stack.push(nested);
        self.rebuild_clip_mask();
    }

    fn pop_clip(&mut self) {
        self.viewport_stack.pop();
        self.rebuild_clip_mask();
    }

    fn clip_bounds(&self) -> Option<Rect> {
        // Fullscreen ⇒ the surface rect (see `EGRenderer::renderer_clip_bounds`).
        // Absolute, like every rect crossing this boundary: the pixmap's own
        // rect is `origin + its size`, not `(0,0) + its size`.
        Some(
            self.current_viewport()
                .clip_bounds()
                .unwrap_or_else(|| Rect::new(self.origin, self.painted_size())),
        )
    }

    fn fill_solid(&mut self, rect: Rect, color: Self::Color) -> RenderResult {
        let mut paint = self.base_paint();

        paint.set_color(color);

        let transform = self.base_transform();
        let (mut canvas, clip) = self.canvas_and_clip();
        canvas.fill_rect(rect.into(), &paint, transform, clip);

        Ok(())
    }

    fn pixel(&mut self, point: Point, color: Self::Color) -> RenderResult {
        // TODO: Replace with a distinct `contains` method to avoid creating a
        // Rect each time.
        if !self.bounding_box().contains(point) {
            return Ok(());
        }

        // Indexes the pixmap directly, so it must apply the origin rebase by
        // hand — the `Transform` the draw calls get does not reach here.
        let local = point - self.origin;
        let mut pixmap = self.canvas();
        let (w, h) = (pixmap.width(), pixmap.height());
        if local.x < 0
            || local.y < 0
            || local.x as u32 >= w
            || local.y as u32 >= h
        {
            return Ok(());
        }
        pixmap.pixels_mut()[local.y as usize * w as usize + local.x as usize] =
            color.premultiply().to_color_u8();

        Ok(())
    }

    // TODO: Shouldn't line only have stroke and no fill? tiny-skia allows line
    // to have both which seems incorrect.
    fn line(
        &mut self,
        from: Point,
        to: Point,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let mut path = tiny_skia::PathBuilder::new();
        path.move_to(from.x as f32, from.y as f32);
        path.line_to(to.x as f32, to.y as f32);
        self.tiny_skia_path(&path.finish().unwrap(), style);

        Ok(())
    }

    fn rect(
        &mut self,
        rect: Rect,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let mut path = tiny_skia::PathBuilder::new();
        path.push_rect(
            tiny_skia::Rect::from_xywh(
                rect.top_left.x as f32,
                rect.top_left.y as f32,
                rect.size.width as f32,
                rect.size.height as f32,
            )
            .unwrap(),
        );

        self.tiny_skia_path(&path.finish().unwrap(), style);

        Ok(())
    }

    // TODO: Pre-clamped corner radius type?
    fn rounded_rect(
        &mut self,
        rect: Rect,
        corners: crate::prelude::CornerRadii,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let mut path = tiny_skia::PathBuilder::new();

        let corners = corners.clamp_for(rect.size);
        path.rounded_rect(rect, corners);

        self.tiny_skia_path(&path.finish().unwrap(), style);

        Ok(())
    }

    fn circle(
        &mut self,
        top_left: Point,
        diameter: u32,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let mut path = tiny_skia::PathBuilder::new();
        path.push_circle(
            top_left.x as f32 + diameter as f32 / 2.0,
            top_left.y as f32 + diameter as f32 / 2.0,
            diameter as f32 / 2.0,
        );

        self.tiny_skia_path(&path.finish().unwrap(), style);

        Ok(())
    }

    fn arc(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let mut path = tiny_skia::PathBuilder::new();
        path.arc(top_left, diameter, start, sweep);

        self.tiny_skia_path(&path.finish().unwrap(), style);

        Ok(())
    }

    fn ellipse(
        &mut self,
        bounding_box: Rect,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let mut path = tiny_skia::PathBuilder::new();
        path.push_oval(
            tiny_skia::Rect::from_xywh(
                bounding_box.top_left.x as f32,
                bounding_box.top_left.y as f32,
                bounding_box.size.width as f32,
                bounding_box.size.height as f32,
            )
            .unwrap(),
        );

        self.tiny_skia_path(&path.finish().unwrap(), style);

        Ok(())
    }

    fn sector(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let radius = diameter as f32 / 2.0;
        let center = tiny_skia::Point::from_xy(
            top_left.x as f32 + radius,
            top_left.y as f32 + radius,
        );

        let mut path = tiny_skia::PathBuilder::new();

        path.move_to(center.x, center.y);
        path.line_to(
            center.x + radius * start.radians.cos(),
            center.y + radius * start.radians.sin(),
        );
        path.arc(top_left, diameter, start, sweep);
        path.line_to(center.x, center.y);
        path.close();

        self.tiny_skia_path(&path.finish().unwrap(), style);

        Ok(())
    }

    fn polygon(
        &mut self,
        points: &[Point],
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let mut path = tiny_skia::PathBuilder::new();
        if let Some(first) = points.first() {
            path.move_to(first.x as f32, first.y as f32);
            for point in &points[1..] {
                path.line_to(point.x as f32, point.y as f32);
            }
            path.close();
        }

        self.tiny_skia_path(&path.finish().unwrap(), style);

        Ok(())
    }

    fn path(
        &mut self,
        path: &Path,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        self.tiny_skia_path(&path.clone().into(), style);

        Ok(())
    }

    fn image<'a>(
        &mut self,
        image: crate::image::DrawImage<'a, Self::Color>,
    ) -> RenderResult {
        let draw_box = self.bounding_box().intersection(&image.bounding_box());
        let image_pixmap = PixmapRef::from_bytes(
            image.data(),
            image.size().width,
            image.size().height,
        )
        .ok_or(())?;
        let paint = PixmapPaint::default();
        let origin = self.origin;
        let (mut canvas, clip) = self.canvas_and_clip();
        // `draw_pixmap` takes integer coordinates rather than a transform for
        // placement, so the rebase is a subtraction here too.
        canvas.draw_pixmap(
            draw_box.top_left.x - origin.x,
            draw_box.top_left.y - origin.y,
            image_pixmap,
            &paint,
            Transform::identity(),
            clip,
        );

        Ok(())
    }
}

// impl Renderer for TinySkiaRenderer {
//     type Color = Rgb888;
//     type Options = ();

//     fn set_options(&mut self, options: Self::Options) {

//     }

//     fn clipped(
//         &mut self,
//         area: embedded_graphics::primitives::Rectangle,
//         f: impl FnOnce(&mut Self) -> crate::prelude::RenderResult,
//     ) -> crate::prelude::RenderResult {
//         todo!()
//     }

//     fn render(
//         &mut self,
//         renderable: &impl super::Renderable<<Self as Renderer>::Color>,
//     ) -> crate::prelude::RenderResult {
//         todo!()
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        geometry::{Point, Size},
        renderer::Renderer as _,
        style::DrawStyle,
    };

    const SIZE: u32 = 32;
    const CLIP_H: u32 = 8;

    /// The pixmap is the TEST's, as it is any caller's — the renderer allocates
    /// nothing (WS6.4d).
    fn pixmap(size: Size) -> Pixmap {
        let mut p = Pixmap::new(size.width, size.height).unwrap();
        p.fill(tiny_skia::Color::WHITE);
        p
    }

    fn renderer() -> TinySkiaRenderer<tiny_skia::Color> {
        let size = Size::new_equal(SIZE);
        TinySkiaRenderer::new(size, pixmap(size))
    }

    /// Ink, not alpha.
    ///
    /// A fresh canvas is **opaque white**, so "alpha != 0" is true on every
    /// pixel of an untouched surface and an assertion built on it passes
    /// whatever the renderer does. (Second time this trap has appeared in this
    /// project — the first was a golden test drawing `WHITE` on a `WHITE`
    /// background.) So: paint BLACK, and look for pixels that are not white.
    fn row_has_ink(r: &TinySkiaRenderer<tiny_skia::Color>, y: u32) -> bool {
        let w = r.region.width;
        (0..w).any(|x| {
            let i = ((y * w + x) * 4) as usize;
            (r.pixels[i], r.pixels[i + 1], r.pixels[i + 2]) != (255, 255, 255)
        })
    }

    /// Guards every test below: a fresh surface must contain no ink, or
    /// "nothing escaped the clip" would be true by construction.
    #[test]
    fn a_fresh_surface_has_no_ink() {
        let r = renderer();
        assert!(!row_has_ink(&r, 0) && !row_has_ink(&r, SIZE - 1));
    }

    /// WS6.11: tiny-skia used to push a viewport and then draw with
    /// `Transform::identity()` and mask `None` — i.e. the clip was a **no-op**,
    /// and `Scrollable`'s content overflowed its window on this backend.
    ///
    /// Harmless while nothing in rsact pushed a clip. WS6.4c(F) made
    /// `CLIPS_CHILDREN` real on every backend, so from that point this was a
    /// visible defect, not a latent one — and WS16.3 plans the desktop tier here.
    #[test]
    fn a_clip_actually_clips() {
        let mut r = renderer();
        let whole = Rect::new(Point::zero(), Size::new_equal(SIZE));

        r.push_clip(Rect::new(Point::zero(), Size::new(SIZE, CLIP_H)));
        r.fill_solid(whole, tiny_skia::Color::BLACK).unwrap();
        r.pop_clip();

        assert!(
            row_has_ink(&r, 0),
            "nothing was painted at all — the test proves nothing"
        );
        assert!(
            !row_has_ink(&r, CLIP_H + 1),
            "the fill escaped the clip: tiny-skia is ignoring it"
        );
    }

    /// The clip must be released again, or every later sibling inherits it.
    #[test]
    fn popping_the_clip_restores_full_drawing() {
        let mut r = renderer();
        let whole = Rect::new(Point::zero(), Size::new_equal(SIZE));

        r.push_clip(Rect::new(Point::zero(), Size::new(SIZE, CLIP_H)));
        r.pop_clip();
        r.fill_solid(whole, tiny_skia::Color::BLACK).unwrap();

        assert!(row_has_ink(&r, SIZE - 1), "the clip outlived its pop");
    }

    /// Nested clips **intersect** — the same composition `ViewportKind::nested_in`
    /// gives the other backends, and what `clip_bounds()` promises the culler.
    /// A wider inner clip must not widen the effective one.
    #[test]
    fn a_nested_clip_narrows_and_never_widens() {
        let mut r = renderer();
        let whole = Rect::new(Point::zero(), Size::new_equal(SIZE));

        r.push_clip(Rect::new(Point::zero(), Size::new(SIZE, CLIP_H)));
        r.push_clip(whole); // wider than the parent
        r.fill_solid(whole, tiny_skia::Color::BLACK).unwrap();
        r.pop_clip();
        r.pop_clip();

        assert!(row_has_ink(&r, 0), "the intersection painted nothing");
        assert!(
            !row_has_ink(&r, CLIP_H + 1),
            "a wider child clip widened the effective clip"
        );
    }

    /// Path drawing (fill and stroke) goes through the mask too, not only
    /// `fill_solid` — all four draw entry points had `None` hard-coded.
    #[test]
    fn paths_are_clipped_as_well() {
        let mut r = renderer();
        let whole = Rect::new(Point::zero(), Size::new_equal(SIZE));

        r.push_clip(Rect::new(Point::zero(), Size::new(SIZE, CLIP_H)));
        r.rect(whole, &DrawStyle::default().fill(tiny_skia::Color::BLACK))
            .unwrap();
        r.pop_clip();

        assert!(row_has_ink(&r, 0));
        assert!(!row_has_ink(&r, CLIP_H + 1), "a filled path escaped the clip");
    }
    /// WS6.4d: a pixmap smaller than the frame paints the same picture as a
    /// full-frame one — the tiny-skia half of the tiling equality.
    ///
    /// The mirror of `a_tiled_surface_paints_the_same_pixels_as_a_full_one` in
    /// the embedded-graphics backend, and it catches the same class of bug:
    /// tiny-skia's rebase is a `Transform` on the path calls plus a subtraction
    /// on the two paths that index pixels directly (`pixel`, `image`), and
    /// getting either sign wrong yields a plausible image.
    #[test]
    fn a_partial_pixmap_paints_what_a_full_one_would() {
        const W: u32 = 48;
        const H: u32 = 48;
        const BAND: u32 = 12;
        let viewport = Size::new(W, H);

        fn content<R: Renderer<Color = tiny_skia::Color>>(r: &mut R) {
            r.fill_solid(
                Rect::new(Point::new(4, 6), Size::new(30, 20)),
                tiny_skia::Color::BLACK,
            )
            .unwrap();
            r.rect(
                Rect::new(Point::new(2, 2), Size::new(44, 44)),
                &DrawStyle::default()
                    .stroke(tiny_skia::Color::from_rgba8(0, 160, 0, 255))
                    .stroke_width(2),
            )
            .unwrap();
            for i in 0..40i32 {
                r.pixel(
                    Point::new(i, 47 - i),
                    tiny_skia::Color::from_rgba8(200, 0, 0, 255),
                )
                .unwrap();
            }
        }

        // Reference: one full-frame pixmap, one pass.
        let mut full = TinySkiaRenderer::new(viewport, pixmap(viewport));
        content(&mut full);
        let (_, reference, _) = full.detach();

        // Tiled: a W x BAND pixmap — a quarter of the frame — reattached per
        // region, exactly as a caller sizing from `Frame::peek_region` would.
        let mut composed = pixmap(viewport);
        let band = Size::new(W, BAND);
        let mut tiled = TinySkiaRenderer::new(viewport, pixmap(band));
        let mut spare = pixmap(band);

        for i in 0..(H / BAND) as i32 {
            let region = Rect::new(Point::new(0, i * BAND as i32), band);
            tiled.begin_region(region).unwrap();
            tiled.push_clip(region);
            content(&mut tiled);
            tiled.pop_clip();
            tiled.end_region().unwrap();

            let (parked, tile, at) = tiled.detach();
            // The caller's blit: raw pixels plus where they go.
            for row in 0..at.size.height as usize {
                for col in 0..at.size.width as usize {
                    let src = tile.pixels()[row * at.size.width as usize + col];
                    let x = at.top_left.x as usize + col;
                    let y = at.top_left.y as usize + row;
                    composed.pixels_mut()[y * W as usize + x] = src;
                }
            }
            tiled = parked.attach(spare);
            spare = tile;
        }

        // Not vacuous: the reference must actually hold a picture.
        let inked = |p: &Pixmap| {
            p.pixels()
                .iter()
                .filter(|c| (c.red(), c.green(), c.blue()) != (255, 255, 255))
                .count()
        };
        assert!(
            inked(&reference) > (W * H) as usize / 8,
            "the reference frame painted only {} of {} pixels",
            inked(&reference),
            W * H
        );

        let mismatches: alloc::vec::Vec<usize> = (0..(W * H) as usize)
            .filter(|&i| reference.pixels()[i] != composed.pixels()[i])
            .collect();
        assert!(
            mismatches.is_empty(),
            "{} of {} pixels differ between a full pixmap and a banded one; \
             first at ({}, {})",
            mismatches.len(),
            W * H,
            mismatches[0] % W as usize,
            mismatches[0] / W as usize,
        );
    }

    /// WS6.4d: a pixmap's bound is a **byte budget**, not a shape — the storage
    /// is re-strided per region, so any region fitting the bytes is painted.
    ///
    /// This is what makes the tiny-skia backend take a fixed pool on the same
    /// terms as the framebuffer one. It replaces a test asserting the opposite:
    /// a 64×8 pixmap used to *refuse* a 32×16 region, on the grounds that a
    /// pixmap has a fixed `width()`. It does — but `take`/`from_vec` let the
    /// same allocation be re-shaped, and both regions are 512 pixels.
    #[test]
    fn a_pixmap_is_reshaped_per_region_not_bound_to_its_shape() {
        let viewport = Size::new(64, 64);
        // 64x8 = 512 px = 2048 bytes.
        let mut r = TinySkiaRenderer::new(viewport, pixmap(Size::new(64, 8)));
        let budget = r.capacity;

        // Same byte count, different shape — accepted, and the view really is
        // 32 wide (a stale stride would report 64 and address the wrong rows).
        let square = Rect::new(Point::new(8, 16), Size::new(32, 16));
        assert!(r.begin_region(square).is_ok());
        assert_eq!(r.region, Size::new(32, 16));
        assert_eq!(r.canvas().width(), 32);

        // Paint at the region's far corner and read it back through the view,
        // which only lands correctly if the stride followed the reshape.
        r.pixel(Point::new(39, 31), tiny_skia::Color::BLACK)
            .unwrap();
        let i = ((15 * 32 + 31) * 4) as usize;
        assert_ne!(
            (r.pixels[i], r.pixels[i + 1], r.pixels[i + 2]),
            (255, 255, 255),
            "the bottom-right pixel of a reshaped region did not land"
        );

        // A narrow tall region — the case a fixed-width pixmap could never hold.
        assert!(
            r.begin_region(Rect::new(Point::zero(), Size::new(4, 128)))
                .is_ok()
        );
        assert_eq!(r.canvas().width(), 4);

        // And none of it reallocated: every reshape stayed inside the vector the
        // caller lent, which is the whole reason a byte budget is the right bound.
        assert_eq!(
            r.capacity, budget,
            "the reshaping ceiling moved — the caller's allocation was replaced"
        );
        assert!(
            r.pixels.capacity() >= budget,
            "reshaping shrank the allocation, so regrowing it will reallocate"
        );
    }

    /// The bound still bites: a region needing more bytes than were lent is
    /// refused rather than silently reallocating behind the caller.
    #[test]
    fn a_region_over_the_byte_budget_is_refused() {
        let viewport = Size::new(64, 64);
        let mut r = TinySkiaRenderer::new(viewport, pixmap(Size::new(64, 8)));
        // 2052 bytes wanted against 2048 lent — one pixel too many.
        let over = Rect::new(Point::zero(), Size::new(513, 1));
        assert!(
            r.begin_region(over).is_err(),
            "a region over the lent byte budget must be refused; reallocating \
             would quietly take ownership of storage that is not ours"
        );
    }
}
