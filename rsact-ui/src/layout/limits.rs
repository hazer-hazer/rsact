use embedded_graphics::primitives::Rectangle;

use super::{
    axis::Axis,
    size::{Length, Size},
};

#[derive(Clone, Copy)]
pub struct Limits {
    min: Size<u32>,
    max: Size<u32>,
}

impl Limits {
    pub fn new(min: Size<u32>, max: Size<u32>) -> Self {
        Self { min, max }
    }

    pub fn unknown() -> Self {
        Self { min: Size::zero(), max: Size::new(u32::MAX, u32::MAX) }
    }

    pub fn only_max(max: Size<u32>) -> Self {
        Self { min: Size::zero(), max }
    }

    pub fn min(&self) -> Size<u32> {
        self.min
    }

    pub fn max(&self) -> Size<u32> {
        self.max
    }

    pub fn min_square(&self) -> u32 {
        self.min().width.min(self.min().height)
    }

    pub fn max_square(&self) -> u32 {
        self.max().width.min(self.max().height)
    }

    pub fn with_max(self, max: Size) -> Self {
        Self::new(self.min, max)
    }

    pub fn limit_by(self, size: impl Into<Size<Length>>) -> Self {
        let size = size.into();

        self.limit_axis(Axis::X, size.width).limit_axis(Axis::Y, size.height)
    }

    pub fn limit_width(self, width: impl Into<Length>) -> Self {
        match width.into() {
            Length::Shrink | Length::Div(_) => self,
            Length::Fixed(fixed) => {
                let new_width = fixed.min(self.max.width).max(self.min.width);

                Self::new(
                    self.min.with_width(new_width),
                    self.max.with_width(new_width),
                )
            },
        }
    }

    pub fn limit_height(self, height: impl Into<Length>) -> Self {
        match height.into() {
            Length::Shrink | Length::Div(_) => self,
            Length::Fixed(fixed) => {
                let new_height =
                    fixed.min(self.max.height).max(self.min.height);

                Self::new(
                    self.min.with_height(new_height),
                    self.max.with_height(new_height),
                )
            },
        }
    }

    pub fn limit_axis(self, axis: Axis, length: impl Into<Length>) -> Self {
        match axis {
            Axis::X => self.limit_width(length),
            Axis::Y => self.limit_height(length),
        }
    }

    pub fn shrink(self, by: impl Into<Size>) -> Self {
        let by = by.into();

        Limits::new(self.min() - by, self.max() - by)
    }

    pub fn resolve_size(
        &self,
        container_size: Size<Length>,
        content_size: Size<u32>,
    ) -> Size<u32> {
        let width = match container_size.width {
            Length::Div(_) => self.max.width,
            Length::Fixed(fixed) => {
                fixed.min(self.max.width).max(self.min.width)
            },
            Length::Shrink => {
                content_size.width.min(self.max.width).max(self.min.width)
            },
        };

        let height = match container_size.height {
            Length::Div(_) => self.max.height,
            Length::Fixed(fixed) => {
                fixed.min(self.max.height).max(self.min.height)
            },
            Length::Shrink => {
                content_size.height.min(self.max.height).max(self.min.height)
            },
        };

        Size::new(width, height)
    }

    pub fn resolve_square(&self, size: impl Into<Length>) -> u32 {
        let min_square = self.min_square();
        let max_square = self.max_square();

        match size.into() {
            Length::Div(_) => max_square,
            Length::Fixed(fixed) => fixed.min(max_square).max(min_square),
            Length::Shrink => min_square,
        }
    }
}

impl From<Rectangle> for Limits {
    fn from(value: Rectangle) -> Self {
        Self::new(Size::zero(), value.size.into())
    }
}
