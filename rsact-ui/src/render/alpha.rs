use embedded_graphics::{
    image::{Image, ImageDrawable},
    prelude::DrawTarget,
    primitives::{PrimitiveStyle, Rectangle, Styled},
    text::renderer::{CharacterStyle, TextRenderer},
    Drawable, Pixel,
};
use embedded_text::TextBox;

use crate::widget::DrawResult;

use super::color::Color;

pub trait AlphaDrawTarget: DrawTarget {
    fn pixel_alpha(
        &mut self,
        pixel: Pixel<Self::Color>,
        blend: f32,
    ) -> DrawResult;
}

#[cfg(feature = "simulator")]
impl<C: Color> AlphaDrawTarget
    for embedded_graphics_simulator::SimulatorDisplay<C>
{
    fn pixel_alpha(
        &mut self,
        pixel: Pixel<Self::Color>,
        blend: f32,
    ) -> DrawResult {
        let color = self.get_pixel(pixel.0).mix(blend, pixel.1);
        Pixel(pixel.0, color).draw(self).unwrap();
        Ok(())
    }
}

pub trait AlphaDrawable {
    type Color: Color;

    fn draw_alpha<A>(&self, target: &mut A) -> DrawResult
    where
        A: AlphaDrawTarget<Color = Self::Color>;
}

pub trait StyledAlphaDrawable<S> {
    type Color: Color;
    type Output;

    fn draw_styled_alpha<D>(&self, style: &S, target: &mut D) -> DrawResult
    where
        D: AlphaDrawTarget<Color = Self::Color>;
}

impl<P: StyledAlphaDrawable<S>, S> AlphaDrawable for Styled<P, S> {
    type Color = P::Color;

    fn draw_alpha<A>(&self, target: &mut A) -> DrawResult
    where
        A: AlphaDrawTarget<Color = Self::Color>,
    {
        self.primitive.draw_styled_alpha(&self.style, target)
    }
}

// TODO //
impl<C: Color> AlphaDrawable for Styled<Rectangle, PrimitiveStyle<C>> {
    type Color = C;

    fn draw_alpha<A>(&self, target: &mut A) -> DrawResult
    where
        A: AlphaDrawTarget<Color = Self::Color>,
    {
        self.draw(target).ok().unwrap();
        Ok(())
    }
}

impl<'a, C: Color, S: TextRenderer<Color = C> + CharacterStyle<Color = C>>
    AlphaDrawable for TextBox<'a, S>
{
    type Color = C;

    fn draw_alpha<A>(&self, target: &mut A) -> DrawResult
    where
        A: AlphaDrawTarget<Color = Self::Color>,
    {
        self.draw(target).ok().unwrap();
        Ok(())
    }
}

impl<'a, C: Color, T: ImageDrawable<Color = C>> AlphaDrawable for Image<'a, T> {
    type Color = C;

    fn draw_alpha<A>(&self, target: &mut A) -> DrawResult
    where
        A: AlphaDrawTarget<Color = Self::Color>,
    {
        self.draw(target).ok().unwrap();
        Ok(())
    }
}
