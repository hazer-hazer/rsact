#[allow(unused)]
use crate::FloatExt as _;
use crate::{
    prelude::{Angle, DrawStyle, Path, Point, Rect, RenderResult, Size, *},
    tiny_skia::path::PathBuilderExt,
};
use alloc::vec::Vec;
use core::marker::PhantomData;
use tiny_skia::{
    Mask, Paint, PathBuilder, Pixmap, PixmapPaint, PixmapRef, Transform,
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
/// # A pixmap is bounded by SHAPE, not capacity
///
/// This is where the mirror stops being exact, and it matters.
/// `PackedFramebuf::retarget` re-strides at each region's own width, so one
/// buffer serves a 240×24 region and a 16×38 one alike — which is what makes a
/// *capacity* bound sound for [`EGRenderer`]. A `Pixmap` has a fixed `width()`
/// and indexes `y * width + x`; it cannot be re-strided. So a 240×24 pixmap
/// cannot hold a 16×38 region at all, even though both fit the same 5760-unit
/// budget, and the planner is entitled to emit exactly that under
/// `Tiles<240, 24>`.
///
/// Hence the honest usage today is **one pixmap per region**, sized from
/// [`Frame::peek_region`] before the region is painted:
///
/// ```ignore
/// while let Some(region) = frame.peek_region() {
///     renderer.attach(Pixmap::new(region.size.width, region.size.height).unwrap());
///     frame.render(&mut renderer);
///     let (pixmap, at) = renderer.detach().unwrap();
///     pixmap.save_png(format!("region-{}-{}.png", at.top_left.x, at.top_left.y))?;
/// }
/// ```
///
/// A fixed pixmap pool would need a policy that bounds region *shape* rather
/// than region *area* — `RegionLimits` carries no such field yet. Until it
/// does, a larger-than-needed pixmap also works: [`begin_region`] only requires
/// the pixmap to be at least as wide and as tall as the region.
///
/// [`EGRenderer`]: crate::eg::renderer::EGRenderer
/// [`Frame::peek_region`]: https://docs.rs/rsact-ui
/// [`begin_region`]: Renderer::begin_region
pub struct TinySkiaRenderer<C, P = crate::region::Unbounded> {
    /// `None` between a `detach` and the next `attach` — the window in which
    /// the owner holds their pixmap (encoding a PNG, blitting it, dropping it).
    pixmap: Option<Pixmap>,
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
        Self::build(size, pixmap)
    }
}

impl<P: crate::region::FramePolicy> TinySkiaRenderer<tiny_skia::Color, P> {
    /// A pixmap reused across regions, bounded by policy `P` — checked as a
    /// **shape** at [`attach`](Self::attach), for the reason in the type docs.
    pub fn tiled(size: Size, pixmap: Pixmap) -> Self {
        Self::build(size, pixmap)
    }

    fn build(size: Size, pixmap: Pixmap) -> Self {
        let mut this = Self {
            pixmap: None,
            origin: Point::zero(),
            size,
            viewport_stack: vec![ViewportKind::root()],
            clip_mask: None,
            _color: PhantomData,
            _policy: PhantomData,
        };
        this.attach(pixmap);
        this
    }

    /// Lend the renderer a pixmap, returning whatever it held.
    ///
    /// # Panics
    ///
    /// If `pixmap` is smaller than policy `P`'s largest region in either
    /// dimension. A **shape** check, not a capacity one — see the type docs.
    pub fn attach(&mut self, pixmap: Pixmap) -> Option<Pixmap> {
        if let Some(max) = <P as crate::region::FramePolicy>::MAX_REGION {
            assert!(
                pixmap.width() >= max.width && pixmap.height() >= max.height,
                "[rsact] pixmap {}x{} is too small for this renderer's frame \
                 policy, whose largest region is {}x{}. A pixmap cannot be \
                 re-strided, so its bound is a shape, not a byte budget.",
                pixmap.width(),
                pixmap.height(),
                max.width,
                max.height,
            );
        }
        self.pixmap.replace(pixmap)
    }

    /// Take the pixmap back, with the region that was painted into it.
    ///
    /// The rect comes from the renderer because `begin_region` told it where it
    /// was painting — re-pairing a surface with a rect by hand produces a
    /// plausible image rather than an obvious failure.
    pub fn detach(&mut self) -> Option<(Pixmap, Rect)> {
        let at = Rect::new(self.origin, self.painted_size()?);
        self.pixmap.take().map(|pixmap| (pixmap, at))
    }

    /// [`detach`](Self::detach) then [`attach`](Self::attach), for callers with
    /// two or more pixmaps who do not care about the ordering. See
    /// `EGRenderer::attach` for why the split pair is the primitive.
    pub fn swap(&mut self, next: Pixmap) -> Option<(Pixmap, Rect)> {
        let ready = self.detach();
        self.attach(next);
        ready
    }

    pub fn is_attached(&self) -> bool {
        self.pixmap.is_some()
    }

    fn painted_size(&self) -> Option<Size> {
        let pixmap = self.pixmap.as_ref()?;
        Some(Size::new(pixmap.width(), pixmap.height()))
    }

