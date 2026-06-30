use super::length::{DeterministicLength, Length};
use crate::{
    layout::{ContentSizing, length::LengthSize},
    render::prelude::*,
};
use core::{fmt::Display, u32};
use rsact_render::geometry::padding::Padding;

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

    pub fn child_limits(self, size: impl Into<LengthSize>) -> Self {
        let size = size.into();

        self.limit_axis(Axis::X, size.width(), true).limit_axis(
            Axis::Y,
            size.height(),
            true,
        )
    }

    pub fn limit_axis(
        self,
        axis: Axis,
        length: impl Into<Length>,
        child_limits: bool,
    ) -> Self {
        match length.into() {
            Length::InfiniteWindow(dl) => {
                if child_limits {
                    self.with_max(axis.canon(u32::MAX, self.max.cross(axis)))
                } else {
                    self.limit_axis(axis, dl, child_limits)
                }
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

                // TODO: This doesn't seem to be right to set min limit to the max limit for Fixed length.
                Self::new(
                    self.min.with_main(axis, new_length),
                    self.max.with_main(axis, new_length),
                )
            },
        }
    }

    /// Unlike `child_limits` won't produce "infinite" limit for
    /// `InfiniteWindow` length
    pub fn self_limits(self, size: LengthSize) -> Self {
        self.limit_axis(Axis::X, size.width(), false).limit_axis(
            Axis::Y,
            size.height(),
            false,
        )
    }

    pub fn shrink(self, by: impl Into<Size>) -> Self {
        let by = by.into();

        Limits::new(self.min() - by, self.max() - by)
    }

    fn resolve_axis(
        &self,
        axis: Axis,
        container_size: LengthSize,
        content_size: Size,
    ) -> u32 {
        let min = self.min.main(axis);
        let max = self.max.main(axis);

        // Unclamped resolved length for this axis, before applying the limits.
        let raw = match container_size.main(axis) {
            Length::Shrink
            | Length::InfiniteWindow(DeterministicLength::Shrink) => {
                content_size.main(axis)
            },
            // TODO: Review percent semantics (rounding, whether it's relative
            // to max or to the resolved parent size).
            Length::Pct(pct) => (max as f32 * pct) as u32,
            Length::Div(_)
            | Length::InfiniteWindow(DeterministicLength::Div(_)) => max,
            Length::Fixed(fixed)
            | Length::InfiniteWindow(DeterministicLength::Fixed(fixed)) => {
                fixed
            },
        };

        // Clamp into the limits uniformly for every length kind, so the `min`
        // limit is enforced for Fixed/Div/Pct exactly as it has always been for
        // Shrink. `min` wins when the limits are inverted (`min > max`), which
        // is reachable for a fluid child whose content min exceeds its computed
        // share (see `Limits::new` call in `model_flex`).
        raw.min(max).max(min)
    }

    /// Resolve a content leaf whose block-axis (height) extent depends on its
    /// resolved inline-axis (width). The width is resolved first from the width
    /// range in `sizing` (`Shrink` takes `max_content`, `Fill`/`Fixed`/`Pct`
    /// as usual), then `height_for_width(width)` gives the content height and
    /// the height axis resolves from that. Used by the `Content` arm of layout
    /// modeling.
    pub fn resolve_content_size(
        &self,
        size: LengthSize,
        sizing: &ContentSizing,
        height_for_width: impl Fn(u32) -> u32,
    ) -> Size<u32> {
        let self_limits = self.self_limits(size);
        // Inline (width) axis resolves from the max-content width: `Shrink`
        // clamps it into limits, `Fill`/`Pct`/`Fixed` behave as usual.
        let width = self_limits.resolve_axis(
            Axis::X,
            size,
            Size::new(sizing.max_content, 0),
        );
        // Block (height) axis resolves from the wrapped height at that width.
        let height = self_limits.resolve_axis(
            Axis::Y,
            size,
            Size::new(0, height_for_width(width)),
        );
        Size::new(width, height)
    }

    pub fn resolve_size(
        &self,
        size: LengthSize,
        content_size: Size<u32>,
        full_padding: Option<Padding>,
    ) -> Size<u32> {
        let self_limits = self.self_limits(size);
        let self_limits = if let Some(full_padding) = full_padding {
            self_limits.shrink(full_padding)
        } else {
            self_limits
        };
        Size::new(
            self_limits.resolve_axis(Axis::X, size, content_size),
            self_limits.resolve_axis(Axis::Y, size, content_size),
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

#[cfg(feature = "embedded-graphics")]
impl From<embedded_graphics::primitives::Rectangle> for Limits {
    fn from(value: embedded_graphics::primitives::Rectangle) -> Self {
        Self::new(Size::zero(), value.size.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::ContentSizing;

    fn sizing(min: u32, max: u32, line_height: u32) -> ContentSizing {
        ContentSizing { min_content: min, max_content: max, line_height }
    }

    // Stand-in for a font's wrapping: one line at/above the unwrapped width,
    // two lines below it.
    fn height_for_width(
        unwrapped: u32,
        line_height: u32,
    ) -> impl Fn(u32) -> u32 {
        move |width| {
            if width >= unwrapped { line_height } else { line_height * 2 }
        }
    }

    #[test]
    fn shrink_text_takes_unwrapped_width_on_one_line() {
        let limits = Limits::only_max(Size::new(200, 100));
        let size = LengthSize::shrink();
        let resolved = limits.resolve_content_size(
            size,
            &sizing(20, 60, 10),
            height_for_width(60, 10),
        );
        assert_eq!(resolved, Size::new(60, 10));
    }

    #[test]
    fn fill_width_wraps_into_available_and_grows_height() {
        let limits = Limits::only_max(Size::new(40, 100));
        let mut size = LengthSize::shrink();
        size.set_width(Length::Div(1)); // fill width, shrink height
        let resolved = limits.resolve_content_size(
            size,
            &sizing(20, 60, 10),
            height_for_width(60, 10),
        );
        assert_eq!(resolved, Size::new(40, 20));
    }

    #[test]
    fn fixed_width_clamps_then_derives_wrapped_height() {
        let limits = Limits::only_max(Size::new(200, 100));
        let mut size = LengthSize::shrink();
        size.set_width(Length::Fixed(30));
        let resolved = limits.resolve_content_size(
            size,
            &sizing(20, 60, 10),
            height_for_width(60, 10),
        );
        assert_eq!(resolved, Size::new(30, 20));
    }

    #[test]
    fn fixed_is_clamped_up_to_min_limit() {
        // A `Fixed` length below the `min` limit is clamped UP to `min`, just
        // like `Shrink` always was (previously the min limit was suppressed for
        // Fixed, so this returned 30).
        let limits = Limits::new(Size::new(50, 0), Size::new(100, 100));
        let mut size = LengthSize::shrink();
        size.set_width(Length::Fixed(30));
        let resolved = limits.resolve_size(size, Size::zero(), None);
        assert_eq!(resolved.width, 50);
    }

    #[test]
    fn inverted_limits_resolve_to_min_uniformly() {
        // `min > max` is reachable for a fluid child whose content min exceeds
        // its computed share. `min` wins regardless of the length kind (here a
        // fill width); previously `Div` returned `max` (50), ignoring `min`.
        let limits = Limits::new(Size::new(80, 0), Size::new(50, 100));
        let mut size = LengthSize::shrink();
        size.set_width(Length::Div(1));
        let resolved = limits.resolve_size(size, Size::zero(), None);
        assert_eq!(resolved.width, 80);
    }
}
