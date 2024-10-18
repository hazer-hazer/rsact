use crate::{
    layout::{block_model::BlockModel, padding::Padding, size::Size},
    style::block::{BlockStyle, BorderRadius, BorderStyle},
    widget::DrawResult,
};
use color::Color;
use embedded_graphics::{
    image::{Image, ImageRaw},
    iterator::raw::RawDataSlice,
    mono_font::MonoTextStyle,
    pixelcolor::raw::ByteOrder,
    prelude::{DrawTarget, PixelColor},
    primitives::{PrimitiveStyle, Rectangle, RoundedRectangle, Styled},
    Pixel,
};
use embedded_text::TextBox;
use rsact_reactive::memo::Memo;

pub mod color;
pub mod draw_target;
pub mod line;

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
pub struct Block<C: Color> {
    pub border: Border<C>,
    pub rect: Rectangle,
    pub background: Option<C>,
}

impl<C: Color + Copy> Block<C> {
    // pub fn new_filled(bounds: Rectangle, background: Option<C>) -> Self {
    //     Self { border: Border::zero(), rect: bounds, background }
    // }

    // TODO: Find better way to construct Block. border width inside layout
    // makes it complex
    #[inline]
    pub fn from_layout_style(
        outer: Rectangle,
        BlockModel { border_width, padding: _ }: BlockModel,
        BlockStyle {
            background_color,
            border: BorderStyle { color: border_color, radius },
        }: BlockStyle<C>,
    ) -> Self {
        Self {
            border: Border {
                color: border_color.get(),
                width: border_width,
                radius,
            },
            rect: outer,
            background: background_color.get(),
        }
    }
}

// TODO: Custom MonoText struct with String to pass from Canvas widget. Lifetime
// in TextBox require Canvas only to draw 'static strings
pub type Line<C> =
    Styled<embedded_graphics::primitives::Line, PrimitiveStyle<C>>;
pub type Rect<C> = Styled<RoundedRectangle, PrimitiveStyle<C>>;
pub type Arc<C> = Styled<embedded_graphics::primitives::Arc, PrimitiveStyle<C>>;

pub trait Renderer {
    type Color: Color;
    type Options: PartialEq + Default;

    fn new(viewport: Size) -> Self;
    fn set_options(&mut self, options: Self::Options);

    // TODO: Generic targets
    // TODO: This is the same as implementing Drawable for Renderer
    fn finish_frame(&self, target: &mut impl DrawTarget<Color = Self::Color>);

    fn clear(&mut self, color: Self::Color) -> DrawResult;
    fn clipped(
        &mut self,
        area: Rectangle,
        f: impl FnOnce(&mut Self) -> DrawResult,
    ) -> DrawResult;
    fn on_layer(
        &mut self,
        index: usize,
        f: impl FnOnce(&mut Self) -> DrawResult,
    ) -> DrawResult;

    fn line(&mut self, line: Line<Self::Color>) -> DrawResult;
    fn rect(&mut self, rect: Rect<Self::Color>) -> DrawResult;
    fn block(&mut self, block: Block<Self::Color>) -> DrawResult;
    fn arc(&mut self, arc: Arc<Self::Color>) -> DrawResult;
    fn mono_text<'a>(
        &mut self,
        text_box: TextBox<'a, MonoTextStyle<'a, Self::Color>>,
    ) -> DrawResult;
    fn image<'a, BO: ByteOrder>(
        &mut self,
        image: Image<'_, ImageRaw<'a, Self::Color, BO>>,
    ) -> DrawResult
    where
        RawDataSlice<'a, <Self::Color as PixelColor>::Raw, BO>:
            IntoIterator<Item = <Self::Color as PixelColor>::Raw>;

    fn pixel_iter(
        &mut self,
        mut pixels: impl Iterator<Item = Pixel<Self::Color>>,
    ) -> DrawResult {
        pixels.try_for_each(|pixel| self.pixel(pixel))?;

        Ok(())
    }

    fn translucent_pixel_iter(
        &mut self,
        mut pixels: impl Iterator<Item = Option<Pixel<Self::Color>>>,
    ) -> DrawResult {
        pixels.try_for_each(|pixel| {
            if let Some(pixel) = pixel {
                self.pixel(pixel)
            } else {
                Ok(())
            }
        })?;

        Ok(())
    }

    fn pixel(&mut self, pixel: Pixel<Self::Color>) -> DrawResult;
}
