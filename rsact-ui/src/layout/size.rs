use super::{
    axis::{Anchor, Axial},
    padding::Padding,
    Axis,
};
use core::{
    fmt::Display,
    num::TryFromIntError,
    ops::{Add, AddAssign, Div, Mul, Neg, Rem, Sub, SubAssign},
};
use embedded_graphics::{
    geometry::{AnchorPoint, Point},
    primitives::{CornerRadii, Rectangle},
};

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

    fn axial_new(x: Self::Data, y: Self::Data) -> Self {
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

// #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// pub struct InfiniteLength;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub enum DeterministicLength {
    Shrink,
    Div(u16),
    Fixed(u32),
}

impl TryFrom<Length> for DeterministicLength {
    type Error = ();

    fn try_from(value: Length) -> Result<Self, Self::Error> {
        match value {
            Length::Shrink => Ok(Self::Shrink),
            Length::Div(div) => Ok(Self::Div(div)),
            Length::Fixed(fixed) => Ok(Self::Fixed(fixed)),
            Length::InfiniteWindow(_) => Err(()),
        }
    }
}

impl DeterministicLength {
    pub fn into_length(self) -> Length {
        self.into()
    }
}

impl Into<Length> for DeterministicLength {
    fn into(self) -> Length {
        match self {
            DeterministicLength::Shrink => Length::Shrink,
            DeterministicLength::Div(div) => Length::Div(div),
            DeterministicLength::Fixed(fixed) => Length::Fixed(fixed),
        }
    }
}

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

    /// Only available for special internal layouts such as Scrollable
    #[non_exhaustive]
    InfiniteWindow(DeterministicLength),
    // /// Fixed scrollable window
    // Scroll(Length),
}

impl Display for Length {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Length::Shrink => write!(f, "Shrink"),
            Length::Div(div) if *div == 1 => write!(f, "Fill"),
            Length::Div(div) => write!(f, "Div({div})"),
            Length::Fixed(fixed) if *fixed == u32::MAX => {
                write!(f, "Inf")
            },
            Length::Fixed(fixed) => write!(f, "Fixed({fixed})"),
            Length::InfiniteWindow(length) => {
                write!(f, "InfiniteWindow({length:?})")
            },
            // Length::Scroll(fixed) => write!(f, "Length::Scroll({fixed})"),
        }
    }
}

impl Length {
    pub fn fill() -> Self {
        Self::Div(1)
    }

    pub fn div_factor(&self) -> u16 {
        match self {
            Length::InfiniteWindow(length) => length.into_length().div_factor(),
            Length::Fixed(_) | Length::Shrink => 0,
            Length::Div(div) => *div,
        }
    }

    fn in_parent(self, parent: Self) -> Self {
        match (self, parent) {
            (Self::Div(_), Self::Shrink) => Self::Shrink,
            _ => self,
        }
    }

    // pub fn infinite() -> Self {
    //     // TODO: Do we need distinct `Length::Infinite`?
    //     Self::Fixed(u32::MAX)
    // }

    // pub fn is_fixed(&self) -> bool {
    //     matches!(self, Self::Fixed(_))
    // }

    // pub fn is_fill(&self) -> bool {
    //     self.div_factor() != 0
    // }

    pub fn set_deterministic(&mut self, length: impl Into<Length>) {
        let length = length.into();
        if let Length::InfiniteWindow(_) = self {
            *self = Length::InfiniteWindow(
                length
                    .try_into()
                    .expect("Setting Length::InfiniteWindow is not allowed"),
            );
        } else {
            *self = length;
        }
    }

    // TODO: Make these methods pub(crate)?

    pub fn is_grow(&self) -> bool {
        match self {
            Length::InfiniteWindow(length) => length.into_length().is_grow(),
            Length::Div(_) => true,
            Length::Shrink | Length::Fixed(_) => false,
        }
    }

