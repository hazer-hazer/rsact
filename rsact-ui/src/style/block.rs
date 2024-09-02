use crate::{layout::size::Size, render::color::Color};
use embedded_graphics::primitives::CornerRadii;

#[derive(Clone, Copy, Debug, PartialEq)]
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

impl From<Size> for Radius {
    fn from(value: Size) -> Self {
        Self::Size(value)
    }
}

impl From<u32> for Radius {
    fn from(value: u32) -> Self {
        Self::SizeEqual(value)
    }
}

impl From<f32> for Radius {
    fn from(value: f32) -> Self {
        Self::PercentageEqual(value)
    }
}

impl From<Size<f32>> for Radius {
    fn from(value: Size<f32>) -> Self {
        Self::Percentage(value)
    }
}

impl From<(f32, f32)> for Radius {
    fn from(value: (f32, f32)) -> Self {
        Self::Percentage(Size::new(value.0, value.1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
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

    pub fn zero() -> Self {
        Self::new_equal(Radius::SizeEqual(0))
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

#[derive(PartialEq)]
pub struct BorderStyle<C: Color> {
    pub color: Option<C>,
    pub radius: BorderRadius,
}

impl<C: Color> Clone for BorderStyle<C> {
    fn clone(&self) -> Self {
        Self { color: self.color.clone(), radius: self.radius.clone() }
    }
}

impl<C: Color> Copy for BorderStyle<C> {}

impl<C: Color> BorderStyle<C> {
    pub fn base() -> Self {
        Self { color: None, radius: BorderRadius::zero() }
    }

    pub fn color(mut self, color: C) -> Self {
        self.color = Some(color);
        self
    }
}

#[derive(PartialEq)]
pub struct BoxStyle<C: Color> {
    pub background_color: Option<C>,
    pub border: BorderStyle<C>,
}

impl<C: Color> Clone for BoxStyle<C> {
    fn clone(&self) -> Self {
        Self {
            background_color: self.background_color.clone(),
            border: self.border.clone(),
        }
    }
}

impl<C: Color> Copy for BoxStyle<C> {}

impl<C: Color> BoxStyle<C> {
    pub fn base() -> Self {
        Self { background_color: None, border: BorderStyle::base() }
    }

    pub fn background_color(mut self, background_color: C) -> Self {
        self.background_color = Some(background_color);
        self
    }

    pub fn border(mut self, border: BorderStyle<C>) -> Self {
        self.border = border;
        self
    }
}
