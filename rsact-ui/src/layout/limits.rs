use super::{
    axis::{Axial, Axis},
    size::{DeterministicLength, Length, Size},
};
use core::{fmt::Display, u32};
use embedded_graphics::primitives::Rectangle;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Limits {
    min: Size<u32>,
    max: Size<u32>,
}

impl Limits {
    pub fn new(min: Size<u32>, max: Size<u32>) -> Self {
        Self { min, max }
    }

    pub fn unlimited() -> Self {
        Self { min: Size::zero(), max: Size::new(u32::MAX, u32::MAX) }
    }

    pub fn only_max(max: Size<u32>) -> Self {
        Self { min: Size::zero(), max }
    }

    pub fn exact(exact: Size<u32>) -> Self {
        Self::new(exact, exact)
    }

    pub fn zero() -> Self {
        Self { min: Size::zero(), max: Size::zero() }
    }

    pub fn min(&self) -> Size<u32> {
        self.min
    }

    pub fn max(&self) -> Size<u32> {
        self.max
    }

    // pub fn min_square(&self) -> u32 {
    //     self.min().width.min(self.min().height)
    // }

    // pub fn max_square(&self) -> u32 {
    //     self.max().width.min(self.max().height)
    // }

    pub fn with_min(self, min: Size) -> Self {
        Self::new(min, self.max)
    }

    pub fn with_max(self, max: Size) -> Self {
        Self::new(self.min, max)
    }

    pub fn limit_by(self, size: impl Into<Size<Length>>) -> Self {
        let size = size.into();

        self.limit_axis(Axis::X, size.width).limit_axis(Axis::Y, size.height)
    }

    pub fn limit_axis(self, axis: Axis, length: impl Into<Length>) -> Self {
        match length.into() {
            Length::InfiniteWindow(_) => {
                self.with_max(axis.canon(u32::MAX, self.max.cross(axis)))
            },
            Length::Div(_) | Length::Shrink => {
                // self.with_min(axis.canon(min, self.min.cross(axis)))
                self
            },
            // Length::Shrink => {
            //     self.with_max(axis.canon(min, self.min.cross(axis)))
            // },
            Length::Pct(pct) => self.with_max(axis.canon(
                (self.max.main(axis) as f32 * pct) as u32,
                self.max.cross(axis),
            )),
            Length::Fixed(fixed) => {
                let new_length =
                    fixed.min(self.max.main(axis)).max(self.min.main(axis));

                Self::new(
                    axis.canon(new_length, self.min.cross(axis)),
                    axis.canon(new_length, self.max.cross(axis)),
                )
            },
        }
    }

    pub fn shrink(self, by: impl Into<Size>) -> Self {
        let by = by.into();

        Limits::new(self.min() - by, self.max() - by)
    }

    fn resolve_axis(
        &self,
        axis: Axis,
        container_size: Size<Length>,
        content_size: Size,
    ) -> u32 {
        match container_size.main(axis) {
            Length::Shrink
            | Length::InfiniteWindow(DeterministicLength::Shrink) => {
                content_size
                    .main(axis)
                    .min(self.max.main(axis))
                    .max(self.min.main(axis))
            },
            Length::Pct(pct) => {
                // TODO: Review
                (self.max.main(axis) as f32 * pct) as u32
            },
            Length::Div(_)
            | Length::InfiniteWindow(DeterministicLength::Div(_)) => {
                self.max.main(axis)
            },
            Length::Fixed(fixed)
            | Length::InfiniteWindow(DeterministicLength::Fixed(fixed)) => {
                fixed.min(self.max.main(axis)).max(self.min.main(axis))
            },
        }
    }

    pub fn resolve_size(
        &self,
        container_size: Size<Length>,
        content_size: Size<u32>,
    ) -> Size<u32> {
        Size::new(
            self.resolve_axis(Axis::X, container_size, content_size),
            self.resolve_axis(Axis::Y, container_size, content_size),
        )
    }

    // pub fn resolve_square(&self, size: impl Into<Length>) -> u32 {
    //     let min_square = self.min_square();
    //     let max_square = self.max_square();

    //     match size.into() {
    //         Length::InfiniteWindow(_) => max_square,
    //         Length::Div(_) => max_square,
    //         Length::Fixed(fixed) => fixed.min(max_square).max(min_square),
    //         Length::Shrink => min_square,
    //     }
    // }
}

impl Display for Limits {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "[{}:{}]", self.min, self.max)
    }
}

impl From<Rectangle> for Limits {
    fn from(value: Rectangle) -> Self {
        Self::new(Size::zero(), value.size.into())
    }
}
