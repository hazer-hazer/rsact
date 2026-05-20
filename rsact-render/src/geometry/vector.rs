use crate::geometry::*;
use core::ops::{Add, Div, Mul, Neg, Sub};

/// Screen-oriented quadrant signs
/// Note: In math (+,+) is top right, whereas on screen (+,+) is bottom right
#[repr(i8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnitV1 {
    Minus = -1,
    Zero = 0,
    Plus = 1,
}

impl Add<i32> for UnitV1 {
    type Output = i32;

    fn add(self, rhs: i32) -> Self::Output {
        self as i32 + rhs
    }
}

impl Sub<i32> for UnitV1 {
    type Output = i32;

    fn sub(self, rhs: i32) -> Self::Output {
        self as i32 - rhs
    }
}

impl Mul<i32> for UnitV1 {
    type Output = i32;

    fn mul(self, rhs: i32) -> Self::Output {
        self as i32 * rhs
    }
}

impl Div<i32> for UnitV1 {
    type Output = i32;

    fn div(self, rhs: i32) -> Self::Output {
        self as i32 / rhs
    }
}

impl Neg for UnitV1 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        (-(self as i32)).into()
    }
}

impl From<i32> for UnitV1 {
    fn from(value: i32) -> Self {
        match value {
            ..=-1 => Self::Minus,
            0 => Self::Zero,
            1.. => Self::Plus,
        }
    }
}

impl Into<i32> for UnitV1 {
    fn into(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct UnitV2 {
    pub x: UnitV1,
    pub y: UnitV1,
}

impl Axial for UnitV2 {
    type Data = UnitV1;

    fn x(&self) -> Self::Data {
        self.x
    }

    fn y(&self) -> Self::Data {
        self.y
    }

    fn x_mut(&mut self) -> &mut Self::Data {
        &mut self.x
    }

    fn y_mut(&mut self) -> &mut Self::Data {
        &mut self.y
    }

    fn axial_new(x: Self::Data, y: Self::Data) -> Self {
        Self::new(x, y)
    }
}

impl UnitV2 {
    pub const LEFT: Self = Self::const_new(UnitV1::Minus, UnitV1::Zero);
    pub const RIGHT: Self = Self::const_new(UnitV1::Plus, UnitV1::Zero);
    pub const UP: Self = Self::const_new(UnitV1::Zero, UnitV1::Minus);
    pub const DOWN: Self = Self::const_new(UnitV1::Zero, UnitV1::Plus);

    pub const fn const_new(x: UnitV1, y: UnitV1) -> Self {
        Self { x, y }
    }

    pub fn new(x: impl Into<UnitV1>, y: impl Into<UnitV1>) -> Self {
        Self { x: x.into(), y: y.into() }
    }

    pub fn destruct(self) -> (i32, i32) {
        (self.x.into(), self.y.into())
    }
}

impl Neg for UnitV2 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y)
    }
}

impl Mul<UnitV2> for Point {
    type Output = Point;

    fn mul(self, rhs: UnitV2) -> Self::Output {
        Self::new(self.x * rhs.x as i32, self.y * rhs.y as i32)
    }
}

impl Mul<i32> for UnitV2 {
    type Output = Point;

    fn mul(self, rhs: i32) -> Self::Output {
        Point::new(self.x * rhs, self.y * rhs)
    }
}

impl From<AnchorPoint> for UnitV2 {
    fn from(value: AnchorPoint) -> Self {
        match value {
            AnchorPoint::TopLeft => Self::new(-1, -1),
            AnchorPoint::TopCenter => Self::new(0, -1),
            AnchorPoint::TopRight => Self::new(1, -1),
            AnchorPoint::CenterLeft => Self::new(-1, 0),
            AnchorPoint::Center => Self::new(0, 0),
            AnchorPoint::CenterRight => Self::new(1, 0),
            AnchorPoint::BottomLeft => Self::new(-1, 1),
            AnchorPoint::BottomCenter => Self::new(0, 1),
            AnchorPoint::BottomRight => Self::new(1, 1),
        }
    }
}

pub trait ByUnitV2 {
    type Output;

    fn by_unit_v(&self, unit_v: UnitV2) -> Self::Output;
}
