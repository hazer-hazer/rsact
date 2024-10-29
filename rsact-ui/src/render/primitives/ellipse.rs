use embedded_graphics::{
    prelude::{Dimensions, Point, Primitive, Transform},
    primitives::{PrimitiveStyle, Rectangle, StyledDrawable},
    Pixel,
};
use num::{integer::Roots, pow::Pow};

use crate::{
    layout::size::PointExt,
    prelude::{Color, Size},
    render::alpha::StyledAlphaDrawable,
};

use super::circle::Circle;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ellipse {
    top_left: Point,
    size: Size,
}

impl Ellipse {
    pub fn new(top_left: Point, size: Size) -> Self {
        Self { top_left, size }
    }
}

impl Primitive for Ellipse {}

impl Transform for Ellipse {
    fn translate(&self, by: Point) -> Self {
        Self::new(self.top_left + by, self.size)
    }

    fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.top_left += by;
        self
    }
}

impl Dimensions for Ellipse {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        Rectangle::new(self.top_left, self.size.into())
    }
}

impl<C: Color> StyledDrawable<PrimitiveStyle<C>> for Ellipse {
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
        embedded_graphics::primitives::Ellipse::new(
            self.top_left,
            self.size.into(),
        )
        .draw_styled(style, target)
    }
}

impl<C: Color> StyledAlphaDrawable<PrimitiveStyle<C>> for Ellipse {
    type Color = C;
    type Output = ();

