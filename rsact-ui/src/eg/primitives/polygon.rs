use crate::{
    eg::alpha::{AlphaDrawTarget, StyledAlphaDrawable},
    geometry::{Point, PointExt as _},
    prelude::Color,
    render::primitives::{line::Line, polygon::Polygon},
};
use alloc::vec::Vec;
use embedded_graphics::{
    Pixel,
    prelude::{Dimensions, Primitive, Transform},
    primitives::{PrimitiveStyle, StyledDrawable},
};
impl Dimensions for Polygon {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        let (min, max) = self.bounds();
        embedded_graphics::primitives::Rectangle::new(
            min.into(),
            embedded_graphics::geometry::Size::new(
                (max.x - min.x).abs() as u32,
                (max.y - min.y).abs() as u32,
            ),
        )
    }
}

impl Primitive for Polygon {}

impl Transform for Polygon {
    fn translate(&self, by: embedded_graphics::prelude::Point) -> Self {
        let by: Point = by.into();
        Self::new(
            self.top_left + by,
            self.vertices.iter().copied().map(|p| p + by),
        )
    }

    fn translate_mut(
        &mut self,
        by: embedded_graphics::prelude::Point,
    ) -> &mut Self {
        let by: Point = by.into();
        self.vertices.iter_mut().for_each(|p| *p += by);
        self
    }
}

impl<C: Color + embedded_graphics::prelude::PixelColor>
    StyledDrawable<PrimitiveStyle<C>> for Polygon
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
        if let Some(fill_color) = style.fill_color {
            let (min, max) = self.bounds();
            let fill = (min.y..=max.y)
                .map(|y| {
                    (min.x..=max.x).filter_map(move |x| {
                        let point = Point::new(x, y);
                        if self.contains(point) {
                            Some(Pixel(point.into(), fill_color))
                        } else {
                            None
                        }
                    })
                })
                .flatten();

            target.draw_iter(fill)?;
        }

        if style.stroke_color.is_some() && style.stroke_width > 0 {
            self.lines()
                .try_for_each(|line| line.draw_styled(style, target))?;
        }

        Ok(())
    }
}

impl<C: Color + embedded_graphics::prelude::PixelColor>
    StyledAlphaDrawable<PrimitiveStyle<C>> for Polygon
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
        if let Some(fill_color) = style.fill_color {
            let (min, max) = self.bounds();

            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    let point = Point::new(x, y);
                    if self.contains(point) {
                        target.pixel_alpha(
                            Pixel(point.into(), fill_color),
                            1.0,
                        )?;
                    } else if style.stroke_color.is_none()
                        || style.stroke_width == 0
                    {
                        // Note: Anti-aliasing happens here
                        // TODO: Can optimize?
                        self.lines().try_for_each(|line| {
                            let distance = line.dist_to(point.into());

                            if distance < 1.0 {
                                let alpha = 1.0 - distance;
                                target.pixel_alpha(
                                    Pixel(point.into(), fill_color),
                                    alpha,
                                )?;
                            }
                            Ok(())
                        })?;
                    }
                }
            }
        }

        if style.stroke_color.is_some() && style.stroke_width > 0 {
            self.lines()
                .try_for_each(|line| line.draw_styled_alpha(style, target))?;
        }

        Ok(())
    }
}

impl Polygon {
    pub fn bounds(&self) -> (Point, Point) {
        let (min_x, min_y, max_x, max_y) = self.vertices.iter().fold(
            (i32::MAX, i32::MAX, i32::MIN, i32::MIN),
            |(min_x, min_y, max_x, max_y), point| {
                (
                    min_x.min(point.x),
                    min_y.min(point.y),
                    max_x.max(point.x),
                    max_y.max(point.y),
                )
            },
        );

        (Point::new(min_x, min_y), Point::new(max_x, max_y))
    }

    pub fn count(&self) -> usize {
        self.vertices.len()
    }

    pub fn lines(&self) -> impl Iterator<Item = Line> + '_ {
        self.vertices.iter().copied().enumerate().map(|(i, v)| {
            Line::new(v.into(), self.vertices[(i + 1) % self.count()].into())
        })
    }

    pub fn contains(&self, point: Point) -> bool {
        self.lines().fold(0, |winding_number, line| {
            let ls: Point = line.start.into();
            let le: Point = line.end.into();
            if ls.y <= point.y {
                if le.y > point.y && (le - ls).determinant(point - ls) > 0 {
                    winding_number + 1
                } else {
                    winding_number
                }
            } else if le.y <= point.y && (le - ls).determinant(point - ls) < 0 {
                winding_number - 1
            } else {
                winding_number
            }
        }) != 0
    }
}

// TODO: https://aykevl.nl/2024/02/tinygl-polygon/
