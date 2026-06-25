use crate::geometry::{axis::Axial, padding::Padding, point::Point};
use core::{
    fmt::Display,
    ops::{Add, AddAssign, Mul, Sub, SubAssign},
};
use rsact_reactive::prelude::IntoMaybeReactive;

pub trait SubTake<Rhs = Self> {
    fn sub_take(&mut self, sub: Rhs) -> Self;
}

impl SubTake for u32 {
    fn sub_take(&mut self, sub: Self) -> Self {
        if *self >= sub {
            *self -= sub;
            sub
        } else {
            0
        }
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    IntoMaybeReactive,
)]
#[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub struct Size<T: PartialEq = u32> {
    pub width: T,
    pub height: T,
}

impl<T: PartialEq> Size<T> {
    pub const fn new(width: T, height: T) -> Self {
        Self { width, height }
    }

    pub const fn new_equal(equal: T) -> Self
    where
        T: Copy,
    {
        Self { width: equal, height: equal }
    }

    pub fn map<F, U: PartialEq>(&self, f: F) -> Size<U>
    where
        F: Fn(T) -> U,
        T: Copy,
    {
        Size::new(f(self.width), f(self.height))
    }

    pub fn swapped(self) -> Size<T> {
        Size::new(self.height, self.width)
    }

    pub fn with_width(self, width: T) -> Self {
        Self { width, height: self.height }
    }

    pub fn with_height(self, height: T) -> Self {
        Self { width: self.width, height }
    }

    pub fn area(self) -> <T as Mul>::Output
    where
        T: Mul,
    {
        self.width * self.height
    }
}

impl Size<u32> {
    pub const fn zero() -> Self {
        Self { width: 0, height: 0 }
    }

    pub fn max_square(self) -> Self {
        let min = self.width.min(self.height);

        Self::new_equal(min)
    }

    pub const fn is_zero(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

impl Display for Size<u32> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.width == u32::MAX {
            f.write_str("Inf")
        } else {
            write!(f, "{}", self.width)
        }?;
        if self.width == u32::MAX {
            f.write_str("xInf")
        } else {
            write!(f, "x{}", self.width)
        }
    }
}

impl Add for Size<u32> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(
            self.width.saturating_add(rhs.width),
            self.height.saturating_add(rhs.height),
        )
    }
}

impl Add<Size<i32>> for Size<u32> {
    type Output = Self;

    fn add(self, rhs: Size<i32>) -> Self::Output {
        Self::new(
            self.width.saturating_add_signed(rhs.width),
            self.height.saturating_add_signed(rhs.height),
        )
    }
}

impl AddAssign for Size<u32> {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Size<u32> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(
            self.width.saturating_sub(rhs.width),
            self.height.saturating_sub(rhs.height),
        )
    }
}

impl SubAssign for Size<u32> {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl SubAssign<u32> for Size<u32> {
    fn sub_assign(&mut self, rhs: u32) {
        *self = *self - Size::new_equal(rhs)
    }
}

impl From<u32> for Size {
    fn from(value: u32) -> Self {
        Self::new(value, value)
    }
}

impl Into<Size<f32>> for Size<u32> {
    fn into(self) -> Size<f32> {
        Size::new(self.width as f32, self.height as f32)
    }
}

impl Add<Size<u32>> for Point {
    type Output = Self;

    fn add(self, rhs: Size) -> Self::Output {
        // TODO: Add debug assertions
        let width = rhs.width as i32;
        let height = rhs.height as i32;

        Self::new(self.x + width, self.y + height)
    }
}

impl SubTake<u32> for Size<u32> {
    fn sub_take(&mut self, sub: u32) -> Self {
        Self::new(self.width.sub_take(sub), self.height.sub_take(sub))
    }
}

impl<T: PartialEq> From<(T, T)> for Size<T> {
    fn from(value: (T, T)) -> Self {
        Size::new(value.0, value.1)
    }
}

impl Mul<f32> for Size<u32> {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(
            (self.width as f32 * rhs) as u32,
            (self.height as f32 * rhs) as u32,
        )
    }
}

impl Mul<Size<f32>> for Size<u32> {
    type Output = Self;

    fn mul(self, rhs: Size<f32>) -> Self::Output {
        Self::new(
            (self.width as f32 * rhs.width) as u32,
            (self.height as f32 * rhs.height) as u32,
        )
    }
}

impl Mul<Size<f32>> for Size<f32> {
    type Output = Self;

    fn mul(self, rhs: Size<f32>) -> Self::Output {
        Self::new(self.width * rhs.width, self.height * rhs.height)
    }
}

impl Mul<f32> for Size<f32> {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.width * rhs, self.height * rhs)
    }
}

impl<S: PartialEq + Copy> Axial for Size<S> {
    type Data = S;

    #[inline]
    fn x(&self) -> S {
        self.width
    }

    #[inline]
    fn y(&self) -> S {
        self.height
    }

    fn x_mut(&mut self) -> &mut S {
        &mut self.width
    }

    fn y_mut(&mut self) -> &mut S {
        &mut self.height
    }

    #[inline]
    fn axial_new(x: S, y: S) -> Self {
        Self::new(x, y)
    }
}

pub trait SizeExt: Copy {
    type Data: core::cmp::Ord + core::cmp::Eq;

    fn width(self) -> Self::Data;
    fn height(self) -> Self::Data;

    fn max_square(self) -> Self;
}

impl<S: core::cmp::Ord + core::cmp::Eq + Copy> SizeExt for Size<S> {
    type Data = S;

    #[inline]
    fn width(self) -> Self::Data {
        self.width
    }

    #[inline]
    fn height(self) -> Self::Data {
        self.height
    }

    fn max_square(self) -> Self {
        Self::new_equal(self.width().min(self.height()))
    }
}

impl Add<Padding> for Size {
    type Output = Self;

    fn add(self, rhs: Padding) -> Self::Output {
        self + Into::<Size>::into(rhs)
    }
}

impl Sub<Padding> for Size {
    type Output = Self;

    fn sub(self, rhs: Padding) -> Self::Output {
        self - Into::<Size>::into(rhs)
    }
}

#[cfg(feature = "embedded-graphics")]
impl From<embedded_graphics::geometry::Size> for Size {
    fn from(value: embedded_graphics::geometry::Size) -> Self {
        Self::new(value.width, value.height)
    }
}

#[cfg(feature = "embedded-graphics")]
impl Into<embedded_graphics::geometry::Size> for Size {
    fn into(self) -> embedded_graphics::geometry::Size {
        embedded_graphics::geometry::Size::new(self.width, self.height)
    }
}

#[cfg(feature = "embedded-graphics")]
impl SizeExt for embedded_graphics::geometry::Size {
    type Data = u32;

    #[inline]
    fn width(self) -> Self::Data {
        self.width
    }

    #[inline]
    fn height(self) -> Self::Data {
        self.height
    }

    fn max_square(self) -> Self {
        Self::new_equal(self.width().min(self.height()))
    }
}

#[cfg(feature = "embedded-graphics")]
impl Axial for embedded_graphics::geometry::Size {
    type Data = u32;

    #[inline]
    fn x(&self) -> Self::Data {
        self.width
    }

    #[inline]
    fn y(&self) -> Self::Data {
        self.height
    }

    fn x_mut(&mut self) -> &mut Self::Data {
        &mut self.width
    }

    fn y_mut(&mut self) -> &mut Self::Data {
        &mut self.height
    }

    #[inline]
    fn axial_new(x: Self::Data, y: Self::Data) -> Self {
        Self::new(x, y)
    }
}
