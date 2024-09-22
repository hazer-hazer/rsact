use super::{color::Color, Block, Renderer};
use crate::{layout::size::Size, widget::DrawResult};
use alloc::vec::Vec;
use core::convert::Infallible;
use embedded_canvas::CanvasAt;
use embedded_graphics::{
    image::{Image, ImageRaw},
    iterator::raw::RawDataSlice,
    pixelcolor::raw::ByteOrder,
    prelude::{Dimensions, DrawTarget, DrawTargetExt, PixelColor, Point},
    primitives::{
        PrimitiveStyleBuilder, Rectangle, RoundedRectangle, StyledDrawable as _,
    },
};
use embedded_graphics_core::Drawable as _;

#[derive(Clone, Copy, Debug)]
pub enum Layer {
    Normal,
    Clipped(Rectangle),
    Cropped(Rectangle),
}

pub struct LayeringRenderer<C: Color> {
    layers: Vec<Layer>,
    canvas: CanvasAt<C>,
}

impl<C: Color> Dimensions for LayeringRenderer<C> {
    fn bounding_box(&self) -> Rectangle {
        self.canvas.bounding_box()
    }
}

impl<C: Color> DrawTarget for LayeringRenderer<C> {
    type Color = C;

    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        match self.layers.last().unwrap() {
            Layer::Normal => self.canvas.draw_iter(pixels),
            Layer::Clipped(area) => self.canvas.clipped(area).draw_iter(pixels),
            Layer::Cropped(area) => self.canvas.cropped(area).draw_iter(pixels),
        }
    }
}

impl<C: Color> Renderer for LayeringRenderer<C>
where
    C: Default,
{
    type Color = C;

    fn new(viewport: Size) -> Self {
        Self {
            layers: vec![Layer::Normal],
            canvas: CanvasAt::new(Point::zero(), viewport.into()),
        }
    }

    fn finish(&self, target: &mut impl DrawTarget<Color = C>) {
        self.canvas.draw(target).ok().unwrap();
    }

    fn clear(&mut self, color: Self::Color) -> DrawResult {
        DrawTarget::clear(self, color).ok().unwrap();
        Ok(())
    }

    fn clipped(
        &mut self,
        area: Rectangle,
        f: impl FnOnce(&mut Self) -> DrawResult,
    ) -> DrawResult {
        self.layers.push(Layer::Clipped(area));
        let result = f(self);
        self.layers.pop();
        result
    }

    fn line(
        &mut self,
        line: embedded_graphics::primitives::Styled<
            embedded_graphics::primitives::Line,
            embedded_graphics::primitives::PrimitiveStyle<Self::Color>,
        >,
    ) -> DrawResult {
        line.draw(self).ok().unwrap();
        Ok(())
    }

    fn rect(
        &mut self,
        rect: embedded_graphics::primitives::Styled<
            RoundedRectangle,
            embedded_graphics::primitives::PrimitiveStyle<Self::Color>,
        >,
    ) -> DrawResult {
        rect.draw(self).ok().unwrap();
        Ok(())
    }

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

        Ok(())
    }

    fn mono_text<'a>(
        &mut self,
        text_box: embedded_text::TextBox<
            'a,
            embedded_graphics::mono_font::MonoTextStyle<'a, Self::Color>,
        >,
    ) -> DrawResult {
        text_box.draw(self).ok().unwrap();

        Ok(())
    }

    fn image<'a, BO: ByteOrder>(
        &mut self,
        image: Image<'_, ImageRaw<'a, Self::Color, BO>>,
    ) -> DrawResult
    where
        RawDataSlice<'a, <Self::Color as PixelColor>::Raw, BO>:
            IntoIterator<Item = <Self::Color as PixelColor>::Raw>,
    {
        image.draw(self).ok().unwrap();

        Ok(())
    }

    fn pixel(
        &mut self,
        pixel: embedded_graphics::Pixel<Self::Color>,
    ) -> DrawResult {
        pixel.draw(self).ok().unwrap();
        Ok(())
    }
}
