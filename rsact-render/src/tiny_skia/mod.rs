use crate::{
    layer::{Layering, Surface},
    output::{FinishRender, MapColor, pixel::Pixel},
    prelude::{Angle, DrawStyle, Path, Point, Rect, RenderResult, Size, *},
    tiny_skia::path::PathBuilderExt,
};
use core::marker::PhantomData;
use tiny_skia::{
    IntSize, Paint, Pixmap, PixmapPaint, PremultipliedColorU8, Transform,
};

#[allow(unused)]
use num::Float as _;

pub mod color;
pub mod geometry;
pub mod path;

// TODO: Generic Layer + Viewport stack structure

// impl<'a> Into<Paint<'a>> for DrawStyle<tiny_skia::Color> {
//     fn into(self) -> Paint<'a> {
//         let mut paint = Paint::default();

//         paint.set_color(self.fill.unwrap_or(tiny_skia::Color::WHITE));

//         paint
//     }
// }

impl Surface for Pixmap {
    fn new(size: Size) -> Self {
        // To avoid using new + fill we preallocate a vector for Pixmap with opaque white background.
        // TODO: We better get rid of default_background and default_foreground for Color as we usually expect white and black for these and Color type must not dictate it as it is not color-type-dependent property, but actual default. Color must have BLACK and WHITE constants instead.
        // TODO: Copy overflow-safe data length computation from tiny-skia?
        let data = vec![0xff; size.width as usize * size.height as usize * 4];
        Pixmap::from_vec(
            data,
            IntSize::from_wh(size.width, size.height).unwrap(),
        )
        .unwrap()
    }
}

// TODO: Renderer settings: arc tolerance

pub struct TinySkiaRenderer<C> {
    layers: Layering<Pixmap>,
    size: Size,
    _color: PhantomData<C>,
}

impl TinySkiaRenderer<tiny_skia::Color> {
    pub fn new(size: Size) -> Self {
        Self { layers: Layering::new(size), size, _color: PhantomData }
    }

    fn base_paint<'a>(&self) -> Paint<'a> {
        let paint = Paint::default();
        // TODO: Renderer options: anti-aliasing, colorspace, etc.
        paint
    }

    // TODO: How do we deal with the StrokeAlignment that is supported by embedded graphics but not by tiny-skia? Do we just ignore it and always stroke centered on the path? Or do we implement it ourselves by stroking with offset?
    fn tiny_skia_path(
        &mut self,
        path: &tiny_skia::Path,
        style: &DrawStyle<tiny_skia::Color>,
    ) {
        if let Some(fill) = style.fill {
            let mut paint = self.base_paint();
            paint.set_color(fill);

            self.layers.surface_mut().fill_path(
                path,
                &paint,
                tiny_skia::FillRule::default(),
                Transform::identity(),
                None,
            );
        }

        if let Some(stroke) = style.stroke {
            // TODO: Play with LineCap, miter_limit and LineJoin

            let mut paint = self.base_paint();
            paint.set_color(stroke);

            let mut stroke = tiny_skia::Stroke::default();
            stroke.width = style.stroke_width as f32;
            stroke.line_cap = tiny_skia::LineCap::Round;

            self.layers.surface_mut().stroke_path(
                path,
                &paint,
                &stroke,
                Transform::identity(),
                None,
            );
        }
    }
}

impl<C> FinishRender<C> for TinySkiaRenderer<tiny_skia::Color>
where
    PremultipliedColorU8: MapColor<C>,
{
    fn finish_frame(&mut self, target: &mut impl RenderTarget<Color = C>) {
        let result = self
            .layers
            .layers_mut()
            .reduce(|result, layer| {
                let paint = PixmapPaint::default();

                result.draw_pixmap(
                    0,
                    0,
                    layer.as_ref(),
                    &paint,
                    Transform::identity(),
                    None,
                );
                result
            })
            .unwrap();

        let colors = result.pixels();
        let points = Rect::new(
            Point::zero(),
            Size::new(result.width(), result.height()),
        )
        .points();

        target.draw(
            points
                .into_iter()
                .zip(colors.into_iter())
                .map(|(point, color)| Pixel(point, color.map_color())),
        );
    }
}

impl Renderer for TinySkiaRenderer<tiny_skia::Color> {
    type Color = tiny_skia::Color;
    type Options = ();

    fn set_options(&mut self, _options: Self::Options) {}

    fn size(&self) -> Size {
        self.size
    }

    fn clipped(
        &mut self,
        area: Rect,
        f: impl FnOnce(&mut Self) -> RenderResult,
    ) -> RenderResult {
        self.layers.enter_viewport(ViewportKind::Clipped(area));
        let result = f(self);
        self.layers.exit_viewport();
        result
    }

    fn fill_solid(&mut self, rect: Rect, color: Self::Color) -> RenderResult {
        let mut paint = self.base_paint();

        paint.set_color(color);

        self.layers.surface_mut().fill_rect(
            rect.into(),
            &paint,
            Transform::identity(),
            None,
        );

        Ok(())
    }

    // TODO: Shouldn't line only have stroke and no fill? tiny-skia allows line to have both which seems incorrect.
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

    // TODO: Pre-clamped corner radius?
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

    // TODO: Shouldn't circle be center-based?
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
        let radius = diameter / 2;
        path.arc(
            top_left + Point::new_equal(radius as i32),
            radius,
            start,
            sweep,
        );

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

    // TODO: Wait, isn't sector just an arc?
    // No, arc fills between start and end angles, while sector also fills between center and arc, so it is more like a pie slice.
    fn sector(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let mut path = tiny_skia::PathBuilder::new();

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
