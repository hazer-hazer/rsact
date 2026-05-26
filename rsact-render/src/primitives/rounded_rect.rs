use crate::{geometry::*, primitives::Primitive};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundedRect {
    pub rect: Rect,
    pub corners: CornerRadii,
}

impl RoundedRect {
    pub fn new(rect: Rect, corners: CornerRadii) -> Self {
        Self { rect, corners }
    }
}

impl Primitive for RoundedRect {
    fn into_kind(self) -> crate::prelude::PrimitiveKind {
        crate::prelude::PrimitiveKind::RoundedRect(self)
    }

    fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.rect.top_left += by;
        self
    }
}
