//! tiny-skia's rasterizer as an L2 citizen.

use crate::{
    blitter::{Blitter, Span},
    geometry::{Angle, CornerRadii, Point, Rect, Size},
    path::Path,
    raster::{RasterCtx, Rasterizer},
    style::DrawStyle,
    tiny_skia::path::PathBuilderExt,
};
use tiny_skia::{FillRule, Mask, PathBuilder, Stroke, Transform};

/// tiny-skia's scan conversion, producing **coverage** rather than pixels.
///
/// # Why this is L2 and not a backend of its own
///
/// tiny-skia's public painting API is *fused*: `PixmapMut::fill_path` rasterizes
/// and blends in one call, into a `Pixmap`. Fusing is what this design undoes,
/// so none of it is used here. [`Mask::fill_path`] is the primitive underneath
/// those calls — this repo already called it, to build clip masks — and it hands
/// back the coverage buffer itself. So coverage is produced **once** and blended
/// **once**, in our blitter.
///
/// The payoff is that no color is pinned. `impl<T: Blitter> Rasterizer<T>` with
/// no bound on `T::Color` makes tiny-skia's anti-aliasing available over an
/// **Rgb565 framebuffer**, not only over a `Pixmap` — which an L1-only tiny-skia
/// backend could never offer.
///
/// # The mask is keyed on the clip, not the region
///
/// L2 never learns a region exists; the clip is the largest rect it may write
/// anyway. The buffer is grow-only and re-used, because a `Mask` is `w·h` bytes
/// and reallocating one per primitive would dominate everything else. Rows are
/// read at the *allocated* width, and only the clip's own width and height are
/// emitted.
///
/// # What is deliberately not overridden
///
/// [`fill`](Rasterizer::fill) and [`pixel`](Rasterizer::pixel). A style-free
/// axis-aligned rect has no edge to anti-alias, so routing it through a mask
/// would cost a coverage buffer and a per-pixel blend to arrive at the same
/// pixels the blitter's own `fill_rect` writes with `slice::fill`.
/// [`image`](Rasterizer::image) is inherited for the same kind of reason: it is
/// a decode, not a rasterization.
pub struct TinySkiaRasterizer {
    mask: Option<Mask>,
    /// The allocated mask's size, which is the high-water mark of every clip
    /// seen so far — not the current clip.
    mask_size: Size,
    /// Reused per stroke so its scratch allocations survive between primitives.
    stroker: tiny_skia::PathStroker,
}

impl Default for TinySkiaRasterizer {
    fn default() -> Self {
        Self::new()
    }
}

impl TinySkiaRasterizer {
    pub fn new() -> Self {
        Self {
            mask: None,
            mask_size: Size::zero(),
            stroker: tiny_skia::PathStroker::new(),
        }
    }

    /// Rasterize `path` into the mask and blit its coverage as `color`.
    ///
    /// The transform carries absolute coordinates into mask-local ones, so the
    /// mask's `(0, 0)` is the clip's top-left — which is what lets one buffer
    /// serve any clip without re-addressing.
    fn emit<T: Blitter + ?Sized>(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        path: &tiny_skia::Path,
        color: T::Color,
    ) {
        let clip = cx.clip();
        if clip.is_zero_sized() {
            return;
        }

        if self.mask_size.width < clip.size.width
            || self.mask_size.height < clip.size.height
        {
            self.mask_size = Size::new(
                self.mask_size.width.max(clip.size.width),
                self.mask_size.height.max(clip.size.height),
            );
            self.mask = Mask::new(self.mask_size.width, self.mask_size.height);
        }
        let Some(mask) = self.mask.as_mut() else { return };

        // `fill_path` accumulates onto whatever is already there — it is
        // documented as drawing on top — so the previous primitive's coverage
        // has to go first, or every shape inherits the last one's edges.
        mask.clear();
        mask.fill_path(
            path,
            FillRule::Winding,
            true,
            Transform::from_translate(
                -clip.top_left.x as f32,
                -clip.top_left.y as f32,
            ),
        );

        let stride = mask.width() as usize;
        let width = clip.size.width as usize;
        let data = mask.data();
        for row in 0..clip.size.height as usize {
            let start = row * stride;
            cx.blend(
                Span::new(
                    clip.top_left.y + row as i32,
                    clip.top_left.x,
                    clip.size.width,
                ),
                color,
                &data[start..start + width],
            );
        }
    }

