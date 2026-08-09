#[allow(unused)]
use crate::FloatExt as _;
use crate::{
    output::{MapColor, RenderTarget, pixel::Pixel},
    prelude::{Angle, DrawStyle, Path, Point, Rect, RenderResult, Size, *},
    surface::{Canvas, Surface},
    tiny_skia::path::PathBuilderExt,
};
use core::marker::PhantomData;
use tiny_skia::{
    IntSize, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, PixmapRef,
    PremultipliedColorU8, Transform,
};

pub mod color;
pub mod geometry;
pub mod path;

impl Surface for Pixmap {
    fn new(size: Size) -> Self {
        // To avoid using new + fill we preallocate a vector for Pixmap with
        // opaque white background. TODO: We better get rid of
        // default_background and default_foreground for Color as we usually
        // expect white and black for these and Color type must not dictate it
        // as it is not color-type-dependent property, but actual default. Color
        // must have BLACK and WHITE constants instead. TODO: Copy
        // overflow-safe data length computation from tiny-skia?
        let data = vec![0xff; size.width as usize * size.height as usize * 4];
        Pixmap::from_vec(
            data,
            IntSize::from_wh(size.width, size.height).unwrap(),
        )
        .unwrap()
    }
}

pub struct TinySkiaRenderer<C> {
    canvas: Canvas<Pixmap>,
    size: Size,
    /// WS6.11: the active clip, as a tiny-skia [`Mask`].
    ///
    /// tiny-skia has no scissor rect — every draw call takes an
    /// `Option<&Mask>`, so a clip has to be an actual alpha mask. Rebuilt only
    /// when the clip stack changes (a `Mask` is `w*h` bytes, which is why it is
    /// cached rather than constructed per primitive), and `None` while the
    /// viewport is `Fullscreen`, which is both cheaper and the common case.
    clip_mask: Option<Mask>,
    _color: PhantomData<C>,
}

impl TinySkiaRenderer<tiny_skia::Color> {
    pub fn new(size: Size) -> Self {
        Self {
            canvas: Canvas::new(size),
            size,
            clip_mask: None,
            _color: PhantomData,
        }
    }

