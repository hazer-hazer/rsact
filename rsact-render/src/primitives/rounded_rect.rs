use crate::geometry::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundedRect {
    pub rect: Rect,
    pub corners: CornerRadii,
}

impl RoundedRect {
    pub fn new(rect: Rect, corners: CornerRadii) -> Self {
        Self { rect, corners }
    }

    pub fn translate(&self, by: Point) -> Self {
        let mut new = *self;
        new.rect.top_left += by;
        new
    }

    pub fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.rect.top_left += by;
        self
    }
}