    fn draw_styled_alpha<D>(
        &self,
        style: &PrimitiveStyle<C>,
        target: &mut D,
    ) -> crate::prelude::DrawResult
    where
        D: crate::render::alpha::AlphaDrawTarget<Color = Self::Color>,
    {
        if self.size.width == self.size.height {
            return Circle::new(self.top_left, self.size.width)
                .draw_styled_alpha(style, target);
        }

        // TODO: StrokeAlignment

        let center = self.top_left
            + Point::new(self.size.width as i32, self.size.height as i32) / 2;

        let r = self.size.map(|axis| axis.div_ceil(2));
        // Note: Xiaolin Wu's algorithm draws line at center of the radius, so think about it already being centered on ellipse line
        let stroke_size = Size::new_equal(style.stroke_width);
        let half_stroke_size = Size::new_equal(style.stroke_width / 2);
        // let half_ceil_stroke_size =
        //     Size::new_equal(style.stroke_width.div_ceil(2));
        let (r_stroke, r_fill) = match style.stroke_alignment {
            embedded_graphics::primitives::StrokeAlignment::Inside => {
                (r - stroke_size, r - half_stroke_size)
            },
            embedded_graphics::primitives::StrokeAlignment::Center => {
                (r, r - half_stroke_size)
            },
            embedded_graphics::primitives::StrokeAlignment::Outside => {
                (r + half_stroke_size, r + half_stroke_size)
            },
        };

        // Naive //
        // let rx = self.size.width as i32 / 2;
        // let ry = self.size.height as i32 / 2;

        // let area = rx * ry;
        // let area_sq = area.pow(2);

        // for y in -ry..=ry {
        //     for x in -rx..=rx {
        //         let point = center + Point::new(x, y);
        //         let pixel = Pixel(point, style.stroke_color.unwrap());

        //         // Multiply terms by area_sq to avoid FP arithmetics
        //         let dist_sq = x.pow(2) * ry.pow(2) + y.pow(2) * rx.pow(2);
        //         if dist_sq <= area_sq {
        //             // let alpha = (dist_sq as f32 / area_sq as f32);

        //             target.pixel_alpha(pixel, 1.0)?;
        //         }
        //     }
        // }

        // Wu //
        // let rx_sq = rx.pow(2) as f32;
        // let ry_sq = ry.pow(2) as f32;

        let r_stroke_sq = r_stroke.map(|r| r.pow(2));
        let r_fill_area = r_fill.width * r_fill.height;
        let fill_area_sq = r_fill_area.pow(2);
        let stroke_offset_x = r_stroke.width as i32 - r_fill.width as i32;
        let stroke_offset_y = r_stroke.height as i32 - r_fill.height as i32;

        // if let Some(fill_color) = style.fill_color {
        //     for x in -(r_fill.width as i32)..=r_fill.width as i32 {
        //         for y in -(r_fill.height as i32)..=r_fill.height as i32 {
        //             let dist_sq = x.pow(2) as u32 * r_stroke_sq.height
        //                 + y.pow(2) as u32 * r_stroke_sq.width;

        //             if dist_sq <= fill_area_sq {
        //                 target.pixel_alpha(
        //                     Pixel(center + Point::new(x, y), fill_color),
        //                     1.0,
        //                 )?;
        //             }
        //         }
        //     }
        // }

        let set_point = |target: &mut D, delta: Point, color: C, blend: f32| {
            delta.each_mirror().try_for_each(|delta| {
                target.pixel_alpha(
                    // TODO: Remove unwrap
                    Pixel(center + delta, color),
                    blend,
                )
            })
        };

        // // Can avoid float usage?
        // let quart = (r_stroke_sq.width as f32
        //     / (r_stroke_sq.width as f32 + r_stroke_sq.height as f32).sqrt())
        // .round() as i32;

        for x in 0..=r_stroke.width as i32 {
            let y = r_stroke.height as f32
                * (1.0 - x.pow(2) as f32 / r_stroke_sq.width as f32).sqrt();

            if let Some(fill_color) = style.fill_color {
                for y in 0..y.floor() as i32 {
                    // if dist_sq <= fill_area_sq {
                    // target.pixel_alpha(
                    //     Pixel(center + Point::new(x, y), fill_color),
                    //     1.0,
                    // )?;
                    set_point(target, Point::new(x, y), fill_color, 1.0)?;
                    // target.pixel_alpha(
                    //     Pixel(center + Point::new(x, -y), fill_color),
                    //     1.0,
                    // )?;
                    // }
                }
            }

            // if let (stroke_width @ 1.., Some(_)) =
            //     (style.stroke_width, style.stroke_color)
            // {
            //     let alpha = y.fract();

            //     let point = Point::new(x, y.floor() as i32 - stroke_offset_y);
            //     stroke_point(target, point, 1.0 - alpha)?;
            //     for w in 1..stroke_width as i32 {
            //         stroke_point(target, point.add_y(w), 1.0)?;
            //     }
            //     stroke_point(target, point.add_y(stroke_width as i32), alpha)?;
            // }
        }

        // let quart = (r_stroke_sq.height as f32
        //     / (r_stroke_sq.width as f32 + r_stroke_sq.height as f32).sqrt())
        // .round() as i32;

        for y in 0..=r_stroke.height as i32 {
            let x = r_stroke.width as f32
                * (1.0 - y.pow(2) as f32 / r_stroke_sq.height as f32).sqrt();

            // if let Some(fill_color) = style.fill_color {
            //     let w = x.round() as i32;
            //     for x in -w..w {
            //         let dist_sq = y.pow(2) as u32 * r_stroke_sq.width
            //             + x.pow(2) as u32 * r_stroke_sq.height;

            //         if dist_sq <= fill_area_sq {
            //             target.pixel_alpha(
            //                 Pixel(center + Point::new(x, y), fill_color),
            //                 1.0,
            //             )?;
            //         }
            //     }
            // }

            // if let (stroke_width @ 1.., Some(_)) =
            //     (style.stroke_width, style.stroke_color)
            // {
            //     let alpha = x.fract();

            //     let point = Point::new(x.floor() as i32 - stroke_offset_x, y);
            //     stroke_point(target, point, 1.0 - alpha)?;
            //     for w in 1..stroke_width as i32 {
            //         stroke_point(target, point.add_x(w), 1.0)?;
            //     }
            //     stroke_point(target, point.add_x(stroke_width as i32), alpha)?;
            // }
        }

        Ok(())
    }
}
