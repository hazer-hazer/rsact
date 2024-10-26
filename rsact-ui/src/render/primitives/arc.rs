use crate::{
    layout::size::PointExt, prelude::Color, render::alpha::StyledAlphaDrawable,
};
use core::f32::consts::PI;
use embedded_graphics::{
    prelude::{Angle, Dimensions, Point, Primitive, Transform},
    primitives::{PrimitiveStyle, Rectangle, StyledDrawable},
    Pixel,
};

use super::line::Line;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    top_left: Point,
    diameter: u32,
    start_angle: Angle,
    sweep_angle: Angle,
}

impl Arc {
    pub fn new(
        top_left: Point,
        diameter: u32,
        start_angle: Angle,
        sweep_angle: Angle,
    ) -> Self {
        Self { top_left, diameter, start_angle, sweep_angle }
    }
}

impl Dimensions for Arc {
    fn bounding_box(&self) -> Rectangle {
        // TODO: Is diameter right for size?
        Rectangle::new(
            self.top_left,
            embedded_graphics::geometry::Size::new_equal(self.diameter),
        )
    }
}

impl Primitive for Arc {}

impl Transform for Arc {
    fn translate(&self, by: Point) -> Self {
        let mut new = *self;
        new.top_left += by;
        new
    }

    fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.top_left += by;
        self
    }
}

impl<C: Color> StyledDrawable<PrimitiveStyle<C>> for Arc {
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
        embedded_graphics::primitives::Sector::new(
            self.top_left,
            self.diameter,
            self.start_angle,
            self.sweep_angle,
        )
        .draw_styled(style, target)
    }
}

impl<C: Color> StyledAlphaDrawable<PrimitiveStyle<C>> for Arc {
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
        let radius = self.diameter as i32 / 2;
        let center = self.top_left + Point::new_equal(radius);
        let r = radius as f32;
        let r_outer = r;
        let r_inner = r - style.stroke_width as f32;

        let start_radians = self.start_angle.to_radians();
        let sweep_radians = self.sweep_angle.to_radians();
        let end_radians = start_radians + sweep_radians;

        let draw_radius = r_outer.ceil() as i32;

        for y in -draw_radius..=draw_radius {
            let rx = (r_outer.powi(2) - y.pow(2) as f32).sqrt().ceil() as i32;
            for x in -rx..=rx {
                // Normalize angle
                let angle = (y as f32).atan2(x as f32).rem_euclid(2.0 * PI);
                let angle_in_range = if sweep_radians > 0.0 {
                    angle >= start_radians && angle <= end_radians
                } else {
                    angle >= end_radians && angle <= start_radians
                };

                if angle_in_range {
                    let point = Point::new(center.x + x, center.y + y);
                    let dist_sq = x * x + y * y;
                    let dist = (dist_sq as f32).sqrt();

                    if let Some(fill_color) = style.fill_color {
                        if dist <= r_inner {
                            target
                                .pixel_alpha(Pixel(point, fill_color), 1.0)?;
                        } else if dist <= r_outer && style.stroke_width == 0 {
                            let alpha = (r_outer - dist).min(1.0).max(0.0);
                            // TODO: Check this case
                            target
                                .pixel_alpha(Pixel(point, fill_color), alpha)?;
                        }
                    }

                    if let Some(stroke_color) = style.stroke_color {
                        if dist >= r_inner && dist <= r_outer {
                            let alpha = (r_outer - dist).min(1.0).max(0.0);
                            target.pixel_alpha(
                                Pixel(point, stroke_color),
                                alpha,
                            )?;
                        } else if dist > r && dist <= r_outer {
                            let alpha = (dist - r).min(1.0).max(0.0);
                            // TODO
                            target.pixel_alpha(
                                Pixel(point, stroke_color),
                                alpha,
                            )?;
                        }
                    }
                }
            }
        }

        if style.stroke_color.is_some() && style.stroke_width > 0 {
            let end_point = (end_radians).sin_cos();
            Line::new(
                center,
                center
                    .add_x_round(end_point.1 * r_inner)
                    .add_y_round(end_point.0 * r_inner),
            )
            .draw_styled_alpha(style, target)?;
            let start_point = start_radians.sin_cos();
            Line::new(
                center,
                center
                    .add_x_round(start_point.1 * r_inner)
                    .add_y_round(start_point.0 * r_inner),
            )
            .draw_styled_alpha(style, target)?;
        }

        Ok(())
    }
}
