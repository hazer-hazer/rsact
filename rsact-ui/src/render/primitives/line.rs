use crate::{
    layout::size::PointExt, prelude::Color, render::alpha::StyledAlphaDrawable,
};
use embedded_graphics::{
    Pixel,
    prelude::{Angle, Dimensions, Point, Primitive, Transform},
    primitives::{PrimitiveStyle, Rectangle, StyledDrawable},
};
use micromath::F32Ext as _;

// TODO: Canonize Line when constructing? Swap start and end in to always keep start.x < end.x?
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub start: Point,
    pub end: Point,
}

impl Dimensions for Line {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        Rectangle::new(
            self.start,
            embedded_graphics::geometry::Size::new(
                (self.end.x - self.start.x).abs() as u32,
                (self.end.y - self.start.y).abs() as u32,
            ),
        )
    }
}

impl Primitive for Line {}

impl Transform for Line {
    fn translate(&self, by: Point) -> Self {
        let mut new = *self;
        new.start += by;
        new.end += by;
        new
    }

    fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.start += by;
        self.end += by;
        self
    }
}

impl Line {
    pub fn new(start: Point, end: Point) -> Self {
        Self { start, end }
    }

    pub fn with_angle(center: Point, angle: Angle, radius: f32) -> Self {
        let (sin, cos) = angle.to_radians().sin_cos();
        Self::new(
            center,
            center.add_x_round(cos * radius).add_y_round(sin * radius),
        )
    }

    pub fn len_sq(&self) -> u32 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        dx.pow(2) as u32 + dy.pow(2) as u32
    }

    pub fn dist_to(&self, point: Point) -> f32 {
        // let l2 = self.start.dist_sq(self.end);
        let delta = self.end - self.start;
        let len_sq = (delta.x.pow(2) + delta.y.pow(2)) as f32;
        // Case when start == end
        // TODO: Can just be replaced with start == end check?
        if len_sq == 0.0 {
            point.dist_to(self.start)
        } else {
            // let t = ((point.x - self.start.x) * (self.end.x - self.start.x)
            //     + (point.y - self.start.y) * (self.end.y - self.start.y))
            //     as f32
            //     / l2;
            // let t =
            //     (point - self.start).dot(self.end - self.start) as f32 / len_sq;
            let t = (point.x - self.start.x) * delta.x
                + (point.y - self.start.y) * delta.y;
            let t = t as f32 / len_sq;
            let t = t.clamp(0.0, 1.0);
            let proj_x = self.start.x as f32 + t * delta.x as f32;
            let proj_y = self.start.y as f32 + t * delta.y as f32;

            ((point.x as f32 - proj_x).powi(2)
                + (point.y as f32 - proj_y).powi(2))
            .sqrt()
            // (point.x as f32 - )
            // point
            //     .dist_to(
            //         self.start
            //             .add_x_round(t * (self.end.x - self.start.x) as f32)
            //             .add_y_round(t * (self.end.y - self.start.y) as f32),
            //     )
            //     .sqrt()
        }
    }
}

impl<C: Color> StyledDrawable<PrimitiveStyle<C>> for Line {
    type Color = C;
    type Output = ();

    fn draw_styled<D>(
        &self,
        style: &PrimitiveStyle<C>,
        target: &mut D,
    ) -> Result<Self::Output, D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        embedded_graphics::primitives::Line::new(self.start, self.end)
            .draw_styled(style, target)
    }
}

impl<C: Color> StyledAlphaDrawable<PrimitiveStyle<C>> for Line {
    type Color = C;
    type Output = ();

    fn draw_styled_alpha<D>(
        &self,
        style: &PrimitiveStyle<C>,
        target: &mut D,
    ) -> crate::prelude::RenderResult
    where
        D: crate::render::alpha::AlphaDrawTarget<Color = Self::Color>,
    {
        if style.stroke_color.is_none() || style.stroke_width == 0 {
            return Ok(());
        }

        let mut start = self.start;
        let mut end = self.end;
        let mut draw_pixel = |point, blend| {
            target.pixel_alpha(Pixel(point, style.stroke_color.unwrap()), blend)
        };

        let steep = (end.y - start.y).abs() > (end.x - start.x).abs();

        start = start.swap_axes_if(steep);
        end = end.swap_axes_if(steep);

        if start.x > end.x {
            core::mem::swap(&mut start, &mut end);
        }

        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let gradient = if dx > 0 { dy as f32 / dx as f32 } else { 1.0 };

        let width = style.stroke_width as i32;
        let w = width as f32 * (1.0 + gradient.powi(2)).sqrt();
        let draw_width = w.round() as i32;

        let x_end = start.x as f32;
        let y_end = start.y as f32 - (w - 1.0) * 0.5;
        let x_gap = 0.5;
        let x_pixel1 = x_end;
        let y_pixel1 = y_end.floor();
        let fpart = y_end.fract();
        let rfpart = 1.0 - fpart;

        let point = Point::new(x_pixel1 as i32, y_pixel1 as i32);
        draw_pixel(point.swap_axes_if(steep), rfpart * x_gap)?;
        for w in 1..draw_width {
            draw_pixel(point.add_y(w).swap_axes_if(steep), 1.0)?;
        }
        draw_pixel(point.add_y(draw_width).swap_axes_if(steep), fpart * x_gap)?;

        let mut inter_y = y_end + gradient;

        let x_end = end.x as f32;
        let y_end = end.y as f32 - (w - 1.0) * 0.5;
        let x_gap = 0.5;
        let x_pixel2 = x_end;
        let y_pixel2 = y_end.floor();
        let fpart = y_end.fract();
        let rfpart = 1.0 - fpart;

        let point = Point::new(x_pixel2 as i32, y_pixel2 as i32);
        draw_pixel(point.swap_axes_if(steep), rfpart * x_gap)?;
        for w in 1..draw_width {
            draw_pixel(point.add_y(w).swap_axes_if(steep), 1.0)?;
        }
        draw_pixel(point.add_y(draw_width).swap_axes_if(steep), fpart * x_gap)?;

        for x in x_pixel1.round() as i32 + 1..x_pixel2.round() as i32 {
            let fpart = inter_y.fract();
            let rfpart = 1.0 - fpart;
            let y = inter_y.floor() as i32;

            let point = Point::new(x, y);
            draw_pixel(point.swap_axes_if(steep), rfpart)?;
            for w in 1..draw_width {
                draw_pixel(point.add_y(w).swap_axes_if(steep), 1.0)?;
            }
            draw_pixel(point.add_y(draw_width).swap_axes_if(steep), fpart)?;
            inter_y += gradient;
        }

        Ok(())
    }
}
