use crate::{color::Color, output::pixel::Pixel};
use core::marker::PhantomData;

pub mod pixel;

pub trait RenderTarget {
    type Color;

    fn draw(&mut self, pixels: impl Iterator<Item = Pixel<Self::Color>>);
}

pub trait FinishRender<C> {
    fn finish_frame(&mut self, target: &mut impl RenderTarget<Color = C>);
}

pub trait MapColor<O> {
    fn map_color(&self) -> O;
}

impl<O: Clone> MapColor<O> for O {
    fn map_color(&self) -> O {
        self.clone()
    }
}

pub struct ColorMapper<C: Color, O: Color, T: RenderTarget<Color = O>> {
    target: T,
    _input: PhantomData<C>,
    _output: PhantomData<O>,
}

impl<C: Color, O: Color, T: RenderTarget<Color = O>> ColorMapper<C, O, T> {
    pub fn new(target: T) -> Self {
        Self { target, _input: PhantomData, _output: PhantomData }
    }
}

impl<C: Color, O: Color, T: RenderTarget<Color = O>> RenderTarget
    for ColorMapper<C, O, T>
where
    C: MapColor<O>,
{
    type Color = C;

    fn draw(&mut self, pixels: impl Iterator<Item = Pixel<Self::Color>>) {
        self.target
            .draw(pixels.map(|p| Pixel(p.0, p.1.map_color())));
    }
}
