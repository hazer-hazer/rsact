use core::ops::{Add, AddAssign, Div, Mul, Rem, Sub, SubAssign};
use embedded_graphics::geometry::Point;

use super::{axis::Axial, padding::Padding};

#[derive(Clone, Copy, Debug)]
pub struct DivFactors {
    pub width: u16,
    pub height: u16,
}

impl DivFactors {
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    pub fn zero() -> Self {
        Self { width: 0, height: 0 }
    }

    // pub fn take_rem(&self, rem: Size, container_div_factors: Self) -> Size {
    //     let rem = rem.map(|l| l as f32);
    //     Size::new(
    //         rem.width * self.width as f32 / container_div_factors.width as
    // f32,         rem.height * self.height as f32
    //             / container_div_factors.height as f32,
    //     )
    //     .map(|l| l as u32)
    // }

    // pub fn gcd(&self, other: Self) -> Self {
    //     Self::new(self.width.gcd(&other.width),
    // self.height.gcd(&other.height)) }
}

impl Axial for DivFactors {
    type Data = u16;

    fn x(&self) -> Self::Data {
        self.width
    }

    fn y(&self) -> Self::Data {
        self.height
    }

    fn x_mut(&mut self) -> &mut Self::Data {
        &mut self.width
    }

    fn y_mut(&mut self) -> &mut Self::Data {
        &mut self.height
    }

    fn new(x: Self::Data, y: Self::Data) -> Self {
        Self::new(x, y)
    }
}

impl Into<Size> for DivFactors {
    fn into(self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl Add for DivFactors {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.width + rhs.width, self.height + rhs.height)
    }
}

impl AddAssign for DivFactors {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Div for DivFactors {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self::new(
            self.width.checked_div(rhs.width).unwrap_or(0),
            self.height.checked_div(rhs.height).unwrap_or(0),
        )
    }
}

impl Div<DivFactors> for Size {
    type Output = Size;

    fn div(self, rhs: DivFactors) -> Self::Output {
        Size::new(
            self.width.checked_div(rhs.width as u32).unwrap_or(0),
            self.height.checked_div(rhs.height as u32).unwrap_or(0),
        )
    }
}

impl Rem<DivFactors> for Size {
    type Output = Size;

    fn rem(self, rhs: DivFactors) -> Self::Output {
        Size::new(
            self.width.checked_rem(rhs.width as u32).unwrap_or(0),
            self.height.checked_rem(rhs.height as u32).unwrap_or(0),
        )
    }
}

// impl Div<u16> for DivFactors {
//     type Output = Self;

//     fn div(self, rhs: u16) -> Self::Output {
//         Self::new(self.width / rhs, self.height / rhs)
//     }
// }

// impl Rem<u16> for DivFactors {
//     type Output = Self;

//     fn rem(self, rhs: u16) -> Self::Output {
//         Self::new(self.width % rhs, self.height % rhs)
//     }
// }

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub enum Length {
    // /// Fills all the remaining space
    // Fill,
    /// Shrink to the minimum space
    Shrink,

    /// Fill a portion of available space. Means `100% / Div(N)`
    Div(u16),

    /// Fixed pixels count
    Fixed(u32),
}

impl Length {
    pub fn fill(&self) -> Self {
        Self::Div(1)
    }

    pub fn div_factor(&self) -> u16 {
        match self {
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

    pub fn into_fixed(&self, base_div: u32) -> u32 {
        match self {
            Length::Shrink => base_div,
            &Length::Div(div) => base_div * div as u32,
            &Length::Fixed(fixed) => fixed,
        }
    }

    pub fn max_fixed(&self, fixed: u32) -> u32 {
        match self {
            Length::Shrink => fixed,
            Length::Div(_) => fixed,
            &Length::Fixed(this) => this.max(fixed),
        }
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

    pub fn map<F, U>(&self, f: F) -> Size<U>
    where
        F: Fn(T) -> U,
        T: Copy,
    {
        Size::new(f(self.width), f(self.height))
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

    pub fn div_factors(&self) -> DivFactors {
        DivFactors {
            width: self.width.div_factor(),
            height: self.height.div_factor(),
        }
    }

    pub fn max_fixed(&self, fixed: Size) -> Size {
        Size::new(
            self.width.max_fixed(fixed.width),
            self.height().max_fixed(fixed.height),
        )
    }

    pub fn into_fixed(&self, base_divs: Size) -> Size {
        Size::new(
            self.width.into_fixed(base_divs.width),
            self.height.into_fixed(base_divs.height),
        )
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

impl Sub<Padding> for Size {
    type Output = Self;

    fn sub(self, rhs: Padding) -> Self::Output {
        self - Into::<Size>::into(rhs)
    }
}

// impl Add<u32> for Size<u32> {
//     type Output = Self;

//     fn add(self, rhs: u32) -> Self::Output {
//         self + Size::new_equal(rhs)
//     }
// }

// impl Sub<u32> for Size<u32> {
//     type Output = Self;

//     fn sub(self, rhs: u32) -> Self::Output {
//         self - Size::new_equal(rhs)
//     }
// }

// impl Div<u32> for Size<u32> {
//     type Output = Self;

//     fn div(self, rhs: u32) -> Self::Output {
//         Self::new(self.width / rhs, self.height / rhs)
//     }
// }

// impl Rem<u32> for Size<u32> {
//     type Output = Self;

//     fn rem(self, rhs: u32) -> Self::Output {
//         Self::new(self.width / rhs, self.height / rhs)
//     }
// }

impl Mul<DivFactors> for Size<u32> {
    type Output = Self;

    fn mul(self, rhs: DivFactors) -> Self::Output {
        Self::new(
            self.width * rhs.width as u32,
            self.height * rhs.height as u32,
        )
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
        Self { width: Length::Div(1), height: Length::Div(1) }
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

    fn x_mut(&mut self) -> &mut Self::Data {
        &mut self.width
    }

    fn y_mut(&mut self) -> &mut Self::Data {
        &mut self.height
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
