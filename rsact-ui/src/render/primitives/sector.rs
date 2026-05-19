use crate::geometry::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sector {
    pub top_left: Point,
    pub diameter: u32,
    pub start_angle: Angle,
    pub sweep_angle: Angle,
}

impl Sector {
    pub fn new(
        top_left: Point,
        diameter: u32,
        start_angle: Angle,
        sweep_angle: Angle,
    ) -> Self {
        Self {
            top_left: top_left.into(),
            diameter,
            start_angle: start_angle.into(),
            sweep_angle: sweep_angle.into(),
        }
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
