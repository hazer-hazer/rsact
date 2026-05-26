use crate::{geometry::Point, primitives::Primitive};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub top_left: Point,
    pub diameter: u32,
}

impl Circle {
    pub fn new(top_left: Point, diameter: u32) -> Self {
        Self { top_left, diameter }
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

impl Primitive for Circle {
    fn into_kind(self) -> crate::prelude::PrimitiveKind {
        crate::prelude::PrimitiveKind::Circle(self)
    }

    fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.top_left += by;
        self
    }
}
