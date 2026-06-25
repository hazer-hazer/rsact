use crate::geometry::Point;
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
