use crate::{
    color::Color,
    geometry::{Point, PointExt as _},
    primitives::{line::Line, polygon::Polygon},
    renderer::{RenderResult, Renderer},
    style::DrawStyle,
};

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

/// Scanline-ish polygon fill plus its edges.
///
/// # This function has no caller, and that is the point of writing it down
///
/// `Renderer::polygon` logs a warning and skips, on both embedded-graphics
/// impls — so this body was already unreachable before PR A, hidden inside a
/// trait impl where dead-code analysis does not look. It is `pub` now, which
/// makes the state visible rather than merely true. The layer split's
/// `raster::polygon` is where it gains a caller: `Rasterizer::polygon`'s
/// default draws, which is the whole of D3 ("no primitive is ever
/// unsupported"), and moving this there supplies the default and fixes the
/// no-op in one step.
///
/// It also drops embedded-graphics entirely — the fill needs a `Renderer`, not
/// a `DrawTarget`, unlike the six delegations beside it. That is why this file
/// is the one that leaves `eg/` when the split lands.
///
/// The fill stays `O(w·h·edges)` on the move. A scanline fill emitting spans is
/// the natural rewrite under the span protocol, but it is an algorithm change
/// and does not belong in a mechanical PR — and the relocated version already
/// draws where today's draws nothing.
// TODO: Review this implementation
pub fn draw<C: Color, R: Renderer<Color = C>>(
    renderer: &mut R,
    polygon: &Polygon,
    style: &DrawStyle<C>,
) -> RenderResult {
    if let Some(fill_color) = style.fill {
        let (min, max) = polygon.bounds();
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let point = Point::new(x, y);
                if polygon.contains(point) {
                    renderer.pixel(point, fill_color)?;
                }
            }
        }
    }

    if style.stroke.is_some() && style.stroke_width > 0 {
        polygon
            .lines()
            .try_for_each(|line| renderer.line(line.from, line.to, style))?;
    }

    Ok(())
}

// TODO: https://aykevl.nl/2024/02/tinygl-polygon/
