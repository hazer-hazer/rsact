use core::ops::{Add, Div, Mul, Sub};

use embedded_graphics::geometry::Point;

use crate::{
    axis::{Axial, Axis},
    padding::Padding,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub enum Length {
    /// Fills all the remaining space
    Fill,

    /// Shrink to the minimum space
    Shrink,

    /// Fill a portion of available space. Means `100% / Div(N)`
    Div(u16),

    /// Fixed pixels count
    Fixed(u32),
}

impl Length {
    pub fn div_factor(&self) -> u16 {
        match self {
            Length::Fill => 1,
            Length::Fixed(_) | Length::Shrink => 0,
            Length::Div(div) => *div,
        }
    }

    pub fn infinite() -> Self {
        // TODO: Do we need distinct `Length::Infinite`?
        Self::Fixed(u32::MAX)
    }

    pub fn is_fixed(&self) -> bool {
        matches!(self, Self::Fixed(_))
    }

    pub fn is_fill(&self) -> bool {
        self.div_factor() != 0
    }
}

impl From<u32> for Length {
    fn from(value: u32) -> Self {
        Self::Fixed(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub struct Size<T = u32> {
    pub width: T,
    pub height: T,
}

impl<T> Size<T> {
    pub const fn new(width: T, height: T) -> Self {
        Self { width, height }
    }

    pub const fn new_equal(equal: T) -> Self
    where
        T: Copy,
    {
        Self { width: equal, height: equal }
    }

    pub fn with_width(self, width: T) -> Self {
        Self { width, height: self.height }
    }

    pub fn with_height(self, height: T) -> Self {
        Self { width: self.width, height }
    }
}

impl Size<Length> {
    pub fn is_fixed(&self) -> bool {
        self.width.is_fixed() && self.height.is_fixed()
    }

    pub fn is_fill(&self) -> bool {
        self.width.is_fill() && self.height.is_fill()
    }
}

impl Size<u32> {
    pub fn zero() -> Self {
        Self { width: 0, height: 0 }
    }

    pub fn expand(self, by: impl Into<Size>) -> Self {
        let by = by.into();

        Self::new(self.width + by.width, self.height + by.height)
    }

    pub fn as_fixed_length(self) -> Size<Length> {
        Size::new(Length::Fixed(self.width), Length::Fixed(self.height))
    }
}

impl Add<Size> for Point {
    type Output = Self;

    fn add(self, rhs: Size) -> Self::Output {
        // TODO: Add debug assertions
        let width = rhs.width as i32;
        let height = rhs.height as i32;

        Self::new(self.x + width, self.y + height)
    }
}

impl From<u32> for Size {
    fn from(value: u32) -> Self {
        Self::new(value, value)
    }
}

impl Add<Padding> for Size {
    type Output = Self;

    fn add(self, rhs: Padding) -> Self::Output {
        self + Into::<Size>::into(rhs)
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

impl Sub for Size<u32> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(
            self.width.saturating_sub(rhs.width),
            self.height.saturating_sub(rhs.height),
        )
    }
}

impl Sub<Padding> for Size {
    type Output = Self;

    fn sub(self, rhs: Padding) -> Self::Output {
        self - Into::<Size>::into(rhs)
    }
}

impl Add<u32> for Size<u32> {
    type Output = Self;

    fn add(self, rhs: u32) -> Self::Output {
        self + Size::new_equal(rhs)
    }
}

impl Sub<u32> for Size<u32> {
    type Output = Self;

    fn sub(self, rhs: u32) -> Self::Output {
        self - Size::new_equal(rhs)
    }
}

impl Div<u32> for Size<u32> {
    type Output = Self;

    fn div(self, rhs: u32) -> Self::Output {
        Self::new(self.width / rhs, self.height / rhs)
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

impl Size<Length> {
    pub fn fixed_length(width: u32, height: u32) -> Self {
        Self { width: Length::Fixed(width), height: Length::Fixed(height) }
    }

    pub fn shrink() -> Self {
        Self { width: Length::Shrink, height: Length::Shrink }
    }

    pub fn fill() -> Self {
        Self { width: Length::Fill, height: Length::Fill }
    }
}

impl Into<Size<Length>> for Size {
    fn into(self) -> Size<Length> {
        Size::new(Length::Fixed(self.width), Length::Fixed(self.height))
    }
}

impl<T> From<(T, T)> for Size<T> {
    fn from(value: (T, T)) -> Self {
        Size::new(value.0, value.1)
    }
}

impl From<embedded_graphics_core::geometry::Size> for Size {
    fn from(value: embedded_graphics_core::geometry::Size) -> Self {
        Self::new(value.width, value.height)
    }
}

impl Into<embedded_graphics_core::geometry::Size> for Size {
    fn into(self) -> embedded_graphics_core::geometry::Size {
        embedded_graphics_core::geometry::Size::new(self.width, self.height)
    }
}

impl<S: Copy> Axial for Size<S> {
    type Data = S;

    #[inline]
    fn x(&self) -> Self::Data {
        self.width
    }

    #[inline]
    fn y(&self) -> Self::Data {
        self.height
    }

    #[inline]
    fn new(x: Self::Data, y: Self::Data) -> Self {
        Self::new(x, y)
    }
}

pub trait SizeExt: Copy {
    type Data: core::cmp::Ord + core::cmp::Eq;

    fn width(self) -> Self::Data;
    fn height(self) -> Self::Data;

    fn max_square(self) -> Self::Data {
        self.width().min(self.height())
    }
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
}

impl SizeExt for embedded_graphics_core::geometry::Size {
    type Data = u32;

    #[inline]
    fn width(self) -> Self::Data {
        self.width
    }

    #[inline]
    fn height(self) -> Self::Data {
        self.height
    }
}
