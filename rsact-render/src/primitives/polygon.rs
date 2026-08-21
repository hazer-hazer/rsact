use crate::geometry::{Point, PointExt as _, Rect, Size};
use alloc::vec::Vec;

// TODO: Cannot go non-vec because polygon is used in the canvas. Maybe we get
// rid of the polygon at all, it is a strange primitive.
#[derive(Clone, PartialEq, Debug)]
pub struct Polygon {
    pub translation: Point,
    pub vertices: Vec<Point>,
}

impl Polygon {
    pub fn new(
        translation: Point,
        vertices: impl IntoIterator<Item = Point>,
    ) -> Self {
        let vertices: Vec<Point> = vertices.into_iter().collect();
        assert!(
            vertices.len() >= 3,
            "Polygon must contain at least 3 vertices"
        );
        assert!(
            vertices.first() != vertices.last(),
            "Polygon must not be closed"
        );

        Self { translation, vertices }
    }

    pub fn translate(&self, by: Point) -> Self {
        let mut new = self.clone();
        new.translation += by;
        new
    }

    pub fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.translation += by;
        self
    }
}

// ─────────────────────────────────────────────── the geometry, over bare points
//
// Free functions rather than methods, because the drawing side has a `&[Point]`
// and not a `Polygon`: `Renderer::polygon` and `Rasterizer::polygon` both take a
// slice, so building a `Polygon` (which owns a `Vec` and a translation) just to
// ask whether a pixel is inside it would allocate per primitive. The methods
// below delegate here, so there is one implementation.

/// The smallest rect containing every vertex, or `None` for no vertices.
///
/// **Inclusive of the far edge**: a single point has a 1×1 bound, not a
/// zero-sized one, because a zero-sized rect intersects nothing and a polygon
/// that covers one pixel does cover it.
pub fn bounds_of(points: &[Point]) -> Option<Rect> {
    let first = *points.first()?;
    let (min, max) = points.iter().fold((first, first), |(min, max), p| {
        (
            Point::new(min.x.min(p.x), min.y.min(p.y)),
            Point::new(max.x.max(p.x), max.y.max(p.y)),
        )
    });
    Some(Rect::new(
        min,
        Size::new((max.x - min.x + 1) as u32, (max.y - min.y + 1) as u32),
    ))
}

/// Non-zero winding rule: is `point` inside the closed polygon `points`?
pub fn contains(points: &[Point], point: Point) -> bool {
    let mut winding = 0i32;
    for i in 0..points.len() {
        let ls = points[i];
        let le = points[(i + 1) % points.len()];
        if ls.y <= point.y {
            if le.y > point.y && (le - ls).determinant(point - ls) > 0 {
                winding += 1;
            }
        } else if le.y <= point.y && (le - ls).determinant(point - ls) < 0 {
            winding -= 1;
        }
    }
    winding != 0
}

impl Polygon {
    /// The inclusive min/max corner pair. Kept in this shape because it is the
    /// published one; [`bounds_of`] is the rect form.
    pub fn bounds(&self) -> (Point, Point) {
        match bounds_of(&self.vertices) {
            Some(rect) => (
                rect.top_left,
                Point::new(
                    rect.top_left.x + rect.size.width as i32 - 1,
                    rect.top_left.y + rect.size.height as i32 - 1,
                ),
            ),
            None => (Point::zero(), Point::zero()),
        }
    }

    pub fn count(&self) -> usize {
        self.vertices.len()
    }

    pub fn lines(
        &self,
    ) -> impl Iterator<Item = crate::primitives::line::Line> + '_ {
        self.vertices.iter().copied().enumerate().map(|(i, v)| {
            crate::primitives::line::Line::new(
                v,
                self.vertices[(i + 1) % self.count()],
            )
        })
    }

    pub fn contains(&self, point: Point) -> bool {
        contains(&self.vertices, point)
    }
}

// TODO: https://aykevl.nl/2024/02/tinygl-polygon/
