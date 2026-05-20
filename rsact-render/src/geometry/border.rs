use crate::{
    color::Color,
    geometry::{Rect, block_model::BlockModel},
    primitives::block::Block,
    style::block::{BlockStyle, BorderRadius},
};

#[derive(Debug, Clone, Copy)]
pub struct Border<C: Color> {
    pub color: Option<C>,
    pub width: u32,
    pub radius: BorderRadius,
}

impl<C: Color> Border<C> {
    pub fn new(block_style: BlockStyle<C>, block_model: BlockModel) -> Self {
        Self {
            color: block_style.border.color.get(),
            width: block_model.border_width,
            radius: block_style.border.radius,
        }
    }

    pub fn zero() -> Self {
        Self { color: None, width: 0, radius: 0.into() }
    }

    pub fn color(mut self, color: Option<C>) -> Self {
        self.color = color;
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

    /// Make Block for border used as outline. Background color is always
    /// removed to avoid drawing above element.
    pub fn into_outline(self, bounds: Rect) -> Block<C> {
        Block { rect: bounds, background: None, border: self }
    }

    pub fn into_block(self, bounds: Rect, background: Option<C>) -> Block<C> {
        Block { rect: bounds, background, border: self }
    }
}

// impl<C: Color> Into<Padding> for Border<C> {
//     fn into(self) -> Padding {
//         self.width.into()
//     }
// }
