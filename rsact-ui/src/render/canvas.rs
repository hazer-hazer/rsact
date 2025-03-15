use super::color::{Color, MapColor};
use crate::prelude::Size;
use alloc::boxed::Box;
use embedded_graphics::{
    Drawable, Pixel,
    pixelcolor::{BinaryColor, Rgb555, Rgb565, Rgb666, Rgb888, raw::RawData},
    prelude::{Dimensions, DrawTarget, IntoStorage, Point, PointsIter},
    primitives::Rectangle,
};
use num::Integer;

pub trait PackedColor: Sized {
    type Storage: Clone + Send + Sync + 'static;

    fn none() -> Self::Storage;
    fn stored_pixels() -> usize;
    // fn into_storage(&self) -> Self::Storage;
    // fn unpack(
    //     packed: &Self::Storage,
    // ) -> impl Iterator<Item = Option<Self::Color>>;
    fn as_color(packed: &Self::Storage, offset: usize) -> Option<Self>;
    fn set_color(
        packed: &mut Self::Storage,
        offset: usize,
        color: Option<Self>,
    );
}

macro_rules! option_packed_color_impl {
    ($($ty: ty: $storage: ty),* $(,)?) => {$(
        impl PackedColor for $ty {
            type Storage = $storage;

            fn none() -> Self::Storage {
                0x0
            }

            fn stored_pixels() -> usize {
                1
            }

            // fn into_storage(&self) -> Self::Storage {
            //     embedded_graphics_core::pixelcolor::IntoStorage::into_storage(*self)
            // }

            fn as_color(packed: &Self::Storage, offset: usize) -> Option<Self> {
                let _ = offset;
                if packed == &0x0 {
                    return None
                } else {
                    Some(<Self as embedded_graphics::pixelcolor::PixelColor>::Raw::from_u32(*packed as u32).into())
                }
            }

            fn set_color(
                packed: &mut Self::Storage,
                offset: usize,
                color: Option<Self>,
            ) {
                let _ = offset;
                *packed = if let Some(color) = color {
                    Into::<<Self as embedded_graphics::pixelcolor::PixelColor>::Raw>::into(color).into_inner()
                } else {
                    0x0
                };
            }
        })*
    };
}

option_packed_color_impl!(Rgb555: u16, Rgb565: u16, Rgb666: u32, Rgb888: u32);

impl PackedColor for BinaryColor {
    type Storage = u8;

    fn none() -> Self::Storage {
        0b00
    }

    fn stored_pixels() -> usize {
        2
    }

    fn as_color(packed: &Self::Storage, offset: usize) -> Option<Self> {
        assert!(offset < 4);
        let color = (*packed >> (3 - offset) * 2) & 0b11;

        match color {
            0b00 => None,
            0b01 => Some(BinaryColor::Off),
            0b11 => Some(BinaryColor::On),
            _ => panic!("Invalid packed BinaryColor contention: {}", packed),
        }
    }

    fn set_color(
        packed: &mut Self::Storage,
        offset: usize,
        color: Option<Self>,
    ) {
        let value = match color {
            Some(color) => match color {
                BinaryColor::Off => 0b01,
                BinaryColor::On => 0b11,
            },
            None => 0b00,
        };

        *packed |= value << (3 - offset) * 2;
    }
}

pub struct RawCanvas<C: Color> {
    size: Size,
    pixels: Box<[C::Storage]>,
}

impl<C: Color> Drawable for RawCanvas<C> {
    type Color = C;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        // TODO: Is this optimal?
        let pixels = self
            .bounding_box()
            .points()
            .map(|point| self.pixel(point).map(|color| Pixel(point, color)))
            .filter_map(|pixel| pixel);
        target.draw_iter(pixels)
    }
}

impl<C: Color> DrawTarget for RawCanvas<C> {
    type Color = C;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        pixels.into_iter().for_each(|Pixel(point, color)| {
            self.set_pixel(point, color);
        });

        Ok(())
    }
}

impl<C: Color> Dimensions for RawCanvas<C> {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        Rectangle::new(Point::zero(), self.size.into())
    }
}

impl<C: Color> RawCanvas<C> {
    pub fn new(size: Size) -> Self {
        let pixels = vec![C::none(); size.area() as usize / C::stored_pixels()]
            .into_boxed_slice();
        Self { size, pixels }
    }

    fn point_to_subpart(&self, point: Point) -> Option<(usize, usize)> {
        if point.x < 0
            || point.y >= self.size.width as i32
            || point.y < 0
            || point.y >= self.size.height as i32
        {
            None
        } else {
            let index =
                point.y as usize * self.size.width as usize + point.x as usize;
            let (pack, offset) = index.div_rem(&C::stored_pixels());

            Some((pack, offset))
        }
    }

    pub fn pixel(&self, point: Point) -> Option<C> {
        self.point_to_subpart(point)
            .and_then(|(pack, offset)| C::as_color(&self.pixels[pack], offset))
    }

    pub fn reset_pixel(&mut self, point: Point) {
        self.point_to_subpart(point).map(|(pack, offset)| {
            C::set_color(&mut self.pixels[pack], offset, None);
        });
    }

    pub fn set_pixel(&mut self, point: Point, color: C) {
        self.point_to_subpart(point).map(|(pack, offset)| {
            C::set_color(&mut self.pixels[pack], offset, Some(color));
        });
    }

    pub fn draw<D: DrawTarget<Color = C>>(
        &self,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let pixels = self.bounding_box().points().filter_map(|point| {
            self.pixel(point).map(|color| Pixel(point, color))
        });

        target.draw_iter(pixels)
    }

    pub async fn draw_buffer(&self, f: impl AsyncFn(&[C::Storage])) {
        f(self.pixels.as_ref()).await
    }
}
