use crate::{
    color::Color,
    eg::{framebuf::PackedColor, primitives::EgPrimitive},
    geometry::{Point, PointExt as _},
    output::pixel::Pixel,
    primitives::{line::Line, polygon::Polygon},
    renderer::{
        AntiAliasingDisabled, AntiAliasingEnabled, RenderResult, Renderer,
    },
};
use embedded_graphics::pixelcolor::PixelColor;

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
            let ls: Point = line.from.into();
            let le: Point = line.to.into();
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

impl<C: Color + PixelColor + PackedColor> EgPrimitive<C> for Polygon {
    // TODO: Review this implementation
    fn draw(
        &self,
        renderer: &mut crate::prelude::EGRenderer<C, AntiAliasingDisabled>,
        style: crate::prelude::DrawStyle<C>,
    ) -> RenderResult {
        if let Some(fill_color) = style.fill {
            let (min, max) = self.bounds();
            let fill = (min.y..=max.y).flat_map(|y| {
                (min.x..=max.x).filter_map(move |x| {
                    let point = Point::new(x, y);
                    if self.contains(point) {
                        Some(Pixel(point.into(), fill_color))
                    } else {
                        None
                    }
                })
            });

            renderer.draw_pixels(fill)?;
        }

        if style.stroke.is_some() && style.stroke_width > 0 {
            self.lines().try_for_each(|line| {
                renderer.line(line.from, line.to, &style)
            })?;
        }

        Ok(())
    }

    fn draw_aa(
        &self,
        renderer: &mut crate::prelude::EGRenderer<C, AntiAliasingEnabled>,
        style: crate::prelude::DrawStyle<C>,
    ) -> RenderResult {
        if let Some(fill_color) = style.fill {
            let (min, max) = self.bounds();

            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    let point = Point::new(x, y);
                    if self.contains(point) {
                        renderer.pixel_alpha(Pixel(point, fill_color), 1.0)?;
                    } else if style.stroke.is_none() || style.stroke_width == 0
                    {
                        // Note: Anti-aliasing happens here
                        // TODO: Can optimize?
                        self.lines().try_for_each(|line| {
                            let distance = line.dist_to(point.into());

                            if distance < 1.0 {
                                let alpha = 1.0 - distance;
                                renderer.pixel_alpha(
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

        if style.stroke.is_some() && style.stroke_width > 0 {
            self.lines()
                .try_for_each(|line| line.draw_aa(renderer, style))?;
        }

        Ok(())
    }
}

// TODO: https://aykevl.nl/2024/02/tinygl-polygon/