    /// Rebuild [`clip_mask`] from the composed viewport (WS6.11).
    ///
    /// Called only from `push_clip`/`pop_clip`, so the per-primitive cost is a
    /// null check. `enter_viewport` already intersects nested clips
    /// (`ViewportKind::nested_in`), so the rect here is the *composed* one —
    /// the same rect `clip_bounds()` reports, which is what keeps the culling
    /// contract and the actual clipping in agreement.
    fn rebuild_clip_mask(&mut self) {
        self.clip_mask = match self.canvas.current_viewport().clip_bounds() {
            None => None,
            Some(area) => {
                let mut mask = Mask::new(self.size.width, self.size.height)
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
                        Transform::identity(),
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
        if let Some(fill) = style.fill {
            let mut paint = self.base_paint();
            paint.set_color(fill);

            let Self { canvas, clip_mask, .. } = self;
            canvas.surface_mut().fill_path(
                path,
                &paint,
                tiny_skia::FillRule::default(),
                Transform::identity(),
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

            let Self { canvas, clip_mask, .. } = self;
            canvas.surface_mut().stroke_path(
                path,
                &paint,
                &stroke,
                Transform::identity(),
                clip_mask.as_ref(),
            );
        }
    }
}

/// WS6.4d: streaming the surface out is the backend's own inherent API, not a
/// trait rsact drives — see the note where `FinishRender` used to live
/// (`output/mod.rs`). This renderer always owns a full-frame pixmap, so its
/// caller can flush whenever it likes; nothing in the render path calls these.
impl TinySkiaRenderer<tiny_skia::Color> {
    /// Stream the whole frame into `target`.
    pub fn output<C>(&self, target: &mut impl RenderTarget<Color = C>)
    where
        PremultipliedColorU8: MapColor<C>,
    {
        let full = Rect::new(Point::zero(), self.size);
        self.output_regions(target, &[full]);
    }

    /// Stream only `regions` into `target` (WS6.3's damage-driven flush).
    ///
    /// Each region is clamped to the surface. Overlapping regions may write a
    /// pixel more than once — harmless, it is the same value — and an empty
    /// slice writes nothing.
    pub fn output_regions<C>(
        &self,
        target: &mut impl RenderTarget<Color = C>,
        regions: &[Rect],
    ) where
        PremultipliedColorU8: MapColor<C>,
    {
        // The single surface already holds the fully-drawn frame — stream only
        // the requested regions. A sub-rect must INDEX the buffer per point
        // (`y * width + x`), unlike a whole-frame zip of the point sequence to
        // the colour buffer.
        let result = self.canvas.surface();

        let width = result.width();
        let bounds =
            Rect::new(Point::zero(), Size::new(width, result.height()));
        let colors = result.pixels();

        for region in regions {
            let region = region.intersection(&bounds);
            target.draw(region.points().map(|point| {
                let idx = point.y as usize * width as usize + point.x as usize;
                Pixel(point, colors[idx].map_color())
            }));
        }
    }
}

impl Renderer for TinySkiaRenderer<tiny_skia::Color> {
    type Color = tiny_skia::Color;

    /// Owns a pixmap the size of the whole frame, so no region can be too
    /// large. Damage still shrinks what the caller flushes — see
    /// [`output_regions`](Self::output_regions).
    type Policy = crate::region::Unbounded;

    fn size(&self) -> Size {
        self.size
    }

    fn push_clip(&mut self, area: Rect) {
        self.canvas.enter_viewport(ViewportKind::Clipped(area));
        self.rebuild_clip_mask();
    }

    fn pop_clip(&mut self) {
        self.canvas.exit_viewport();
        self.rebuild_clip_mask();
    }

    fn clip_bounds(&self) -> Option<Rect> {
        // Fullscreen ⇒ the surface rect (see `EGRenderer::renderer_clip_bounds`).
        Some(
            self.canvas
                .current_viewport()
                .clip_bounds()
                .unwrap_or(Rect::new(Point::zero(), self.size)),
        )
    }

    fn fill_solid(&mut self, rect: Rect, color: Self::Color) -> RenderResult {
        let mut paint = self.base_paint();

        paint.set_color(color);

        let Self { canvas, clip_mask, .. } = self;
        canvas.surface_mut().fill_rect(
            rect.into(),
            &paint,
            Transform::identity(),
            clip_mask.as_ref(),
        );

        Ok(())
    }

    fn pixel(&mut self, point: Point, color: Self::Color) -> RenderResult {
        // TODO: Replace with a distinct `contains` method to avoid creating a
        // Rect each time.
        if !self.bounding_box().contains(point) {
            return Ok(());
        }

        let pixel_mut = &mut self.canvas.surface_mut().pixels_mut()
            [point.y as usize * self.size.width as usize + point.x as usize];

        *pixel_mut = color.premultiply().to_color_u8();

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
        let Self { canvas, clip_mask, .. } = self;
        canvas.surface_mut().draw_pixmap(
            draw_box.top_left.x,
            draw_box.top_left.y,
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

    fn renderer() -> TinySkiaRenderer<tiny_skia::Color> {
        TinySkiaRenderer::new(Size::new_equal(SIZE))
    }

    /// Ink, not alpha.
    ///
    /// A fresh canvas is **opaque white**, so "alpha != 0" is true on every
    /// pixel of an untouched surface and an assertion built on it passes
    /// whatever the renderer does. (Second time this trap has appeared in this
    /// project — the first was a golden test drawing `WHITE` on a `WHITE`
    /// background.) So: paint BLACK, and look for pixels that are not white.
    fn row_has_ink(r: &TinySkiaRenderer<tiny_skia::Color>, y: u32) -> bool {
        let px = r.canvas.surface().pixels();
        (0..SIZE).any(|x| {
            let p = px[(y * SIZE + x) as usize];
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
}
