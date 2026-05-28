use crate::geometry::{
    Sided,
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

    pub fn new_equal_radius(radius: u32) -> Self {
        Self::new_equal(Size::new(radius, radius))
    }

    pub fn map(&self, f: impl Fn(Size) -> Size) -> Self {
        Self {
            top_left: f(self.top_left),
            top_right: f(self.top_right),
            bottom_right: f(self.bottom_right),
            bottom_left: f(self.bottom_left),
        }
    }

    /// Clamp radius for a rect of the given size, ensuring that radii do not overlap.
    pub fn clamp_for(&self, size: Size) -> Self {
        let w = size.width;
        let h = size.height;

        let mut tl = self.top_left;
        let mut tr = self.top_right;
        let mut br = self.bottom_right;
        let mut bl = self.bottom_left;

        let top_sum = tl.width + tr.width;
        if top_sum > w {
            let factor = w as f32 / top_sum as f32;
            tl.width = (tl.width as f32 * factor) as u32;
            tr.width = (tr.width as f32 * factor) as u32;
        }
        let bottom_sum = bl.width + br.width;
        if bottom_sum > w {
            let factor = w as f32 / bottom_sum as f32;
            bl.width = (bl.width as f32 * factor) as u32;
            br.width = (br.width as f32 * factor) as u32;
        }

        let left_sum = tl.height + bl.height;
        if left_sum > h {
            let factor = h as f32 / left_sum as f32;
            tl.height = (tl.height as f32 * factor) as u32;
            bl.height = (bl.height as f32 * factor) as u32;
        }
        let right_sum = tr.height + br.height;
        if right_sum > h {
            let factor = h as f32 / right_sum as f32;
            tr.height = (tr.height as f32 * factor) as u32;
            br.height = (br.height as f32 * factor) as u32;
        }

        Self { top_left: tl, top_right: tr, bottom_right: br, bottom_left: bl }
    }
}

impl Sided<u32> for CornerRadii {
    fn side(&self, side: crate::prelude::Side) -> u32 {
        match side {
            super::Side::Top => self.top_left.width + self.top_right.width,
            super::Side::Right => {
                self.top_right.height + self.bottom_right.height
            },
            super::Side::Bottom => {
                self.bottom_left.width + self.bottom_right.width
            },
            super::Side::Left => self.top_left.height + self.bottom_left.height,
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
