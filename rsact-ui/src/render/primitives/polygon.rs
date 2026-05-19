use crate::geometry::Point;
use alloc::vec::Vec;

// TODO: get rid of the vector
#[derive(Clone, PartialEq, Debug)]
pub struct Polygon {
    pub top_left: Point,
    pub vertices: Vec<Point>,
}

impl Polygon {
    pub fn new(
        top_left: Point,
        vertices: impl IntoIterator<Item = Point>,
    ) -> Self {
        let points: Vec<Point> = vertices.into_iter().collect();

        assert!(points.len() >= 3, "Polygon must contain at least 3 vertices");
        assert!(points.first() != points.last(), "Polygon must not be closed");

        Self { top_left, vertices: points }
    }

    pub fn translate(&self, by: Point) -> Self {
        let mut new = self.clone();
        new.top_left += by;
        new
    }

    pub fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.top_left += by;
        self
    }
}
