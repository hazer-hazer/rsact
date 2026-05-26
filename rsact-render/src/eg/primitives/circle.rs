use crate::{
    color::Color,
    eg::{framebuf::PackedColor, primitives::EgPrimitive},
    geometry::{Axial as _, Point},
    output::pixel::Pixel,
    primitives::circle::Circle,
    renderer::RenderResult,
    style::StrokeAlignment,
};
use embedded_graphics::{pixelcolor::PixelColor, primitives::StyledDrawable};

impl<C: Color + PixelColor + PackedColor> EgPrimitive<C> for Circle {
    fn draw(
        &self,
        renderer: &mut crate::prelude::EGRenderer<
            C,
            crate::renderer::AntiAliasingDisabled,
        >,
        style: crate::prelude::DrawStyle<C>,
    ) -> RenderResult {
        embedded_graphics::primitives::Circle::new(
            self.top_left.into(),
            self.diameter,
        )
        .draw_styled(&style.into_primitive_style(), renderer)
    }

    fn draw_aa(
        &self,
        renderer: &mut crate::prelude::EGRenderer<
            C,
            crate::renderer::AntiAliasingEnabled,
        >,
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

        let draw_radius = r_outer.ceil() as i32;

        for y in -draw_radius..=draw_radius {
            for x in -draw_radius..=draw_radius {
                let point = Point::new(center.x + x, center.y + y);

                let dist_sq = (x * x + y * y) as f32;
                let dist = dist_sq.sqrt();

                // TODO: Antialias circle inside when stroke used
                if let Some(stroke_color) = style.stroke {
                    if style.stroke_width > 0 {
                        if dist >= r_inner && dist <= r_outer {
                            let alpha = (r_outer - dist).min(1.0).max(0.0);
                            renderer.pixel_alpha(
                                Pixel(point, stroke_color),
                                alpha,
                            )?;
                        } else if dist >= radius as f32 && dist <= r_outer {
                            let alpha =
                                (dist - radius as f32).min(1.0).max(0.0);
                            renderer.pixel_alpha(
                                Pixel(point, stroke_color),
                                alpha,
                            )?;
                        }
                    }
                }

                if let Some(fill_color) = style.fill {
                    if dist <= r_inner as f32 {
                        if let Some(stroke_color) = style.stroke {
                            renderer.pixel_alpha(
                                Pixel(
                                    point,
                                    stroke_color
                                        .mix(r_inner - dist, fill_color),
                                ),
                                1.0,
                            )?;
                        } else {
                            renderer.pixel_alpha(
                                Pixel(point, fill_color),
                                r_inner - dist,
                            )?;
                        }
                    } else if dist <= r_outer
                        && (style.stroke_width == 0 || style.stroke.is_none())
                    {
                        // TODO
                        if dist > radius as f32 {
                            let alpha = (r_outer - dist).clamp(0.0, 1.0);
                            renderer
                                .pixel_alpha(Pixel(point, fill_color), alpha)?;
                        } else {
                            let alpha = (dist - radius as f32).clamp(0.0, 1.0);
                            renderer
                                .pixel_alpha(Pixel(point, fill_color), alpha)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
