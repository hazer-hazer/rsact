use crate::{
    layout::{box_model::BoxModel, padding::Padding},
    style::{BorderRadius, BorderStyle, BoxStyle},
    widget::DrawResult,
};
use color::Color;
use embedded_graphics::{
    prelude::DrawTarget,
    primitives::{
        PrimitiveStyleBuilder, Rectangle, RoundedRectangle, StyledDrawable,
    },
};

pub mod color;

#[derive(Debug, Clone, Copy)]
pub struct Border<C: Color>
where
    C: Copy,
{
    pub color: Option<C>,
    pub width: u32,
    pub radius: BorderRadius,
}

impl<C: Color> Border<C> {
    // pub fn new() -> Self {
    //     Self { color: None, width: 1, radius: BorderRadius::zero() }
    // }

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

    // Make Block for border used as outline. Background color is always removed
    // to avoid drawing above element.
    pub fn into_outline(self, bounds: Rectangle) -> Block<C> {
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
    // pub fn new_filled(bounds: Rectangle, background: Option<C>) -> Self {
    //     Self { border: Border::zero(), rect: bounds, background }
    // }

    #[inline]
    pub fn from_layout_style(
        area: Rectangle,
        BoxModel { border_width, padding: _ }: BoxModel,
        BoxStyle {
            background_color,
            border: BorderStyle { color: border_color, radius },
        }: BoxStyle<C>,
    ) -> Self {
        Self {
            border: Border { color: border_color, width: border_width, radius },
            rect: area,
            background: background_color,
        }
    }
}

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
        let style =
            PrimitiveStyleBuilder::new().stroke_width(block.border.width);

        let style = if let Some(border_color) = block.border.color {
            style.stroke_color(border_color)
        } else {
            style
        };

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
