use crate::geometry::{Axial, size::Size};
use core::{
    fmt::Display,
    num::TryFromIntError,
    ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign},
};
#[allow(unused)]
use num::Float as _;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub const fn zero() -> Self {
        Self::new(0, 0)
    }

    pub const fn sign(&self) -> Self {
        Self::new(self.x.signum(), self.y.signum())
    }

    // pub const fn assert_positive(&self) -> Self {
    //     assert!(self.x >= 0 && self.y >= 0);
    //     *self
    // }
}

impl Axial for Point {
    type Data = i32;

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

    #[inline]
    fn axial_new(x: Self::Data, y: Self::Data) -> Self {
        Self::new(x, y)
    }
}

#[cfg(feature = "embedded-graphics")]
impl Axial for embedded_graphics::geometry::Point {
    type Data = i32;

    #[inline]
    fn x(&self) -> Self::Data {
        self.x
    }

    #[inline]
    fn y(&self) -> Self::Data {
        self.y
    }

    fn x_mut(&mut self) -> &mut Self::Data {
        &mut self.x
    }

    fn y_mut(&mut self) -> &mut Self::Data {
        &mut self.y
    }

    #[inline]
    fn axial_new(x: Self::Data, y: Self::Data) -> Self {
        Self::new(x, y)
    }
}

