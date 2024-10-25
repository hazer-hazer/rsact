use core::f32::consts::PI;

use embedded_graphics::prelude::{Angle, Point};
use num::{
    integer::Roots,
    traits::{float::FloatCore, real::Real},
    Float, Integer,
};

use crate::layout::size::PointExt;

use super::{color::Color, line::line_aa};

pub enum PrimitiveColorKind {
    Stroke,
    Fill,
}

pub fn circle_aa<C: Color, F>(
    center: Point,
    radius: u32,
    stroke_width: u32,
    stroke_color: Option<C>,
    fill_color: Option<C>,
    mut draw_pixel: F,
) where
    F: FnMut(Point, C, f32),
{
    // TODO: Ceil/floor for radiuses
    let r_outer = radius as f32 + stroke_width as f32 / 2.0;
    let r_inner = radius as f32 - stroke_width as f32 / 2.0;
    let r_inner_sq = r_inner * r_inner;
    let r_outer_sq = r_outer * r_outer;

    let draw_radius = r_outer.ceil() as i32;

    for y in -draw_radius..=draw_radius {
        for x in -draw_radius..=draw_radius {
            let point = Point::new(center.x + x, center.y + y);

            let dist_sq = (x * x + y * y) as f32;
            let dist = dist_sq.sqrt();

            // TODO: Antialias circle inside when stroke used
            if let Some(stroke_color) = stroke_color {
                if dist >= r_inner && dist <= r_outer {
                    let alpha = (r_outer - dist).min(1.0).max(0.0);
                    draw_pixel(point, stroke_color, alpha);
                } else if dist >= radius as f32 && dist <= r_outer {
                    let alpha = (dist - radius as f32).min(1.0).max(0.0);
                    draw_pixel(point, stroke_color, alpha);
                }
            }

            if let Some(fill_color) = fill_color {
                // if dist_sq < r_inner_sq {
                //     draw_pixel(point, fill_color, 1.0);
                // } else if dist_sq < r_outer_sq {
                //     let alpha = (r_outer_sq - dist_sq).sqrt();
                //     draw_pixel(point, fill_color, alpha);
                // }
                if dist <= r_inner as f32 {
                    if let Some(stroke_color) = stroke_color {
                        draw_pixel(
                            point,
                            stroke_color.mix(r_inner - dist, fill_color),
                            1.0,
                        );
                    } else {
                        draw_pixel(point, fill_color, r_inner - dist);
                    }
                } else if dist < r_outer
                    && (stroke_width == 0 || stroke_color.is_none())
                {
                    // if dist > radius as f32 {
                    //     let alpha = (r_outer - dist).min(1.0).max(0.0);
                    //     draw_pixel(point, fill_color, alpha);
                    // } else {
                    //     let alpha = (dist - radius as f32).min(1.0).max(0.0);
                    //     draw_pixel(point, fill_color, alpha);
                    // }
                }
            }

            // if let Some(stroke_color) = stroke_color {
            //     if dist_sq > r_inner_sq && dist_sq <= r_outer_sq {
            //         let dist = (dist_sq as f32).sqrt();
            //         let alpha = if dist > r_inner as f32 {
            //             (r_outer as f32 - dist).min(1.0).max(0.0)
            //         } else if dist < r_outer as f32 {
            //             (dist - r_inner as f32).min(1.0).max(0.0)
            //         } else {
            //             continue;
            //         };

            //         draw_pixel(point, stroke_color, alpha);
            //     }
            // }
        }
    }
}

pub fn arc_aa<C: Color, F: FnMut(Point, C, f32)>(
    center: Point,
    radius: u32,
    start_angle: Angle,
    sweep_angle: Angle,
    stroke_color: Option<C>,
    stroke_width: u32,
    fill_color: Option<C>,
    mut draw_pixel: F,
) {
    let r = radius as f32;
    let r_outer = r;
    let r_inner = r - stroke_width as f32;

    let r_inner_sq = r_inner * r_inner;
    let r_outer_sq = r_outer * r_outer;

    let start_radians = start_angle.to_radians();
    let sweep_radians = sweep_angle.to_radians();
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

                if let Some(fill_color) = fill_color {
                    if dist <= r_inner {
                        draw_pixel(point, fill_color, 1.0);
                    } else if dist <= r_outer && stroke_width == 0 {
                        let alpha = (r_outer - dist).min(1.0).max(0.0);
                        // TODO: Check this case
                        draw_pixel(point, fill_color, alpha);
                    }
                }

                if let Some(stroke_color) = stroke_color {
                    if dist >= r_inner && dist <= r_outer {
                        let alpha = (r_outer - dist).min(1.0).max(0.0);
                        draw_pixel(point, stroke_color, alpha);
                    } else if dist > r && dist <= r_outer {
                        let alpha = (dist - r).min(1.0).max(0.0);
                        // TODO
                        draw_pixel(point, stroke_color, alpha);
                    }
                }
            }
        }
    }

    if let Some(stroke_color) = stroke_color {
        if stroke_width > 0 {
            let end_point = (end_radians).sin_cos();
            line_aa(
                center,
                center
                    .add_x_round(end_point.1 * r_inner)
                    .add_y_round(end_point.0 * r_inner),
                stroke_width,
                |point, blend| {
                    draw_pixel(point, stroke_color, blend);
                },
            );
            let start_point = start_radians.sin_cos();
            line_aa(
                center,
                center
                    .add_x_round(start_point.1 * r_inner)
                    .add_y_round(start_point.0 * r_inner),
                stroke_width,
                |point, blend| draw_pixel(point, stroke_color, blend),
            );
        }
    }
}
