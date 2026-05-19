use crate::{
    eg::alpha::{AlphaDrawTarget, StyledAlphaDrawable},
    geometry::Point,
    prelude::Color,
    render::primitives::{line::Line, sector::Sector},
};
use core::{f32, ops::Rem as _};
use embedded_graphics::{
    Pixel,
    geometry::Point as EgPoint,
    prelude::{Angle, Dimensions, Primitive, Transform},
    primitives::{PrimitiveStyle, StyledDrawable},
};
use num::Float;

impl Transform for Sector {
    fn translate(&self, by: EgPoint) -> Self {
        let mut new = *self;
        new.top_left += by.into();
        new
    }

    fn translate_mut(&mut self, by: EgPoint) -> &mut Self {
        self.top_left += by.into();
        self
    }
}

impl Primitive for Sector {}

impl Dimensions for Sector {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        // TODO: Is diameter right for size?
        embedded_graphics::primitives::Rectangle::new(
            self.top_left.into(),
            embedded_graphics::geometry::Size::new_equal(self.diameter),
        )
    }
}

impl<C: Color + embedded_graphics::prelude::PixelColor>
    StyledDrawable<PrimitiveStyle<C>> for Sector
{
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
            self.top_left.into(),
            self.diameter,
            self.start.into(),
            self.sweep.into(),
        )
        .draw_styled(style, target)
    }
}

impl<C: Color + embedded_graphics::prelude::PixelColor>
    StyledAlphaDrawable<PrimitiveStyle<C>> for Sector
{
    type Color = C;
    type Output = ();

    fn draw_styled_alpha<D>(
        &self,
        style: &PrimitiveStyle<C>,
        target: &mut D,
    ) -> crate::prelude::RenderResult
    where
        D: AlphaDrawTarget<Color = Self::Color>,
    {
        let radius = self.diameter as i32 / 2;
        let center = self.top_left + Point::new_equal(radius);
        let r = radius as f32;
        let (r_outer, r_inner) = match style.stroke_alignment {
            embedded_graphics::primitives::StrokeAlignment::Inside => {
                (r, r - style.stroke_width as f32)
            },
            embedded_graphics::primitives::StrokeAlignment::Center => (
                r + style.stroke_width.div_ceil(2) as f32,
                r - (style.stroke_width / 2) as f32,
            ),
            embedded_graphics::primitives::StrokeAlignment::Outside => {
                (r + style.stroke_width as f32, r)
            },
        };

        let start_radians = self.start.to_radians();
        let sweep_radians = self.sweep.to_radians();
        let end_angle = Angle::from_radians(start_radians + sweep_radians);
        let end_radians = end_angle.to_radians();

        let draw_radius = r_outer.ceil() as i32;

        for y in -draw_radius..=draw_radius {
            // let rx = (r_outer.powi(2) - y.pow(2) as f32).sqrt().ceil() as i32;
            for x in -draw_radius..=draw_radius {
                // Normalize angle
                let angle =
                    (y as f32).atan2(x as f32).rem(2.0 * f32::consts::PI);
                let angle_in_range = if sweep_radians > 0.0 {
                    angle >= start_radians && angle <= end_radians
                } else {
                    angle >= end_radians && angle <= start_radians
                };

                // TODO: Antialias inner angle line

                if angle_in_range {
                    let point = EgPoint::new(center.x + x, center.y + y);
                    let dist_sq = x * x + y * y;
                    let dist = (dist_sq as f32).sqrt();

                    if let Some(fill_color) = style.fill_color {
                        if dist <= r_inner {
                            let alpha = (r_inner - dist).clamp(0.0, 1.0);
                            target
                                .pixel_alpha(Pixel(point, fill_color), alpha)?;
                        } else if dist <= r_outer
                            && (style.stroke_width == 0
                                || style.stroke_color.is_none())
                        {
                            let alpha =
                                1.0 - (r_outer - dist).min(1.0).max(0.0);
                            // TODO: Check this case
                            target
                                .pixel_alpha(Pixel(point, fill_color), alpha)?;
                        }
                    }

                    // TODO: Invalid logic? stroke width is not used
                    // Note: Stroke width affects r_inner and r_outer
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
                        } else if let alpha @ 0.0..1.0 = r_inner - dist {
                            target.pixel_alpha(
                                Pixel(point, stroke_color),
                                1.0 - alpha,
                            )?;
                        }
                    }
                }
            }
        }

        if style.stroke_color.is_some() && style.stroke_width > 0 {
            Line::with_angle(center.into(), end_angle.into(), r)
                .draw_styled_alpha(style, target)?;
            Line::with_angle(center.into(), self.start.into(), r)
                .draw_styled_alpha(style, target)?;
        }

        Ok(())
    }
}
