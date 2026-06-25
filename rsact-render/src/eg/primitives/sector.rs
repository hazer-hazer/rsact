use crate::{
    color::Color,
    eg::{framebuf::PackedColor, primitives::EgPrimitive},
    geometry::{Axial as _, Point},
    output::pixel::Pixel,
    primitives::{line::Line, sector::Sector},
    renderer::{AntiAliasingDisabled, AntiAliasingEnabled, RenderResult},
    style::StrokeAlignment,
};
use core::f32;
use embedded_graphics::{
    pixelcolor::PixelColor, prelude::Angle, primitives::StyledDrawable,
};

impl<C: Color + PixelColor + PackedColor> EgPrimitive<C> for Sector {
    fn draw(
        &self,
        renderer: &mut crate::prelude::EGRenderer<C, AntiAliasingDisabled>,
        style: crate::prelude::DrawStyle<C>,
    ) -> RenderResult {
        embedded_graphics::primitives::Sector::new(
            self.top_left.into(),
            self.diameter,
            self.start.into(),
            self.sweep.into(),
        )
        .draw_styled(&style.into_primitive_style(), renderer)
    }

    fn draw_aa(
        &self,
        renderer: &mut crate::prelude::EGRenderer<C, AntiAliasingEnabled>,
        style: crate::prelude::DrawStyle<C>,
    ) -> RenderResult {
        let radius = self.diameter as i32 / 2;
        let center = self.top_left + Point::new_equal(radius);
        let r = radius as f32;
        let (r_outer, r_inner) = match style.stroke_alignment {
            StrokeAlignment::Inside => (r, r - style.stroke_width as f32),
            StrokeAlignment::Center => (
                r + style.stroke_width.div_ceil(2) as f32,
                r - (style.stroke_width / 2) as f32,
            ),
            StrokeAlignment::Outside => (r + style.stroke_width as f32, r),
        };

        let start_radians = self.start.to_radians();
        let sweep_radians = self.sweep.to_radians();
        let end_angle = Angle::from_radians(start_radians + sweep_radians);
        let end_radians = end_angle.to_radians();

        let draw_radius = r_outer.ceil() as i32;

        for y in -draw_radius..=draw_radius {
            // let rx = (r_outer.powi(2) - y.pow(2) as f32).sqrt().ceil() as
            // i32;
            for x in -draw_radius..=draw_radius {
                // Normalize angle
                let angle = (y as f32)
                    .atan2(x as f32)
                    .rem_euclid(2.0 * f32::consts::PI);
                let angle_in_range = if sweep_radians > 0.0 {
                    angle >= start_radians && angle <= end_radians
                } else {
                    angle >= end_radians && angle <= start_radians
                };

                // TODO: Antialias inner angle line

                if angle_in_range {
                    let point = Point::new(center.x + x, center.y + y);
                    let dist_sq = x * x + y * y;
                    let dist = (dist_sq as f32).sqrt();

                    if let Some(fill_color) = style.fill {
                        if dist <= r_inner {
                            let alpha = (r_inner - dist).clamp(0.0, 1.0);
                            renderer
                                .pixel_alpha(Pixel(point, fill_color), alpha)?;
                        } else if dist <= r_outer
                            && (style.stroke_width == 0
                                || style.stroke.is_none())
                        {
                            let alpha =
                                1.0 - (r_outer - dist).min(1.0).max(0.0);
                            // TODO: Check this case
                            renderer
                                .pixel_alpha(Pixel(point, fill_color), alpha)?;
                        }
                    }

                    // TODO: Invalid logic? stroke width is not used
                    // Note: Stroke width affects r_inner and r_outer
                    if let Some(stroke_color) = style.stroke {
                        if dist >= r_inner && dist <= r_outer {
                            let alpha = (r_outer - dist).min(1.0).max(0.0);
                            renderer.pixel_alpha(
                                Pixel(point, stroke_color),
                                alpha,
                            )?;
                        } else if dist > r && dist <= r_outer {
                            let alpha = (dist - r).min(1.0).max(0.0);
                            // TODO
                            renderer.pixel_alpha(
                                Pixel(point, stroke_color),
                                alpha,
                            )?;
                        } else if let alpha @ 0.0..1.0 = r_inner - dist {
                            renderer.pixel_alpha(
                                Pixel(point, stroke_color),
                                1.0 - alpha,
                            )?;
                        }
                    }
                }
            }
        }

        if style.stroke.is_some() && style.stroke_width > 0 {
            Line::with_angle(center, end_angle.into(), r)
                .draw_aa(renderer, style)?;
            Line::with_angle(center, self.start.into(), r)
                .draw_aa(renderer, style)?;
        }

        Ok(())
    }
}
