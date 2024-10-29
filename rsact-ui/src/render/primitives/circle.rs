use crate::{prelude::Color, render::alpha::StyledAlphaDrawable};
use embedded_graphics::{
    prelude::{Dimensions, Point, Primitive, Transform},
    primitives::{PrimitiveStyle, Rectangle, StyledDrawable},
    Pixel,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    top_left: Point,
    diameter: u32,
}

impl Circle {
    pub fn new(top_left: Point, diameter: u32) -> Self {
        Self { top_left, diameter }
    }
}

impl Dimensions for Circle {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        Rectangle::new(
            self.top_left,
            embedded_graphics::geometry::Size::new_equal(self.diameter),
        )
    }
}

impl Primitive for Circle {}

impl Transform for Circle {
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

impl<C: Color> StyledDrawable<PrimitiveStyle<C>> for Circle {
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
        embedded_graphics::primitives::Circle::new(self.top_left, self.diameter)
            .draw_styled(style, target)
    }
}

impl<C: Color> StyledAlphaDrawable<PrimitiveStyle<C>> for Circle {
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

        let draw_radius = r_outer.ceil() as i32;

        for y in -draw_radius..=draw_radius {
            for x in -draw_radius..=draw_radius {
                let point = Point::new(center.x + x, center.y + y);

                let dist_sq = (x * x + y * y) as f32;
                let dist = dist_sq.sqrt();

                // TODO: Antialias circle inside when stroke used
                if let Some(stroke_color) = style.stroke_color {
                    if style.stroke_width > 0 {
                        if dist >= r_inner && dist <= r_outer {
                            let alpha = (r_outer - dist).min(1.0).max(0.0);
                            target.pixel_alpha(
                                Pixel(point, stroke_color),
                                alpha,
                            )?;
                        } else if dist >= radius as f32 && dist <= r_outer {
                            let alpha =
                                (dist - radius as f32).min(1.0).max(0.0);
                            target.pixel_alpha(
                                Pixel(point, stroke_color),
                                alpha,
                            )?;
                        }
                    }
                }

                if let Some(fill_color) = style.fill_color {
                    if dist <= r_inner as f32 {
                        if let Some(stroke_color) = style.stroke_color {
                            target.pixel_alpha(
                                Pixel(
                                    point,
                                    stroke_color
                                        .mix(r_inner - dist, fill_color),
                                ),
                                1.0,
                            )?;
                        } else {
                            target.pixel_alpha(
                                Pixel(point, fill_color),
                                r_inner - dist,
                            )?;
                        }
                    } else if dist < r_outer
                        && (style.stroke_width == 0
                            || style.stroke_color.is_none())
                    {
                        // TODO
                        // if dist > radius as f32 {
                        //     let alpha = (r_outer - dist).min(1.0).max(0.0);
                        //     draw_pixel(point, fill_color, alpha);
                        // } else {
                        //     let alpha = (dist - radius as f32).min(1.0).max(0.0);
                        //     draw_pixel(point, fill_color, alpha);
                        // }
                    }
                }
            }
        }

        Ok(())
    }
}
