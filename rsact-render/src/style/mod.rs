use crate::color::Color;

pub mod block;

/**
 * Prioritized extended Option type for colors.
 * [`ColorStyle::Unset`], [`ColorStyle::LowPriority`] and
 * `ColorStyle::Default*` are low-priority option, which are overwritten by
 * any new non-Unset color. HighPriority and Transparent are only
 * overwritten by new high-priority color.
 * Unset and Default* are the lowest priority
 * variant which cannot be set, and only must be used as initial value.
 */
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorStyle<C: Color> {
    /// Color is unset
    Unset,
    /// Color is set to transparent. High priority
    Transparent,
    /// Color set with low priority
    LowPriority(C),
    /// Color set with high priority
    HighPriority(C),
    /// Color is unset and will fallback to default foreground
    DefaultForeground,
    /// Color is unset and will fallback to default background
    DefaultBackground,
}

impl<C: Color> ColorStyle<C> {
    pub fn get(self) -> Option<C> {
        match self {
            ColorStyle::Unset => None,
            ColorStyle::Transparent => None,
            ColorStyle::LowPriority(color)
            | ColorStyle::HighPriority(color) => Some(color),
            ColorStyle::DefaultForeground => Some(C::default_foreground()),
            ColorStyle::DefaultBackground => Some(C::default_background()),
        }
    }

    // TODO: Dangerous, should not work so. Color must always be set to
    // something. Color trait must have unset and transparent case
    // implementation.
    pub fn expect(self) -> C {
        self.get().expect("Color must be set at this point")
    }

    pub fn set_low_priority(&mut self, new: Option<C>) {
        if let Some(new) = new {
            match self {
                Self::DefaultBackground
                | Self::DefaultForeground
                | Self::LowPriority(_)
                | Self::Unset => *self = Self::LowPriority(new),
                _ => {},
            }
        }
    }

    pub fn set_high_priority(&mut self, new: Option<C>) {
        match new {
            Some(color) => {
                *self = Self::HighPriority(color);
            },
            None => {
                *self = Self::Transparent;
            },
        }
    }

    pub fn set_transparent(&mut self) {
        *self = Self::Transparent
    }
}

/// Unified style for drawing filled/stroked primitives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawStyle<C: Color> {
    pub fill: Option<C>,
    pub stroke: Option<C>,
    pub stroke_width: u32,
    pub stroke_alignment: StrokeAlignment,
}

impl<C: Color> Default for DrawStyle<C> {
    fn default() -> Self {
        Self {
            fill: None,
            stroke: None,
            stroke_width: 0,
            stroke_alignment: StrokeAlignment::Inside,
        }
    }
}

impl<C: Color> DrawStyle<C> {
    // pub fn filled(color: C) -> Self {
    //     Self { fill: Some(color), ..Default::default() }
    // }

    // pub fn stroked(color: C, width: u32) -> Self {
    //     Self { stroke: Some(color), stroke_width: width, ..Default::default()
    // } }

    pub fn fill(mut self, color: C) -> Self {
        self.fill = Some(color);
        self
    }

    pub fn stroke(mut self, color: C) -> Self {
        self.stroke = Some(color);
        self
    }

    pub fn stroke_width(mut self, width: u32) -> Self {
        self.stroke_width = width;
        self
    }

    // pub fn fill_only(&self) -> Self {
    //     Self {
    //         fill: self.fill,
    //         stroke: None,
    //         stroke_width: 0,
    //         stroke_alignment: Default::default(),
    //     }
    // }

    // pub fn stroke_only(&self) -> Self {
    //     Self {
    //         fill: None,
    //         stroke: self.stroke,
    //         stroke_width: self.stroke_width,
    //         stroke_alignment: self.stroke_alignment,
    //     }
    // }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeAlignment {
    Inside,
    Center,
    Outside,
}

impl Default for StrokeAlignment {
    fn default() -> Self {
        Self::Inside
    }
}
