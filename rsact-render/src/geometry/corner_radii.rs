use crate::geometry::{
    size::Size,
    vector::{ByUnitV2, UnitV2},
};

/// First-class corner radii, replacing embedded_graphics::primitives::CornerRadii.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CornerRadii {
    pub top_left: Size,
    pub top_right: Size,
    pub bottom_right: Size,
    pub bottom_left: Size,
}

impl CornerRadii {
    pub fn new(
        top_left: Size,
        top_right: Size,
        bottom_right: Size,
        bottom_left: Size,
    ) -> Self {
        Self { top_left, top_right, bottom_right, bottom_left }
    }

    pub fn new_equal(size: Size) -> Self {
        Self {
            top_left: size,
            top_right: size,
            bottom_right: size,
            bottom_left: size,
        }
    }
}

impl ByUnitV2 for CornerRadii {
    type Output = Option<Size>;

    fn by_unit_v(&self, unit_v: UnitV2) -> Self::Output {
        match unit_v.destruct() {
            (-1, -1) => Some(self.top_left),
            (1, -1) => Some(self.top_right),
            (-1, 1) => Some(self.bottom_left),
            (1, 1) => Some(self.bottom_right),
            _ => None,
        }
    }
}

#[cfg(feature = "embedded-graphics")]
impl From<CornerRadii> for embedded_graphics::primitives::CornerRadii {
    fn from(c: CornerRadii) -> Self {
        embedded_graphics::primitives::CornerRadii {
            top_left: c.top_left.into(),
            top_right: c.top_right.into(),
            bottom_right: c.bottom_right.into(),
            bottom_left: c.bottom_left.into(),
        }
    }
}
