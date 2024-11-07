use super::{ColorStyle, WidgetStyle};
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
    pub fn circle() -> Radius {
        Radius::Percentage(Size::new_equal(0.5))
    }

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

    pub fn into_corner_radii(self, block_size: impl Into<Size>) -> CornerRadii {
        // Note: I use `max_square` for corner radius to make it so "round" as
        // user needs more likely, but this is not really the right way. Better
        // add `BorderRadius::MaxSquare` variant to make it look like a sausage
        // instead of UFO. This is invalid logic!
        // TODO
        let block_size = block_size.into().max_square();
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
    pub color: ColorStyle<C>,
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
        Self {
            color: ColorStyle::DefaultForeground,
            radius: BorderRadius::zero(),
        }
    }

    pub fn color(mut self, color: C) -> Self {
        self.color.set_high_priority(Some(color));
        self
    }

    pub fn radius(mut self, radius: impl Into<BorderRadius>) -> Self {
        self.radius = radius.into();
        self
    }
}

// TODO: Define styles with declare_widget_style for consistency and
//  universality (deep setters such as border_radius)
#[derive(PartialEq)]
pub struct BlockStyle<C: Color> {
    pub background_color: ColorStyle<C>,
    pub border: BorderStyle<C>,
}

impl<C: Color> WidgetStyle for BlockStyle<C> {
    type Color = C;
    type Inputs = ();
}

impl<C: Color> Clone for BlockStyle<C> {
    fn clone(&self) -> Self {
        Self {
            background_color: self.background_color.clone(),
            border: self.border.clone(),
        }
    }
}

impl<C: Color> Copy for BlockStyle<C> {}

impl<C: Color> BlockStyle<C> {
    pub fn base() -> Self {
        Self {
            background_color: ColorStyle::Unset,
            border: BorderStyle::base(),
        }
    }

    pub fn background_color(mut self, background_color: C) -> Self {
        self.background_color.set_high_priority(Some(background_color));
        self
    }

    pub fn border(mut self, border: BorderStyle<C>) -> Self {
        self.border = border;
        self
    }
}
