use crate::{color::Color, geometry::Point};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixel<C: Color>(pub Point, pub C);