impl Display for Point {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

impl Add for Point {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

#[cfg(feature = "embedded-graphics")]
impl Add<embedded_graphics::geometry::Point> for Point {
    type Output = Self;
    fn add(self, rhs: embedded_graphics::geometry::Point) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Point {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Point {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign for Point {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Neg for Point {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

impl Mul<i32> for Point {
    type Output = Self;
    fn mul(self, rhs: i32) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl Div<i32> for Point {
    type Output = Self;

    fn div(self, rhs: i32) -> Self {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

impl TryFrom<Size> for Point {
    type Error = TryFromIntError;

    fn try_from(value: Size) -> Result<Self, Self::Error> {
        Ok(Self::new(value.width.try_into()?, value.height.try_into()?))
    }
}

#[cfg(feature = "embedded-graphics")]
impl From<embedded_graphics::geometry::Point> for Point {
    fn from(p: embedded_graphics::geometry::Point) -> Self {
        Self::new(p.x, p.y)
    }
}

#[cfg(feature = "embedded-graphics")]
impl From<Point> for embedded_graphics::geometry::Point {
    #[inline(always)]
    fn from(p: Point) -> Self {
        Self::new(p.x, p.y)
    }
}

pub trait PointExt: Sized + Copy {
    fn new_rounded(x: f32, y: f32) -> Self;
    fn new_floor(x: f32, y: f32) -> Self;

    fn swap_axes(self) -> Self;

    fn swap_axes_if(self, cond: bool) -> Self {
        if cond { self.swap_axes() } else { self }
    }

    /// Unlike `PartialOrd::clamp` this method does fine-grained clamping per
    /// axis.
    fn clamp_axes(self, min: Self, max: Self) -> Self;

    fn map(self, f: impl FnMut(i32) -> i32) -> Self;

    fn mirror_x(self) -> Self;
    fn mirror_y(self) -> Self;
    fn each_mirror(self) -> impl Iterator<Item = Self> {
        [self, self.mirror_x(), self.mirror_y(), self.mirror_x().mirror_y()]
            .into_iter()
    }

    fn add_x(self, x: i32) -> Self;
    fn add_y(self, y: i32) -> Self;
    fn add_x_round(self, x: f32) -> Self;
    fn add_y_round(self, y: f32) -> Self;
    fn add_x_floor(self, x: f32) -> Self;
    fn add_y_floor(self, y: f32) -> Self;

    fn scale_round(self, scale: f32) -> Self;
    fn dist_sq(self, other: Self) -> f32;
    fn dist_to(self, other: Self) -> f32;
    fn dot(self, other: Self) -> i32;
    fn determinant(self, other: Self) -> i32;
}

impl PointExt for Point {
    fn new_rounded(x: f32, y: f32) -> Self {
        Self::new(x.round() as i32, y.round() as i32)
    }

    fn new_floor(x: f32, y: f32) -> Self {
        Self::new(x.floor() as i32, y.floor() as i32)
    }

    fn swap_axes(self) -> Self {
        Self::new(self.y, self.x)
    }

    fn clamp_axes(self, min: Self, max: Self) -> Self {
        Self::new(self.x.min(max.x).max(min.x), self.y.min(max.y).max(min.y))
    }

    fn map(self, mut f: impl FnMut(i32) -> i32) -> Self {
        Self::new(f(self.x), f(self.y))
    }

    fn mirror_x(self) -> Self {
        Self::new(-self.x, self.y)
    }

    fn mirror_y(self) -> Self {
        Self::new(self.x, -self.y)
    }

    fn add_x(self, x: i32) -> Self {
        Self::new(self.x + x, self.y)
    }

    fn add_y(self, y: i32) -> Self {
        Self::new(self.x, self.y + y)
    }

    fn add_x_round(self, x: f32) -> Self {
        Self::new((self.x as f32 + x).round() as i32, self.y)
    }

    fn add_y_round(self, y: f32) -> Self {
        Self::new(self.x, (self.y as f32 + y).round() as i32)
    }

    fn add_x_floor(self, x: f32) -> Self {
        Self::new((self.x as f32 + x).floor() as i32, self.y)
    }

    fn add_y_floor(self, y: f32) -> Self {
        Self::new(self.x, (self.y as f32 + y).floor() as i32)
    }

    fn scale_round(self, scale: f32) -> Self {
        Self::new(
            (self.x as f32 * scale).round() as i32,
            (self.y as f32 * scale).round() as i32,
        )
    }

    fn dist_sq(self, other: Self) -> f32 {
        (self.x - other.x).pow(2) as f32 + (self.y - other.y).pow(2) as f32
    }

    fn dist_to(self, other: Self) -> f32 {
        self.dist_sq(other).sqrt()
    }

    fn dot(self, other: Self) -> i32 {
        self.x * other.x + self.y * other.y
    }

    fn determinant(self, other: Self) -> i32 {
        self.x * other.y - self.y * other.x
    }
}

// Allow embedded_graphics Point to use PointExt methods inside the EG backend.
#[cfg(feature = "embedded-graphics")]
impl PointExt for embedded_graphics::geometry::Point {
    fn new_rounded(x: f32, y: f32) -> Self {
        Self::new(x.round() as i32, y.round() as i32)
    }
    fn new_floor(x: f32, y: f32) -> Self {
        Self::new(x.floor() as i32, y.floor() as i32)
    }
    fn swap_axes(self) -> Self {
        Self::new(self.y, self.x)
    }
    fn clamp_axes(self, min: Self, max: Self) -> Self {
        Self::new(self.x.min(max.x).max(min.x), self.y.min(max.y).max(min.y))
    }
    fn map(self, mut f: impl FnMut(i32) -> i32) -> Self {
        Self::new(f(self.x), f(self.y))
    }
    fn mirror_x(self) -> Self {
        Self::new(-self.x, self.y)
    }
    fn mirror_y(self) -> Self {
        Self::new(self.x, -self.y)
    }
    fn add_x(self, x: i32) -> Self {
        Self::new(self.x + x, self.y)
    }
    fn add_y(self, y: i32) -> Self {
        Self::new(self.x, self.y + y)
    }
    fn add_x_round(self, x: f32) -> Self {
        Self::new((self.x as f32 + x).round() as i32, self.y)
    }
    fn add_y_round(self, y: f32) -> Self {
        Self::new(self.x, (self.y as f32 + y).round() as i32)
    }
    fn add_x_floor(self, x: f32) -> Self {
        Self::new((self.x as f32 + x).floor() as i32, self.y)
    }
    fn add_y_floor(self, y: f32) -> Self {
        Self::new(self.x, (self.y as f32 + y).floor() as i32)
    }
    fn scale_round(self, scale: f32) -> Self {
        Self::new(
            (self.x as f32 * scale).round() as i32,
            (self.y as f32 * scale).round() as i32,
        )
    }
    fn dist_sq(self, other: Self) -> f32 {
        let dx = (self.x - other.x) as f32;
        let dy = (self.y - other.y) as f32;
        dx * dx + dy * dy
    }
    fn dist_to(self, other: Self) -> f32 {
        self.dist_sq(other).sqrt()
    }
    fn dot(self, other: Self) -> i32 {
        self.x * other.x + self.y * other.y
    }
    fn determinant(self, other: Self) -> i32 {
        self.x * other.y - self.y * other.x
    }
}
