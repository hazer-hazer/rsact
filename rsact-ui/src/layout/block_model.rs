use super::padding::Padding;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockModel {
    // TODO: Do we need non-equal border widths?
    // pub border: Padding,
    pub border_width: u32,
    pub padding: Padding,
}

impl BlockModel {
    pub fn zero() -> Self {
        Self { border_width: 0, padding: Padding::zero() }
    }

    pub fn full_padding(&self) -> Padding {
        self.padding + Padding::new_equal(self.border_width)
    }

    pub fn border_width(mut self, border_width: u32) -> Self {
        self.border_width = border_width;
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }
}
