use embedded_graphics::{
    pixelcolor::raw::ByteOrder,
    prelude::{PixelColor, Point},
    Drawable, Pixel,
};
use std::marker::PhantomData;

mod rendered;

#[derive(Clone, Copy)]
pub struct IconRaw<BO: ByteOrder> {
    data: &'static [u8],
    size: u32,
    bo: PhantomData<BO>,
}

impl<BO: ByteOrder> IconRaw<BO> {
    pub const fn new(data: &'static [u8], size: u32) -> Self {
        Self { data, size, bo: PhantomData }
    }

    fn bit(&self, x: u32, y: u32) -> bool {
        let index = x + y * self.size;
        (self.data[index as usize / 8] & (1 << index % 8)) != 0
    }
}

pub struct Icon<C: PixelColor, BO: ByteOrder> {
    raw: IconRaw<BO>,
    position: Point,
    background: Option<C>,
    foreground: Option<C>,
}

impl<C: PixelColor, BO: ByteOrder> Icon<C, BO> {
    pub fn new(
        raw: IconRaw<BO>,
        position: Point,
        background: Option<C>,
        foreground: Option<C>,
    ) -> Self {
        Self { raw, position, background, foreground }
    }

    fn color(&self, bit: bool) -> Option<C> {
        if bit {
            self.foreground
        } else {
            self.background
        }
    }
}

impl<C: PixelColor, BO: ByteOrder> Drawable for Icon<C, BO> {
    type Color = C;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        for y in 0..self.raw.size {
            for x in 0..self.raw.size {
                if let Some(color) = self.color(self.raw.bit(x, y)) {
                    Pixel(
                        Point::new(x as i32, y as i32) + self.position,
                        color,
                    )
                    .draw(target)?;
                }
            }
        }

        Ok(())
    }
}
