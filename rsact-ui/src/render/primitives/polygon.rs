use crate::{
    layout::size::PointExt as _, prelude::Color,
    render::alpha::StyledAlphaDrawable,
};
use alloc::vec::Vec;
use embedded_graphics::{
    prelude::{Dimensions, Point, Primitive, Transform},
    primitives::{PrimitiveStyle, Rectangle, StyledDrawable, Triangle},
    Pixel,
};

use super::line::Line;

// pub struct PolygonStyle<C: Color> {
//     fill_color: Option<C>,
//     stroke_color: Option<C>,
// }

pub struct Polygon {
    vertices: Vec<Point>,
}

impl Dimensions for Polygon {
    fn bounding_box(&self) -> Rectangle {
        let (min, max) = self.bounds();
        Rectangle::new(
            min,
            embedded_graphics_core::geometry::Size::new(
                (max.x - min.x).abs() as u32,
                (max.y - min.y).abs() as u32,
            ),
        )
    }
}

impl Primitive for Polygon {}

impl Transform for Polygon {
    fn translate(&self, by: Point) -> Self {
        Self::new(self.vertices.iter().copied().map(|point| point + by))
    }

    fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.vertices.iter_mut().for_each(|point| *point += by);
        self
    }
}

impl<C: Color> StyledDrawable<PrimitiveStyle<C>> for Polygon {
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
                            Some(Pixel(point, fill_color))
                        } else {
                            None
                        }
                    })
                })
                .flatten();

            target.draw_iter(fill)?;
        }

        if style.stroke_color.is_some() && style.stroke_width > 0 {
            self.lines().try_for_each(|line| {
                Line::new(line.start, line.end).draw_styled(style, target)
            })?;
        }

        Ok(())
    }
}

impl<C: Color> StyledAlphaDrawable<PrimitiveStyle<C>> for Polygon {
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
        if let Some(fill_color) = style.fill_color {
            let (min, max) = self.bounds();

            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    let point = Point::new(x, y);
                    if self.contains(point) {
                        target.pixel_alpha(Pixel(point, fill_color), 1.0)?;
                    } else if style.stroke_color.is_none()
                        || style.stroke_width == 0
                    {
                        // Note: Anti-aliasing happens here
                        // TODO: Can optimize?
                        self.lines().try_for_each(|line| {
                            let distance = line.dist_to(point);

                            if distance < 1.0 {
                                // assert!(false);
                                let alpha = 1.0 - distance;
                                target.pixel_alpha(
                                    Pixel(point, fill_color),
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
            self.lines().try_for_each(|line| {
                Line::new(line.start, line.end).draw_styled_alpha(style, target)
            })?;
        }

        Ok(())
    }
}

impl Polygon {
    pub fn new(vertices: impl IntoIterator<Item = Point>) -> Self {
        let points: Vec<Point> = vertices.into_iter().collect();

        assert!(points.len() >= 3, "Polygon must contain at least 3 vertices");
        assert!(points.first() != points.last(), "Polygon must not be closed");

        Self { vertices: points }
    }

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
        self.vertices
            .iter()
            .copied()
            .enumerate()
            .map(|(i, v)| Line::new(v, self.vertices[(i + 1) % self.count()]))
    }

    // pub fn triangles(&self) -> impl Iterator<Item = Triangle> + '_ {
    //     self.vertices.iter().copied().enumerate().step_by(3).map(|(i, v)| {
    //         Triangle::new(
    //             v,
    //             self.vertices[(i + 1) % self.count()],
    //             self.vertices[(i + 2) % self.count()],
    //         )
    //     })
    // }

    pub fn contains(&self, point: Point) -> bool {
        self.lines().fold(0, |winding_number, line| {
            if line.start.y <= point.y {
                if line.end.y > point.y
                    && (line.end - line.start).determinant(point - line.start)
                        > 0
                {
                    winding_number + 1
                } else {
                    winding_number
                }
            } else if line.end.y <= point.y
                && (line.end - line.start).determinant(point - line.start) < 0
            {
                winding_number - 1
            } else {
                winding_number
            }
        }) != 0
    }
}

// TODO: https://aykevl.nl/2024/02/tinygl-polygon/