    /// Fill then stroke, each as its own coverage pass.
    ///
    /// Two passes rather than one because they are two colors: a mask carries
    /// coverage, not paint, so a shape that is filled *and* stroked needs one
    /// mask per color. The stroke pass rasterizes the stroke **outline** — an
    /// ordinary filled path — which is how tiny-skia strokes internally too.
    fn draw<T: Blitter + ?Sized>(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        path: &tiny_skia::Path,
        style: &DrawStyle<T::Color>,
    ) {
        if let Some(fill) = style.fill {
            self.emit(cx, path, fill);
        }
        if let (Some(color), width @ 1..) = (style.stroke, style.stroke_width) {
            // TODO: `StrokeAlignment` is not expressible in tiny-skia, which
            // always strokes centred on the path. Implementing Inside/Outside
            // means offsetting the path first. Recorded rather than silently
            // ignored — it is exactly the kind of *parameter semantics*
            // divergence the post-refactor rasterizer audit has to collect.
            let mut stroke = Stroke::default();
            stroke.width = width as f32;
            stroke.line_cap = tiny_skia::LineCap::Round;
            if let Some(outline) = self.stroker.stroke(path, &stroke, 1.0) {
                self.emit(cx, &outline, color);
            }
        }
    }
}

impl<T: Blitter + ?Sized> Rasterizer<T> for TinySkiaRasterizer {
    fn line(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        from: Point,
        to: Point,
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        path.move_to(from.x as f32, from.y as f32);
        path.line_to(to.x as f32, to.y as f32);
        // A line has no interior; only the stroke pass can produce anything, and
        // `Mask::fill_path` refuses a zero-area path anyway.
        if let Some(path) = path.finish() {
            self.draw(cx, &path, &DrawStyle { fill: None, ..*style });
        }
    }

    fn rect(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        rect: Rect,
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        if let Some(r) = tiny_skia::Rect::from_xywh(
            rect.top_left.x as f32,
            rect.top_left.y as f32,
            rect.size.width as f32,
            rect.size.height as f32,
        ) {
            path.push_rect(r);
        }
        if let Some(path) = path.finish() {
            self.draw(cx, &path, style);
        }
    }

    fn rounded_rect(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        rect: Rect,
        corners: CornerRadii,
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        path.rounded_rect(rect, corners.clamp_for(rect.size));
        if let Some(path) = path.finish() {
            self.draw(cx, &path, style);
        }
    }

    fn circle(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        top_left: Point,
        diameter: u32,
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        path.push_circle(
            top_left.x as f32 + diameter as f32 / 2.0,
            top_left.y as f32 + diameter as f32 / 2.0,
            diameter as f32 / 2.0,
        );
        if let Some(path) = path.finish() {
            self.draw(cx, &path, style);
        }
    }

    fn arc(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        path.arc(top_left, diameter, start, sweep);
        if let Some(path) = path.finish() {
            // An arc is a curve: stroke only, matching `scan::arc` and the
            // parameter semantics on the trait.
            self.draw(cx, &path, &DrawStyle { fill: None, ..*style });
        }
    }

    fn sector(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<T::Color>,
    ) {
        let radius = diameter as f32 / 2.0;
        let center = tiny_skia::Point::from_xy(
            top_left.x as f32 + radius,
            top_left.y as f32 + radius,
        );
        let mut path = PathBuilder::new();
        path.move_to(center.x, center.y);
        path.arc(top_left, diameter, start, sweep);
        path.line_to(center.x, center.y);
        path.close();
        if let Some(path) = path.finish() {
            self.draw(cx, &path, style);
        }
    }

    fn ellipse(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        bounding_box: Rect,
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        if let Some(r) = tiny_skia::Rect::from_xywh(
            bounding_box.top_left.x as f32,
            bounding_box.top_left.y as f32,
            bounding_box.size.width as f32,
            bounding_box.size.height as f32,
        ) {
            path.push_oval(r);
        }
        if let Some(path) = path.finish() {
            self.draw(cx, &path, style);
        }
    }

    fn polygon(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        points: &[Point],
        style: &DrawStyle<T::Color>,
    ) {
        let mut path = PathBuilder::new();
        if let Some(first) = points.first() {
            path.move_to(first.x as f32, first.y as f32);
            for point in &points[1..] {
                path.line_to(point.x as f32, point.y as f32);
            }
            path.close();
        }
        if let Some(path) = path.finish() {
            self.draw(cx, &path, style);
        }
    }

    fn path(
        &mut self,
        cx: &mut RasterCtx<'_, T>,
        path: &Path,
        style: &DrawStyle<T::Color>,
    ) {
        let ts: tiny_skia::Path = path.clone().into();
        self.draw(cx, &ts, style);
    }
}
