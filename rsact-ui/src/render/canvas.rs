use super::color::{Color, MapColor};
use crate::prelude::Size;
use alloc::boxed::Box;
use embedded_graphics::{
    Pixel,
    pixelcolor::{BinaryColor, Rgb555, Rgb565, Rgb666, Rgb888},
    prelude::{Dimensions, DrawTarget, Point, PointsIter},
    primitives::Rectangle,
};
use num::Integer;

pub trait PackedColor: Sized {
    type Storage: Clone;

    fn none() -> Self::Storage;
    fn stored_pixels() -> usize;
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
    ($($ty: ty),* $(,)?) => {$(
        impl PackedColor for $ty {
            type Storage = Option<Self>;

            fn none() -> Self::Storage {
                None
            }

            fn stored_pixels() -> usize {
                1
            }

            fn as_color(packed: &Self::Storage, offset: usize) -> Option<Self> {
                let _ = offset;
                *packed
            }

            fn set_color(
                packed: &mut Self::Storage,
                offset: usize,
                color: Option<Self>,
            ) {
                let _ = offset;
                *packed = color;
            }
        })*
    };
}

option_packed_color_impl!(Rgb555, Rgb565, Rgb666, Rgb888);

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

pub struct Canvas<C: Color> {
    size: Size,
    pixels: Box<[C::Storage]>,
}

impl<C: Color> DrawTarget for Canvas<C> {
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

impl<C: Color> Dimensions for Canvas<C> {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        Rectangle::new(Point::zero(), self.size.into())
    }
}

impl<C: Color> Canvas<C> {
    pub fn draw<D: DrawTarget<Color = C>>(
        &self,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let pixels = self.bounding_box().points().filter_map(|point| {
            self.pixel(point).map(|color| Pixel(point, color))
        });

        target.draw_iter(pixels)
    }
}

impl<C: Color> Canvas<C> {
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
}
