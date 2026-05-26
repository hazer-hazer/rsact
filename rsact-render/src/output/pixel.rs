use crate::geometry::Point;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixel<C>(pub Point, pub C);