    /// The attached pixmap, or `None` while its owner holds it.
    ///
    /// Every drawing path goes through this and degrades to a logged no-op when
    /// detached (WS1.8: the UI logs and continues; a panic in a render loop that
    /// runs every frame is not recoverable).
    fn surface_mut(&mut self) -> Option<&mut Pixmap> {
        if self.pixmap.is_none() {
            log::warn!(
                "drawing with no pixmap attached — the region is discarded. \
                 Attach one before painting, or paint before detaching."
            );
        }
        self.pixmap.as_mut()
    }

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
        let Some(size) = self.painted_size() else {
            self.clip_mask = None;
            return;
        };
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

            let Self { pixmap, clip_mask, .. } = self;
            let Some(pixmap) = pixmap.as_mut() else { return };
            pixmap.fill_path(
                path,
                &paint,
                tiny_skia::FillRule::default(),
                transform,
                clip_mask.as_ref(),
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

            let Self { pixmap, clip_mask, .. } = self;
            let Some(pixmap) = pixmap.as_mut() else { return };
            pixmap.stroke_path(
                path,
                &paint,
                &stroke,
                transform,
                clip_mask.as_ref(),
            );
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
        let full_frame = self.painted_size().is_some_and(|s| {
            s.width >= self.size.width && s.height >= self.size.height
        });
        let Some(size) = self.painted_size() else { return Ok(()) };
        if full_frame {
            return Ok(());
        }
        if size.width < region.size.width || size.height < region.size.height {
            // A pixmap cannot be re-strided, so this is unrecoverable for THIS
            // region — but not for the frame. Log and skip (WS1.8); the region
            // is re-planned next frame, and the caller can size the next pixmap
            // from `Frame::peek_region`.
            log::error!(
                "pixmap {}x{} cannot hold region {region:?}; skipping it. A \
                 pixmap's bound is a shape, not a byte budget — size it from \
                 `Frame::peek_region`, or attach a larger one.",
                size.width,
                size.height,
            );
            return Err(());
        }
        self.origin = region.top_left;
        if let Some(pixmap) = self.pixmap.as_mut() {
            pixmap.fill(tiny_skia::Color::WHITE);
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
        Some(self.current_viewport().clip_bounds().unwrap_or_else(|| {
            Rect::new(self.origin, self.painted_size().unwrap_or(self.size))
        }))
    }

    fn fill_solid(&mut self, rect: Rect, color: Self::Color) -> RenderResult {
        let mut paint = self.base_paint();

        paint.set_color(color);

        let transform = self.base_transform();
        let Self { pixmap, clip_mask, .. } = self;
        let Some(pixmap) = pixmap.as_mut() else { return Ok(()) };
        pixmap.fill_rect(rect.into(), &paint, transform, clip_mask.as_ref());

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
        let Some(pixmap) = self.surface_mut() else { return Ok(()) };
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
        let Self { pixmap, clip_mask, .. } = self;
        let Some(pixmap) = pixmap.as_mut() else { return Ok(()) };
        // `draw_pixmap` takes integer coordinates rather than a transform for
        // placement, so the rebase is a subtraction here too.
        pixmap.draw_pixmap(
            draw_box.top_left.x - origin.x,
            draw_box.top_left.y - origin.y,
            image_pixmap,
            &paint,
            Transform::identity(),
            clip_mask.as_ref(),
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
        let pixmap = r.pixmap.as_ref().expect("attached");
        let w = pixmap.width();
        let px = pixmap.pixels();
        (0..w).any(|x| {
            let p = px[(y * w + x) as usize];
            (p.red(), p.green(), p.blue()) != (255, 255, 255)
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
        let (reference, _) = full.detach().expect("attached");

        // Tiled: a W x BAND pixmap — a quarter of the frame — reattached per
        // region, exactly as a caller sizing from `Frame::peek_region` would.
        let mut composed = pixmap(viewport);
        let band = Size::new(W, BAND);
        let mut tiled = TinySkiaRenderer::new(viewport, pixmap(band));
        let mut spare = Some(pixmap(band));

        for i in 0..(H / BAND) as i32 {
            let region = Rect::new(Point::new(0, i * BAND as i32), band);
            tiled.begin_region(region).unwrap();
            tiled.push_clip(region);
            content(&mut tiled);
            tiled.pop_clip();
            tiled.end_region().unwrap();

            let (tile, at) = tiled.detach().expect("a painted tile");
            // The caller's blit: raw pixels plus where they go.
            for row in 0..at.size.height as usize {
                for col in 0..at.size.width as usize {
                    let src = tile.pixels()[row * at.size.width as usize + col];
                    let x = at.top_left.x as usize + col;
                    let y = at.top_left.y as usize + row;
                    composed.pixels_mut()[y * W as usize + x] = src;
                }
            }
            tiled.attach(spare.take().unwrap());
            spare = Some(tile);
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

    /// A pixmap is bounded by SHAPE: a region taller than it cannot be painted,
    /// and saying so beats corrupting the frame silently.
    #[test]
    fn a_region_too_tall_for_the_pixmap_is_refused() {
        let viewport = Size::new(64, 64);
        let mut r = TinySkiaRenderer::new(viewport, pixmap(Size::new(64, 8)));
        // Same pixel count as 64x8, but 16 rows deep — a packed framebuffer
        // would re-stride and take it; a pixmap cannot.
        let tall = Rect::new(Point::zero(), Size::new(32, 16));
        assert!(
            r.begin_region(tall).is_err(),
            "a 64x8 pixmap must refuse a 32x16 region rather than paint \
             outside itself"
        );
    }
}
