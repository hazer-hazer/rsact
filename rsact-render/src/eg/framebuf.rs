use crate::{
    color::Color,
    geometry::{Point, Rect, Size},
    output::{MapColor, RenderTarget, pixel::Pixel},
};
use alloc::boxed::Box;
use embedded_graphics::{
    geometry::OriginDimensions,
    pixelcolor::{
        BinaryColor, Rgb555, Rgb565, Rgb666, Rgb888,
        raw::{RawData, RawU1},
    },
    prelude::DrawTarget,
};

// TODO: Maybe PackedColor and Framebuf in common are not specific to the eg.

pub trait PackedColor {
    type Storage: Clone + Send + Sync + 'static;

    /// Pixels-per-storage for a specific color (e.g. BinaryColor is one bit and
    /// 8 of it can be stored inside a single byte)
    fn pps() -> usize;

    fn into_storage(&self) -> Self::Storage;

    fn as_color(packed: &Self::Storage, offset: usize) -> Self;
    fn set_color(packed: &mut Self::Storage, offset: usize, color: Self);
}

/// Rgb colors are not packed
macro_rules! rgb_packed_color_impl {
    ($($ty: ty: $storage: ty),* $(,)?) => {$(
        impl PackedColor for $ty {
            type Storage = $storage;

            fn pps() -> usize {
                1
            }

            fn into_storage(&self) -> Self::Storage {
                embedded_graphics::pixelcolor::IntoStorage::into_storage(*self)
            }

            fn as_color(packed: &Self::Storage, offset: usize) -> Self {
                let _ = offset;

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

rgb_packed_color_impl!(Rgb555: u16, Rgb565: u16, Rgb666: u32, Rgb888: u32);

impl PackedColor for BinaryColor {
    type Storage = u8;

    // fn none() -> Self::Storage {
    //     0b00
    // }

    fn pps() -> usize {
        8
    }

    fn into_storage(&self) -> Self::Storage {
        embedded_graphics::pixelcolor::IntoStorage::into_storage(*self)
    }

    fn as_color(packed: &Self::Storage, offset: usize) -> Self {
        debug_assert!(offset < 8);

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
        debug_assert!(offset < 8);

        // let value = match color {
        //     Some(color) => match color {
        //         BinaryColor::Off => 0b01,
        //         BinaryColor::On => 0b11,
        //     },
        //     None => 0b00,
        // };

        // *packed |= value << (3 - offset) * 2;

        // Clear the target bit before setting it: a plain `|=` can only turn a
        // pixel On, never back Off, so redrawing On->Off would leave a stale
        // set bit (ghosting on partial redraw / reused framebuffers).
        let mask = 1u8 << (7 - offset);
        match color {
            BinaryColor::Off => *packed &= !mask,
            BinaryColor::On => *packed |= mask,
        }
    }
}

pub trait Framebuf<C: Color + PackedColor> {
    fn data(&self) -> &[C::Storage];
    fn data_mut(&mut self) -> &mut [C::Storage];
    // fn pack(&self, pack: usize) -> &C::Storage;
    // fn pack_mut(&mut self, pack: usize) -> &mut C::Storage;

    fn pixel(&self, point: Point) -> Option<C> {
        self.point_to_subpart(point)
            .map(|(pack, offset)| C::as_color(&self.data()[pack], offset))
    }

    fn viewport(&self) -> Rect;

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

    fn output<T>(&self, target: &mut T)
    where
        T: RenderTarget,
        C: MapColor<T::Color>,
    {
        // The whole-frame flush is just the region flush over the full viewport
        // — one code path (WS6.3).
        self.output_region(target, self.viewport());
    }

    /// WS6.3: stream only the pixels inside `region` (clamped to the viewport)
    /// to `target`, instead of the whole framebuffer. This is the per-pixel
    /// replacement the damage-driven flush needs — a one-label change flushes a
    /// handful of rows, not the full screen.
    ///
    /// TODO (6.3b): still pixel-at-a-time. `PackedFramebuf` implements only
    /// `draw_iter`, so this fans out to one `set_pixel` per point on the target.
    /// A `fill_contiguous`/row-run path (whole-byte writes for mono, `slice::fill`
    /// for RGB) is the render-side win, distinct from this flush-side scoping.
    fn output_region<T>(&self, target: &mut T, region: Rect)
    where
        T: RenderTarget,
        C: MapColor<T::Color>,
    {
        let region = region.intersection(&self.viewport());
        let pixels = region
            .points()
            .map(|point| {
                self.pixel(point)
                    .map(|color| Pixel(point, color.map_color()))
            })
            .filter_map(|pixel| pixel);
        target.draw(pixels);
    }

    fn point_to_subpart(&self, point: Point) -> Option<(usize, usize)> {
        let size = self.viewport().size;
        if point.x < 0
            || point.x >= size.width as i32
            || point.y < 0
            || point.y >= size.height as i32
        {
            None
        } else {
            let index =
                point.y as usize * size.width as usize + point.x as usize;

            Some((index / C::pps(), index % C::pps()))
        }
    }

    fn draw_buffer(&self, f: impl FnOnce(&[C::Storage])) {
        f(self.data())
    }
}

pub struct PackedFramebuf<C: Color + PackedColor> {
    size: Size,
    pixels: Box<[C::Storage]>,
}

impl<C: Color + PackedColor> OriginDimensions for PackedFramebuf<C> {
    fn size(&self) -> embedded_graphics::prelude::Size {
        self.size.into()
    }
}

impl<C: Color + PackedColor + embedded_graphics::prelude::PixelColor> DrawTarget
    for PackedFramebuf<C>
{
    type Color = C;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::prelude::Pixel<Self::Color>>,
    {
        pixels.into_iter().for_each(
            |embedded_graphics::prelude::Pixel(point, color)| {
                self.set_pixel(point.into(), color);
            },
        );

        Ok(())
    }
}

impl<C: Color + PackedColor> Framebuf<C> for PackedFramebuf<C> {
    fn data(&self) -> &[C::Storage] {
        self.pixels.as_ref()
    }

    fn data_mut(&mut self) -> &mut [C::Storage] {
        self.pixels.as_mut()
    }

    fn viewport(&self) -> Rect {
        Rect::new(Point::zero(), self.size)
    }
}

impl<C: Color + PackedColor> PackedFramebuf<C> {
    pub fn new(size: Size, initial_color: C) -> Self {
        // TODO: Not really, unused space is possible, just choose least
        // sufficient framebuf size
        assert!(
            size.area() as usize % C::pps() == 0,
            "PackedFramebuf area must be divisible by {} to store pixels packed",
            C::pps()
        );

        let pixels =
            vec![initial_color.into_storage(); size.area() as usize / C::pps()]
                .into_boxed_slice();
        Self { size, pixels }
    }
}

// TODO: When #[feature(generic_const_exprs)] is stabilized
// pub struct CPackedFramebuf<C: Color, const WIDTH: usize, const HEIGHT: usize>
// {     size: Size,
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
//         let pixels = vec![C::none(); size.area() as usize /
// C::stored_pixels()]             .into_boxed_slice();
//         Self { size, pixels }
//     }
// }

#[cfg(test)]
mod tests {
    use super::{Framebuf, PackedFramebuf};
    use crate::{
        geometry::{Point, Rect, Size},
        output::{RenderTarget, pixel::Pixel},
    };
    use alloc::vec::Vec;
    use embedded_graphics::{
        pixelcolor::{BinaryColor, Rgb888},
        prelude::RgbColor,
    };

    /// A [`RenderTarget`] that records the points it was asked to draw — lets a
    /// test assert *which* pixels a flush streamed (WS6.3 region flush).
    struct RecordTarget {
        points: Vec<Point>,
    }

    impl RenderTarget for RecordTarget {
        type Color = Rgb888;
        fn draw(&mut self, pixels: impl Iterator<Item = Pixel<Self::Color>>) {
            self.points.extend(pixels.map(|p| p.0));
        }
    }

    #[test]
    fn rgb_framebuf_indexing() {
        // This should work as a straightforward framebuffer without packing,
        // because Rgb888 stored in a single u32

        const WIDTH: u32 = 120;
        const HEIGHT: u32 = 180;

        let mut framebuf =
            PackedFramebuf::new(Size::new(WIDTH, HEIGHT), Rgb888::BLACK);

        for x in 0..WIDTH as i32 {
            for y in 0..HEIGHT as i32 {
                assert!(
                    framebuf.pixel(Point::new(x, y)).is_some(),
                    "Framebuf of size {WIDTH}x{HEIGHT} must contain pixel ({x},{y})"
                );
            }
        }

        for x in 0..WIDTH as i32 {
            for y in 0..HEIGHT as i32 {
                framebuf.set_pixel(Point::new(x, y), Rgb888::WHITE);
            }
        }
    }

    #[test]
    fn packed_framebuf_indexing() {
        const WIDTH: u32 = 120;
        const HEIGHT: u32 = 180;

        let mut framebuf =
            PackedFramebuf::new(Size::new(WIDTH, HEIGHT), BinaryColor::Off);

        for x in 0..WIDTH as i32 {
            for y in 0..HEIGHT as i32 {
                assert!(
                    framebuf.pixel(Point::new(x, y)).is_some(),
                    "Framebuf of size {WIDTH}x{HEIGHT} must contain pixel ({x},{y})"
                );
            }
        }

        for x in 0..WIDTH as i32 {
            for y in 0..HEIGHT as i32 {
                framebuf.set_pixel(Point::new(x, y), BinaryColor::On);
            }
        }
    }

    /// WS6.3: `output_region` streams exactly the region's pixels (in draw
    /// order), NOT the whole framebuffer — the flush-side scoping the damage
    /// pipeline needs.
    #[test]
    fn output_region_streams_only_the_region() {
        const W: u32 = 20;
        const H: u32 = 16;
        let framebuf = PackedFramebuf::new(Size::new(W, H), Rgb888::BLACK);

        let region = Rect::new(Point::new(5, 4), Size::new(6, 3));
        let mut target = RecordTarget { points: Vec::new() };
        framebuf.output_region(&mut target, region);

        // Exactly the region's points, in the same order.
        let expected: Vec<Point> = region.points().collect();
        assert_eq!(target.points, expected);
        assert_eq!(target.points.len(), (6 * 3) as usize);
        // Emphatically not the whole 20x16 framebuffer.
        assert!(target.points.len() < (W * H) as usize);
    }

    /// A region reaching past the framebuffer edge is clamped to the viewport —
    /// no out-of-bounds points are streamed (and, with indexed backends, none
    /// would index out of the buffer).
    #[test]
    fn output_region_clamps_to_viewport() {
        const W: u32 = 10;
        const H: u32 = 10;
        let framebuf = PackedFramebuf::new(Size::new(W, H), Rgb888::BLACK);

        // Overlaps the bottom-right corner and extends beyond → clamps to the
        // 2x2 square at (8,8).
        let region = Rect::new(Point::new(8, 8), Size::new(5, 5));
        let mut target = RecordTarget { points: Vec::new() };
        framebuf.output_region(&mut target, region);

        let expected: Vec<Point> = Rect::new(Point::new(8, 8), Size::new(2, 2))
            .points()
            .collect();
        assert_eq!(target.points, expected);
    }

    /// The whole-frame `output` is the region flush over the full viewport, so
    /// it streams every pixel — the region path did not change full-flush
    /// behaviour.
    #[test]
    fn output_covers_the_whole_framebuffer() {
        const W: u32 = 8;
        const H: u32 = 6;
        let framebuf = PackedFramebuf::new(Size::new(W, H), Rgb888::BLACK);

        let mut target = RecordTarget { points: Vec::new() };
        framebuf.output(&mut target);

        assert_eq!(target.points.len(), (W * H) as usize);
        let expected: Vec<Point> = framebuf.viewport().points().collect();
        assert_eq!(target.points, expected);
    }
}
