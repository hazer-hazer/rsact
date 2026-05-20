use crate::geometry::*;

// TODO: Should ellipse just store a Rect?
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ellipse {
    pub top_left: Point,
    pub size: Size,
}

impl Ellipse {
    pub fn new(top_left: Point, size: Size) -> Self {
        Self { top_left, size }
    }

    pub fn translate(&self, by: Point) -> Self {
        let mut new = *self;
        new.top_left += by;
        new
    }

    pub fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.top_left += by;
        self
    }
}
