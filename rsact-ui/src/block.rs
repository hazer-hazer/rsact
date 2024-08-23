use embedded_graphics::primitives::{CornerRadii, Rectangle};

use crate::padding::Padding;
use crate::render::color::Color;
use crate::size::Size;

#[derive(Clone, Copy)]
pub struct BoxModel {
    pub border: Padding,
    pub padding: Padding,
}

impl BoxModel {
    pub fn new() -> Self {
        Self { border: Padding::zero(), padding: Padding::zero() }
    }

    pub fn border(mut self, border: impl Into<Padding>) -> Self {
        self.border = border.into();
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Radius {
    Size(Size),
    SizeEqual(u32),
    Percentage(Size<f32>),
    PercentageEqual(f32),
}

impl Radius {
    pub fn into_real(
        self,
        corner_size: Size,
    ) -> embedded_graphics_core::geometry::Size {
        match self {
            Radius::Size(size) => size,
            Radius::SizeEqual(size) => Size::new_equal(size),
            Radius::Percentage(percentage) => corner_size * percentage,
            Radius::PercentageEqual(percentage) => corner_size * percentage,
        }
        .min(corner_size)
        .into()
    }
}

/// Radius x,y
impl From<Size> for Radius {
    fn from(value: Size) -> Self {
        Self::Size(value)
    }
}

/// Equal Radius r,r
impl From<u32> for Radius {
    fn from(value: u32) -> Self {
        Self::SizeEqual(value)
    }
}

/// Equal Radius in percentage p,p
impl From<f32> for Radius {
    fn from(value: f32) -> Self {
        Self::PercentageEqual(value)
    }
}

/// Radius in percentage px,py
impl From<Size<f32>> for Radius {
    fn from(value: Size<f32>) -> Self {
        Self::Percentage(value)
    }
}

/// Shortcut for radius in percentage (px,py)
impl From<(f32, f32)> for Radius {
    fn from(value: (f32, f32)) -> Self {
        Self::Percentage(Size::new(value.0, value.1))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BorderRadius {
    pub top_left: Radius,
    pub top_right: Radius,
    pub bottom_right: Radius,
    pub bottom_left: Radius,
}

impl BorderRadius {
    pub fn new(
        top_left: Radius,
        top_right: Radius,
        bottom_right: Radius,
        bottom_left: Radius,
    ) -> Self {
        Self { top_left, top_right, bottom_right, bottom_left }
    }

    pub fn new_equal(ellipse: Radius) -> Self {
        Self::new(ellipse, ellipse, ellipse, ellipse)
    }

    pub fn into_corner_radii(self, block_size: Size) -> CornerRadii {
        CornerRadii {
            top_left: self.top_left.into_real(block_size),
            top_right: self.top_right.into_real(block_size),
            bottom_right: self.bottom_right.into_real(block_size),
            bottom_left: self.bottom_left.into_real(block_size),
        }
    }
}

// impl Into<CornerRadii> for BorderRadius {
//     fn into(self) -> CornerRadii {
//         CornerRadii {
//             top_left: self.top_left.into(),
//             top_right: self.top_right.into(),
//             bottom_right: self.bottom_right.into(),
//             bottom_left: self.bottom_left.into(),
//         }
//     }
// }

impl<T> From<T> for BorderRadius
where
    T: Into<Radius>,
{
    fn from(value: T) -> Self {
        Self::new_equal(value.into())
    }
}

impl<T> From<[T; 4]> for BorderRadius
where
    T: Into<Radius> + Copy,
{
    fn from(value: [T; 4]) -> Self {
        Self::new(
            value[0].into(),
            value[1].into(),
            value[2].into(),
            value[3].into(),
        )
    }
}

impl Default for BorderRadius {
    fn default() -> Self {
        Self::new_equal(Radius::SizeEqual(0))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Border<C: Color>
where
    C: Copy,
{
    pub color: C,
    pub width: u32,
    pub radius: BorderRadius,
}

impl<C: Color> Border<C> {
    pub fn new() -> Self {
        Self {
            color: C::default_foreground(),
            width: 1,
            radius: BorderRadius::default(),
        }
    }

    pub fn zero() -> Self {
        Self { color: C::default_foreground(), width: 0, radius: 0.into() }
    }

    pub fn color(mut self, color: impl Into<C>) -> Self {
        self.color = color.into();
        self
    }

    pub fn width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    pub fn radius(mut self, radius: impl Into<BorderRadius>) -> Self {
        self.radius = radius.into();
        self
    }

    // Make Block for border used as outline. Background color is always removed
    // to avoid drawing above element.
    pub fn into_outline(self, bounds: Rectangle) -> Block<C> {
        // FIXME: Wrong transparent
        Block { rect: bounds, background: None, border: self }
    }

    pub fn into_block(
        self,
        bounds: Rectangle,
        background: Option<C>,
    ) -> Block<C> {
        Block { rect: bounds, background, border: self }
    }
}

impl<C: Color> Into<Padding> for Border<C> {
    fn into(self) -> Padding {
        self.width.into()
    }
}

#[derive(Clone, Copy)]
pub struct Block<C: Color + Copy> {
    pub border: Border<C>,
    pub rect: Rectangle,
    pub background: Option<C>,
}

impl<C: Color + Copy> Block<C> {
    pub fn new_filled(bounds: Rectangle, background: Option<C>) -> Self {
        Self { border: Border::zero(), rect: bounds, background }
    }
}