    pub fn into_fixed(&self, base_div: u32) -> u32 {
        match self {
            Length::InfiniteWindow(length) => {
                length.into_length().into_fixed(base_div)
            },
            Length::Shrink => base_div,
            &Length::Div(div) => base_div * div as u32,
            &Length::Fixed(fixed) => fixed,
        }
    }

    pub fn max_fixed(&self, fixed: u32) -> u32 {
        match self {
            Length::InfiniteWindow(length) => {
                length.into_length().max_fixed(fixed)
            },
            Length::Shrink | Length::Div(_) => fixed,
            &Length::Fixed(fixed) => fixed.max(fixed),
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

impl From<embedded_graphics_core::geometry::Size> for Size<Length> {
    fn from(value: embedded_graphics_core::geometry::Size) -> Self {
        Self::new(Length::Fixed(value.width), Length::Fixed(value.height))
    }
}

impl Size<Length> {
    // pub fn is_fixed(&self) -> bool {
    //     self.width.is_fixed() && self.height.is_fixed()
    // }

    // pub fn is_fill(&self) -> bool {
    //     self.width.is_fill() && self.height.is_fill()
    // }

    pub fn in_parent(self, parent: Self) -> Self {
        Self::new(
            self.width.in_parent(parent.width),
            self.height.in_parent(parent.height),
        )
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
            self.height.max_fixed(fixed.height),
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

    // pub fn div_ceil(self, rhs: u32) -> Self {
    //     Self::new(self.width.div_ceil(rhs), self.height.div_ceil(rhs))
    // }

    pub fn max_square(self) -> Self {
        let min = self.width.min(self.height);

        Self::new_equal(min)
    }

    pub fn is_zero(&self) -> bool {
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

impl Display for Size<Length> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl SubTake<u32> for Size<u32> {
    fn sub_take(&mut self, sub: u32) -> Self {
        Self::new(self.width.sub_take(sub), self.height.sub_take(sub))
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
    fn axial_new(x: Self::Data, y: Self::Data) -> Self {
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

    fn max_square(self) -> Self {
        Self::new_equal(self.width().min(self.height()))
    }
}

// #[derive(Clone, Copy)]
// pub enum AxisAnchorPoint {
//     MainStart,
//     MainCenter,
//     MainEnd,
//     CenterLeft,
//     Center,
//     CenterEnd,
//     CrossStart,
//     CrossCenter,
//     CrossEnd,
// }

pub trait RectangleExt {
    fn center_offset_of(&self, child: Self) -> Point;
    fn resized_axis(&self, axis: Axis, size: u32, anchor: Anchor) -> Self;
}

impl RectangleExt for Rectangle {
    fn center_offset_of(&self, child: Self) -> Point {
        self.center() - child.center()
    }

    fn resized_axis(&self, axis: Axis, value: u32, anchor: Anchor) -> Self {
        match axis {
            Axis::X => self.resized_width(value, anchor.into()),
            Axis::Y => self.resized_height(value, anchor.into()),
        }
    }
}

pub trait PointExt: Sized + Copy {
    fn new_rounded(x: f32, y: f32) -> Self;
    fn new_floor(x: f32, y: f32) -> Self;

    fn swap_axes(self) -> Self;

    fn swap_axes_if(self, cond: bool) -> Self {
        if cond {
            self.swap_axes()
        } else {
            self
        }
    }

    /// Unlike `PartialOrd::clamp` this method does fine-grained clamping per axis.
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

impl ByUnitV2 for CornerRadii {
    type Output = Option<Size>;

    fn by_unit_v(&self, unit_v: UnitV2) -> Self::Output {
        match unit_v.destruct() {
            (-1, -1) => Some(self.top_left.into()),
            (1, -1) => Some(self.top_right.into()),
            (-1, 1) => Some(self.bottom_left.into()),
            (1, 1) => Some(self.bottom_right.into()),
            _ => None,
        }
    }
}

impl TryFrom<Size> for Point {
    type Error = TryFromIntError;

    fn try_from(value: Size) -> Result<Self, Self::Error> {
        Ok(Self::new(value.width.try_into()?, value.height.try_into()?))
    }
}
