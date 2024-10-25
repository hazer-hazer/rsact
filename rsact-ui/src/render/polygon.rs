use super::{color::Color, line::Line};
use crate::{layout::size::PointExt, render::line::line_aa, style::ColorStyle};
use alloc::vec::Vec;
use embedded_graphics::{
    prelude::{Dimensions, Point, Primitive, Transform},
    primitives::{
        PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, Styled,
        StyledDrawable,
    },
};

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

pub fn polygon_aa<C: Color, F: FnMut(Point, C, f32)>(
    styled: &Styled<Polygon, PrimitiveStyle<C>>,
    mut draw_pixel: F,
) {
    let polygon = &styled.primitive;
    let style = &styled.style;

    let (min, max) = polygon.bounds();

    if let Some(fill_color) = style.fill_color {
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let point = Point::new(x, y);
                if polygon.contains(point) {
                    draw_pixel(point, fill_color, 1.0);
                } else if style.stroke_color.is_none()
                    || style.stroke_width == 0
                {
                    // Note: Anti-aliasing happens here
                    // TODO: Can optimize?
                    polygon.lines().for_each(|line| {
                        let distance = line.dist_to(point);

                        if distance < 1.0 {
                            // assert!(false);
                            let alpha = 1.0 - distance;
                            draw_pixel(point, fill_color, alpha);
                        }
                    });
                }
            }
        }
    }

    if let Some(stroke_color) = style.stroke_color {
        polygon.lines().for_each(|line| {
            line_aa(
                line.start,
                line.end,
                style.stroke_width,
                |point, blend| {
                    draw_pixel(point, stroke_color, blend);
                },
            );
        });
    }
}
