use super::color::{Color, MapColor};
use crate::prelude::Size;
use alloc::boxed::Box;
use core::convert::Infallible;
use embedded_graphics::{
    Drawable, Pixel,
    pixelcolor::{
        BinaryColor, Rgb555, Rgb565, Rgb666, Rgb888,
        raw::{RawData, RawU1},
    },
    prelude::{Dimensions, DrawTarget, IntoStorage, Point, PointsIter},
    primitives::Rectangle,
};
use num::Integer;

pub trait PackedColor: Sized {
    type Storage: Clone + Send + Sync + 'static;

    fn none() -> Self::Storage;
    fn pps() -> usize;
    // fn into_storage(&self) -> Self::Storage;
    // fn unpack(
    //     packed: &Self::Storage,
    // ) -> impl Iterator<Item = Option<Self::Color>>;
    // fn as_color(packed: &Self::Storage, offset: usize) -> Option<Self>;
    // fn set_color(
    //     packed: &mut Self::Storage,
    //     offset: usize,
    //     color: Option<Self>,
    // );

    fn as_color(packed: &Self::Storage, offset: usize) -> Self;
    fn set_color(packed: &mut Self::Storage, offset: usize, color: Self);
}

macro_rules! option_packed_color_impl {
    ($($ty: ty: $storage: ty),* $(,)?) => {$(
        impl PackedColor for $ty {
            type Storage = $storage;

            fn none() -> Self::Storage {
                0x0
            }

            fn pps() -> usize {
                1
            }

            // fn into_storage(&self) -> Self::Storage {
            //     embedded_graphics_core::pixelcolor::IntoStorage::into_storage(*self)
            // }

            fn as_color(packed: &Self::Storage, offset: usize) -> Self {
                let _ = offset;
                // if packed == &0x0 {
                //     return None
                // } else {
                //     Some(<Self as embedded_graphics::pixelcolor::PixelColor>::Raw::from_u32(*packed as u32).into())
                // }

                <Self as embedded_graphics::pixelcolor::PixelColor>::Raw::from_u32(*packed as u32).into()
            }

            fn set_color(
                packed: &mut Self::Storage,
                offset: usize,
                color: Self,
            ) {
                let _ = offset;
                *packed =
                    Into::<<Self as embedded_graphics::pixelcolor::PixelColor>::Raw>::into(color).into_inner();
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

    fn pps() -> usize {
        8
    }

    fn as_color(packed: &Self::Storage, offset: usize) -> Self {
        assert!(offset < 8);

        // let color = (*packed >> (3 - offset) * 2) & 0b11;

        // match color {
        //     0b00 => None,
        //     0b01 => Some(BinaryColor::Off),
        //     0b11 => Some(BinaryColor::On),
        //     _ => panic!("Invalid packed BinaryColor contention: {}", packed),
        // }

        let color = (*packed >> (7 - offset)) & 0b1;

        RawU1::from(color).into()
    }

    fn set_color(packed: &mut Self::Storage, offset: usize, color: Self) {
        assert!(offset < 8);

        // let value = match color {
        //     Some(color) => match color {
        //         BinaryColor::Off => 0b01,
        //         BinaryColor::On => 0b11,
        //     },
        //     None => 0b00,
        // };

        // *packed |= value << (3 - offset) * 2;

        *packed |= match color {
            BinaryColor::Off => 0b0,
            BinaryColor::On => 0b1,
        } << (7 - offset);
    }
}

pub trait Framebuf<C: Color>: Dimensions + DrawTarget {
    fn data(&self) -> &[C::Storage];
    fn data_mut(&mut self) -> &mut [C::Storage];
    // fn pack(&self, pack: usize) -> &C::Storage;
    // fn pack_mut(&mut self, pack: usize) -> &mut C::Storage;

    fn pixel(&self, point: Point) -> Option<C> {
        self.point_to_subpart(point)
            .map(|(pack, offset)| C::as_color(&self.data()[pack], offset))
    }

    // fn reset_pixel(&mut self, point: Point) {
    //     self.point_to_subpart(point).map(|(pack, offset)| {
    //         C::set_color(&mut self.data_mut()[pack], offset, None);
    //     });
    // }

    fn set_pixel(&mut self, point: Point, color: C) {
        self.point_to_subpart(point).map(|(pack, offset)| {
            C::set_color(&mut self.data_mut()[pack], offset, color);
        });
    }

    fn draw<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
    {
        // TODO: Is this optimal?
        let pixels = self
            .bounding_box()
            .points()
            .map(|point| self.pixel(point).map(|color| Pixel(point, color)))
            .filter_map(|pixel| pixel);
        target.draw_iter(pixels)
    }

    fn point_to_subpart(&self, point: Point) -> Option<(usize, usize)> {
        let size = self.bounding_box().size;
        if point.x < 0
            || point.y >= size.width as i32
            || point.y < 0
            || point.y >= size.height as i32
        {
            None
        } else {
            let index =
                point.y as usize * size.width as usize + point.x as usize;
            let (pack, offset) = index.div_rem(&C::pps());

            Some((pack, offset))
        }
    }

    fn draw_buffer(&self, f: impl FnOnce(&[C::Storage])) {
        f(self.data())
    }
}

pub struct PackedFramebuf<C: Color> {
    size: Size,
    pixels: Box<[C::Storage]>,
}

impl<C: Color> DrawTarget for PackedFramebuf<C> {
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

impl<C: Color> Framebuf<C> for PackedFramebuf<C> {
    fn data(&self) -> &[C::Storage] {
        self.pixels.as_ref()
    }

    fn data_mut(&mut self) -> &mut [C::Storage] {
        self.pixels.as_mut()
    }
}

impl<C: Color> Dimensions for PackedFramebuf<C> {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        Rectangle::new(Point::zero(), self.size.into())
    }
}

impl<C: Color> PackedFramebuf<C> {
    pub fn new(size: Size) -> Self {
        assert!(
            size.area() as usize % C::pps() == 0,
            "PackedFramebuf area must be divisible by {} to store pixels packed",
            C::pps()
        );

        let pixels =
            vec![C::none(); size.area() as usize / C::pps()].into_boxed_slice();
        Self { size, pixels }
    }
}

// TODO: When #[feature(generic_const_exprs)] is stabilized
// pub struct CPackedFramebuf<C: Color, const WIDTH: usize, const HEIGHT: usize> {
//     size: Size,
//     pixels: [[C::Storage; WIDTH]; HEIGHT],
// }

// impl<C: Color, const WIDTH: usize, const HEIGHT: usize> DrawTarget
//     for CPackedFramebuf<C, WIDTH, HEIGHT>
// {
//     type Color = C;
//     type Error = ();

//     fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
//     where
//         I: IntoIterator<Item = Pixel<Self::Color>>,
//     {
//         pixels.into_iter().for_each(|Pixel(point, color)| {
//             self.set_pixel(point, color);
//         });

//         Ok(())
//     }
// }

// impl<C: Color, const WIDTH: usize, const HEIGHT: usize> Framebuf<C>
//     for CPackedFramebuf<C, WIDTH, HEIGHT>
// {
//     // Theses reinterpretations are done because it is still not possible to give pixels `WIDTH * HEIGHT` size (issue https://github.com/rust-lang/rust/issues/76560)
//     // TODO: When #[feature(generic_const_exprs)] is stabilized

//     fn data(&self) -> &[C::Storage] {
//         unsafe {
//             core::slice::from_raw_parts(
//                 core::mem::transmute(self.pixels.as_ptr()),
//                 WIDTH * HEIGHT,
//             )
//         }
//     }

//     fn data_mut(&mut self) -> &mut [C::Storage] {
//         unsafe {
//             core::slice::from_raw_parts_mut(
//                 core::mem::transmute(self.pixels.as_ptr()),
//                 WIDTH * HEIGHT,
//             )
//         }
//     }
// }

// impl<C: Color, const WIDTH: usize, const HEIGHT: usize> Dimensions
//     for CPackedFramebuf<C, WIDTH, HEIGHT>
// {
//     fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
//         Rectangle::new(Point::zero(), self.size.into())
//     }
// }

// impl<C: Color, const WIDTH: usize, const HEIGHT: usize>
//     CPackedFramebuf<C, WIDTH, HEIGHT>
// {
//     pub fn new(size: Size) -> Self {
//         let pixels = []
//         let pixels = vec![C::none(); size.area() as usize / C::stored_pixels()]
//             .into_boxed_slice();
//         Self { size, pixels }
//     }
// }
