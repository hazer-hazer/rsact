use crate::render::prelude::*;
use core::{
    fmt::Display,
    ops::{Add, AddAssign, Div, Mul, Rem},
};
use rsact_reactive::prelude::*;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LengthSize(Size<Length>);

impl From<Size<Length>> for LengthSize {
    fn from(value: Size<Length>) -> Self {
        Self(value)
    }
}

impl Axial for LengthSize {
    type Data = Length;

    fn x(&self) -> Self::Data {
        self.width()
    }

    fn y(&self) -> Self::Data {
        self.height()
    }

    fn x_mut(&mut self) -> &mut Self::Data {
        &mut self.0.width
    }

    fn y_mut(&mut self) -> &mut Self::Data {
        &mut self.0.height
    }

    fn axial_new(x: Self::Data, y: Self::Data) -> Self {
        Self::new(x, y)
    }
}

impl LengthSize {
    pub fn fixed_zero() -> Self {
        Self::new(Length::Fixed(0), Length::Fixed(0))
    }

    #[inline(always)]
    pub fn width(&self) -> Length {
        self.0.width
    }

    #[inline(always)]
    pub fn height(&self) -> Length {
        self.0.height
    }

    pub fn set_width(&mut self, width: Length) {
        self.0.width = width;
    }

    pub fn set_height(&mut self, height: Length) {
        self.0.height = height;
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

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub enum DeterministicLength {
    Shrink,
    Div(u16),
    Fixed(u32),
    // TODO: Are percents deterministic?
    // Pct(f32),
}

impl TryFrom<Length> for DeterministicLength {
    type Error = ();

    fn try_from(value: Length) -> Result<Self, Self::Error> {
        match value {
            Length::Shrink => Ok(Self::Shrink),
            Length::Div(div) => Ok(Self::Div(div)),
            Length::Fixed(fixed) => Ok(Self::Fixed(fixed)),
            Length::Pct(_) | Length::InfiniteWindow(_) => Err(()),
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
            // DeterministicLength::Pct(pct) => Length::Pct(pct),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, IntoMaybeReactive)]
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

    // non_exhaustive to allow creation only through check constructor
    #[non_exhaustive]
    /// Percent of parent length
    Pct(f32),

    /// Only available for special internal layouts such as Scrollable
    #[non_exhaustive]
    InfiniteWindow(DeterministicLength),
    // /// Fixed scrollable window
    // Scroll(Length),
}

impl Display for Length {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Shrink => write!(f, "Shrink"),
            Self::Div(div) if *div == 1 => write!(f, "Fill"),
            Self::Div(div) => write!(f, "Div({div})"),
            Self::Fixed(fixed) if *fixed == u32::MAX => {
                write!(f, "Inf")
            },
            Self::Fixed(fixed) => write!(f, "{fixed}"),
            Self::Pct(pct) => write!(f, "{pct}%"),
            Self::InfiniteWindow(length) => {
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

    /// Percent value in range 0..=1.0
    pub fn pct(pct: f32) -> Self {
        assert!(pct >= 0.0 && pct <= 1.0);

        Self::Pct(pct)
    }

    // pub fn div_factor(&self) -> u16 {
    //     match self {
    //         Self::InfiniteWindow(length) =>
    // length.into_length().div_factor(),         Self::Fixed(_) |
    // Self::Shrink | Self::Pct(_) => 0,         Self::Div(div) => *div,
    //     }
    // }

    pub fn div_factor(&self) -> Option<u16> {
        match self {
            Self::InfiniteWindow(length) => length.into_length().div_factor(),
            Self::Fixed(_) | Self::Shrink | Self::Pct(_) => None,
            Self::Div(div) => Some(*div),
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
        if let Self::InfiniteWindow(_) = self {
            *self = Self::InfiniteWindow(
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
            Self::InfiniteWindow(length) => length.into_length().is_grow(),
            Self::Div(_) => true,
            Self::Shrink | Self::Fixed(_) | Self::Pct(_) => false,
        }
    }

    pub fn into_fixed(&self, base_div: u32) -> u32 {
        match self {
            Self::InfiniteWindow(length) => {
                length.into_length().into_fixed(base_div)
            },
            // TODO: This might not be right
            Self::Pct(pct) => (base_div as f32 * pct) as u32,
            Self::Shrink => base_div,
            &Self::Div(div) => base_div * div as u32,
            &Self::Fixed(fixed) => fixed,
        }
    }

    pub fn div_into_fixed(div: u16, base_div: u32) -> u32 {
        base_div * div as u32
    }

    pub fn max_fixed(&self, max: u32, max_size: u32) -> u32 {
        match self {
            Self::InfiniteWindow(length) => {
                length.into_length().max_fixed(max, max_size)
            },
            Self::Pct(pct) => (max_size as f32 * pct) as u32,
            Self::Shrink | Self::Div(_) => max,
            &Self::Fixed(fixed) => fixed.max(max),
        }
    }
}

impl From<u32> for Length {
    fn from(value: u32) -> Self {
        Self::Fixed(value)
    }
}

#[cfg(feature = "embedded-graphics")]
impl From<embedded_graphics_core::geometry::Size> for LengthSize {
    fn from(value: embedded_graphics_core::geometry::Size) -> Self {
        Self::new(Length::Fixed(value.width), Length::Fixed(value.height))
    }
}

impl LengthSize {
    fn new(width: Length, height: Length) -> Self {
        Self(Size::new(width, height))
    }

    pub fn fixed_length(width: u32, height: u32) -> Self {
        Self::new(Length::Fixed(width), Length::Fixed(height))
    }

    pub fn shrink() -> Self {
        Self::new(Length::Shrink, Length::Shrink)
    }

    pub fn fill() -> Self {
        Self::new(Length::Div(1), Length::Div(1))
    }

    // pub fn is_fixed(&self) -> bool {
    //     self.width.is_fixed() && self.height.is_fixed()
    // }

    // pub fn is_fill(&self) -> bool {
    //     self.width.is_fill() && self.height.is_fill()
    // }

    pub fn in_parent(self, parent: Self) -> Self {
        Self::new(
            self.0.width.in_parent(parent.0.width),
            self.0.height.in_parent(parent.0.height),
        )
    }

    pub fn div_factors(&self) -> DivFactors {
        DivFactors {
            width: self.0.width.div_factor().unwrap_or(0),
            height: self.0.height.div_factor().unwrap_or(0),
        }
    }

    pub fn max_fixed(&self, fixed: Size, max_size: Size) -> Size {
        Size::new(
            self.0.width.max_fixed(fixed.width, max_size.width),
            self.0.height.max_fixed(fixed.height, max_size.height),
        )
    }

    pub fn into_fixed(&self, base_divs: Size) -> Size {
        Size::new(
            self.0.width.into_fixed(base_divs.width),
            self.0.height.into_fixed(base_divs.height),
        )
    }
}

impl Into<LengthSize> for Size {
    fn into(self) -> LengthSize {
        LengthSize::new(Length::Fixed(self.width), Length::Fixed(self.height))
    }
}

impl Display for LengthSize {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}x{}", self.0.width, self.0.height)
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
