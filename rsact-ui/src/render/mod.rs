use crate::{
    layout::{box_model::BoxModel, padding::Padding, size::Size, Layout},
    style::block::{BorderRadius, BorderStyle, BoxStyle},
    widget::DrawResult,
};
use color::Color;
use embedded_canvas::CanvasAt;
use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::DrawTarget,
    primitives::{Line, PrimitiveStyle, Rectangle, RoundedRectangle, Styled},
};
use embedded_text::TextBox;

pub mod color;
pub mod draw_target;

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

    pub fn new(box_style: BoxStyle<C>, box_model: BoxModel) -> Self {
        Self {
            color: box_style.border.color,
            width: box_model.border_width,
            radius: box_style.border.radius,
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

    fn new(viewport: Size) -> Self;

    // TODO: Generic targets
    fn finish(&self, target: &mut impl DrawTarget<Color = Self::Color>);

    fn clear(&mut self, color: Self::Color) -> DrawResult;
    fn clipped(
        &mut self,
        area: Rectangle,
        f: impl FnOnce(&mut Self) -> DrawResult,
    ) -> DrawResult;

    fn line(
        &mut self,
        line: Styled<Line, PrimitiveStyle<Self::Color>>,
    ) -> DrawResult;
    fn rect(
        &mut self,
        rect: Styled<RoundedRectangle, PrimitiveStyle<Self::Color>>,
    ) -> DrawResult;
    fn block(&mut self, block: Block<Self::Color>) -> DrawResult;
    fn mono_text<'a>(
        &mut self,
        text_box: TextBox<'a, MonoTextStyle<'a, Self::Color>>,
    ) -> DrawResult;
}
