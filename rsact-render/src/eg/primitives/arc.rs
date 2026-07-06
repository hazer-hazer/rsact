#[allow(unused)]
use crate::FloatExt as _;
use crate::{
    color::Color,
    eg::{framebuf::PackedColor, primitives::EgPrimitive},
    geometry::*,
    output::pixel::Pixel,
    primitives::arc::Arc,
    renderer::{AntiAliasingDisabled, AntiAliasingEnabled, RenderResult},
    style::StrokeAlignment,
};
use core::f32::consts::PI;
use embedded_graphics::{pixelcolor::PixelColor, primitives::StyledDrawable};

impl<C: Color + PixelColor + PackedColor> EgPrimitive<C> for Arc {
    fn draw(
        &self,
        renderer: &mut crate::prelude::EGRenderer<C, AntiAliasingDisabled>,
        style: crate::prelude::DrawStyle<C>,
    ) -> RenderResult {
        embedded_graphics::primitives::Arc::new(
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
        if style.stroke.is_none() || style.stroke_width == 0 {
            return Ok(());
        }

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
        let end_radians = start_radians + sweep_radians;

        let draw_radius = r_outer.ceil() as i32;

        let stroke_color = style.stroke.unwrap();

        for y in -draw_radius..=draw_radius {
            let rx = (r_outer.powi(2) - y.pow(2) as f32).sqrt().ceil() as i32;
            for x in -rx..=rx {
                // Normalize angle into [0, 2*PI). `atan2` returns (-PI, PI],
                // so a single branch is equivalent to `rem_euclid(2*PI)` here —
                // and `rem_euclid` is not on `num_traits::Float` (the `libm`
                // FloatExt backend), only on `micromath::F32Ext`.
                let raw = (y as f32).atan2(x as f32);
                let angle = if raw < 0.0 { raw + 2.0 * PI } else { raw };
                let angle_in_range = if sweep_radians > 0.0 {
                    angle >= start_radians && angle <= end_radians
                } else {
                    angle >= end_radians && angle <= start_radians
                };

                if angle_in_range {
                    let point = Point::new(center.x + x, center.y + y);
                    let dist_sq = x * x + y * y;
                    let dist = (dist_sq as f32).sqrt();

                    if dist >= r_inner && dist <= r_outer {
                        let alpha = (r_outer - dist).min(1.0).max(0.0);
                        renderer
                            .pixel_alpha(Pixel(point, stroke_color), alpha)?;
                    } else if dist > r && dist <= r_outer {
                        let alpha = (dist - r).min(1.0).max(0.0);
                        // TODO
                        renderer
                            .pixel_alpha(Pixel(point, stroke_color), alpha)?;
                    } else if let alpha @ 0.0..1.0 = r_inner - dist {
                        renderer.pixel_alpha(
                            Pixel(point, stroke_color),
                            1.0 - alpha,
                        )?;
                    }
                }
            }
        }

        Ok(())
    }
}
