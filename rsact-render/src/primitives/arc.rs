use crate::{geometry::*, primitives::Primitive};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    pub top_left: Point,
    pub diameter: u32,
    pub start: Angle,
    pub sweep: Angle,
}

impl Arc {
    pub fn new(
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
    ) -> Self {
        Self { top_left, diameter, start, sweep }
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

impl Primitive for Arc {
    fn into_kind(self) -> crate::prelude::PrimitiveKind {
        crate::prelude::PrimitiveKind::Arc(self)
    }

    fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.top_left += by;
        self
    }
}
