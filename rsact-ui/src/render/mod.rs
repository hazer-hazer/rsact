use color::Color;
use embedded_graphics::{
    prelude::DrawTarget,
    primitives::{
        PrimitiveStyle, PrimitiveStyleBuilder, RoundedRectangle, StyledDrawable,
    },
};

use crate::{block::Block, widget::DrawResult};

pub mod color;

pub trait Renderer {
    type Color: Color;

    fn block(&mut self, block: Block<Self::Color>) -> DrawResult;
}

impl<D> Renderer for D
where
    D: DrawTarget,
    D::Color: Color,
{
    type Color = D::Color;

    fn block(&mut self, block: Block<Self::Color>) -> DrawResult {
        let style = PrimitiveStyleBuilder::new()
            .stroke_color(block.border.color)
            .stroke_width(block.border.width);

        let style = if let Some(background) = block.background {
            style.fill_color(background)
        } else {
            style
        };

        RoundedRectangle::new(
            block.rect,
            block.border.radius.into_corner_radii(block.rect.size.into()),
        )
        .draw_styled(&style.build(), self)
        .ok()
        .unwrap();

        // TODO: Errors
        Ok(())
    }
}
