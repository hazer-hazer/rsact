//! What a [`Framebuf`] needs embedded-graphics for.
//!
//! **WS6.4e: the framebuffer itself moved to [`crate::framebuf`]** — nothing
//! about packed storage, capacity or addressing was ever specific to this
//! crate, and living here meant the render layer split's `FramebufBlitter`
//! could not use it without depending on embedded-graphics.
//!
//! Two things genuinely do need it, and they are what stayed:
//!
//! - the [`PackedColor`] impls for embedded-graphics' color types, which is
//!   where the `IntoStorage`/`RawData` conversions live;
//! - the `Dimensions` + `DrawTarget` impls that let a [`Framebuf`] *be* an
//!   embedded-graphics draw target.

use crate::{
    color::Color,
    framebuf::{Framebuf, FramebufStorage, PackedColor},
    geometry::Rect,
};
use embedded_graphics::{
    geometry::Dimensions,
    pixelcolor::{
        BinaryColor, Rgb555, Rgb565, Rgb666, Rgb888,
        raw::{RawData, RawU1},
    },
    prelude::DrawTarget,
};

/// Rgb colors are not packed
macro_rules! rgb_packed_color_impl {
    ($($ty: ty: $storage: ty),* $(,)?) => {$(
        impl PackedColor for $ty {
            type Storage = $storage;

            const PPS: usize = 1;

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

            // pps == 1: a full word IS one pixel.
            fn solid_storage(color: Self) -> Self::Storage {
                color.into_storage()
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

    const PPS: usize = 8;

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

    // 8 pixels/byte, 1 bit each: a full word is all-on or all-off.
    fn solid_storage(color: Self) -> u8 {
        match color {
            BinaryColor::On => 0xff,
            BinaryColor::Off => 0x00,
        }
    }
}

// `Dimensions`, not `OriginDimensions`: embedded-graphics' `clipped`/`cropped`
// intersect against this box, and rsact hands them ABSOLUTE rects. Reporting
// origin-zero was correct only while the buffer always was the whole frame; a
// tile at (0, 24) would have had its every write clipped away.
impl<C: Color + PackedColor, B: FramebufStorage<C>> Dimensions
    for Framebuf<C, B>
{
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        self.viewport().into()
    }
}

impl<
    C: Color + PackedColor + embedded_graphics::prelude::PixelColor,
    B: FramebufStorage<C>,
> DrawTarget for Framebuf<C, B>
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

    /// WS6.3b's fast solid fill, reached from embedded-graphics.
    ///
    /// **WS6.4e: a delegation, not the implementation.** The algorithm is
    /// inherent on [`Framebuf::fill_solid`] so that a caller who is not an
    /// embedded-graphics draw target — the layer split's `FramebufBlitter` —
    /// gets the whole-word writes too, instead of re-forking the addressing to
    /// find them. This override still has to exist: without it eg's default
    /// `fill_solid` bounces through `fill_contiguous` → `draw_iter` and the fast
    /// path is never reached from `Rectangle::draw_styled`.
    fn fill_solid(
        &mut self,
        area: &embedded_graphics::primitives::Rectangle,
        color: Self::Color,
    ) -> Result<(), Self::Error> {
        Framebuf::fill_solid(self, Rect::from(*area), color);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::framebuf::{Framebuf, PackedColor};

    /// Host-side test surfaces. Local to the tests on purpose — see the note in
    /// `eg/renderer.rs`'s test module: the library exports no allocating helper
    /// because it must not choose where a framebuffer lives.
    /// A `&'static mut` loan, which is what both `FramebufStorage` impls are
    /// for.
    ///
    /// `Vec::leak` rather than a local `&mut buf[..]`: a leak is honest here and
    /// a borrow is not, because `WidgetCtx: 'static` means any renderer reachable
    /// through the UI must hold a `'static` loan anyway — a test that borrowed a
    /// local would be exercising a shape production cannot use. On a device this
    /// is a `StaticCell`, which is the same `'static` loan without the leak.
    fn heap_surface<C: crate::color::Color + PackedColor>(
        size: Size,
    ) -> &'static mut [<C as PackedColor>::Storage] {
        alloc::vec![
            C::default_background().into_storage();
            crate::framebuf::units_for::<C>(size.width, size.height)
        ]
        .leak()
    }
    use crate::geometry::{Point, Rect, Size};
    use alloc::vec::Vec;
    use embedded_graphics::{
        pixelcolor::{BinaryColor, Rgb888},
        prelude::RgbColor,
    };

    #[test]
    fn rgb_framebuf_indexing() {
        // This should work as a straightforward framebuffer without packing,
        // because Rgb888 stored in a single u32

        const WIDTH: u32 = 120;
        const HEIGHT: u32 = 180;

        let mut framebuf = Framebuf::new(
            Size::new(WIDTH, HEIGHT),
            heap_surface::<Rgb888>(Size::new(WIDTH, HEIGHT)),
        )
        .unwrap();

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

        let mut framebuf = Framebuf::new(
            Size::new(WIDTH, HEIGHT),
            heap_surface::<BinaryColor>(Size::new(WIDTH, HEIGHT)),
        )
        .unwrap();

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

    /// WS6.4d: what survived the flush seam is the **reading** contract, and
    /// this is it — a buffer answers for the coordinates it covers and refuses
    /// the rest.
    ///
    /// Three tests lived here (`output_region_streams_only_the_region`,
    /// `output_region_clamps_to_viewport`, `output_covers_the_whole_framebuffer`)
    /// and went with `output`/`output_region`: they asserted which pixels a
    /// *flush* streamed, and flushing is no longer rsact's. The clamping they
    /// pinned is not lost, because it was never a property of the loop — it is a
    /// property of `pixel`, which is what a caller walking a detached buffer
    /// actually calls.
    #[test]
    fn a_buffer_answers_only_for_the_region_it_covers() {
        const W: u32 = 10;
        const H: u32 = 10;
        let framebuf = Framebuf::<Rgb888, _>::new(
            Size::new(W, H),
            heap_surface::<Rgb888>(Size::new(W, H)),
        )
        .unwrap();

        // Inside: answered.
        assert!(framebuf.pixel(Point::new(0, 0)).is_some());
        assert!(framebuf.pixel(Point::new(9, 9)).is_some());

        // Outside, on every side: refused rather than wrapped to some other
        // row. An indexed read that wrapped would produce a plausible image
        // instead of an obvious failure.
        for outside in [
            Point::new(10, 0),
            Point::new(0, 10),
            Point::new(-1, 0),
            Point::new(0, -1),
            Point::new(100, 100),
        ] {
            assert!(
                framebuf.pixel(outside).is_none(),
                "{outside:?} is outside a {W}x{H} buffer and must not resolve"
            );
        }

        // The same contract on a buffer with a non-zero origin — a tile. Only
        // the covered rect answers, and it answers in ABSOLUTE coordinates.
        let mut tile = Framebuf::<Rgb888, _>::tile(heap_surface::<Rgb888>(
            Size::new(4, 4),
        ));
        tile.retarget(Rect::new(Point::new(6, 6), Size::new(4, 4)));
        assert!(tile.pixel(Point::new(6, 6)).is_some());
        assert!(tile.pixel(Point::new(9, 9)).is_some());
        assert!(
            tile.pixel(Point::new(0, 0)).is_none(),
            "a tile at (6,6) does not cover the frame origin"
        );
        assert!(tile.pixel(Point::new(10, 6)).is_none());
    }

    /// WS6.4.0(i-2): addressing is origin-aware, in ONE place.
    ///
    /// `flat_index` / `point_to_subpart` take **absolute** coordinates and
    /// resolve them against `viewport()`, so a buffer that covers a sub-rect of
    /// the screen — a tile — indexes correctly without every caller translating
    /// by hand. This is what `fill_solid` now inherits instead of open-coding
    /// `y*width + x` against an assumed zero origin.
    /// This used to need a bespoke `OffsetBuf` implementing the old `Framebuf`
    /// trait, because the buffer's viewport was pinned at the origin and a
    /// second implementor was the only way to give the origin term a non-zero
    /// value. `retarget` (WS6.4d) made a real tile expressible, so the test now
    /// runs against the type that ships — and the trait it needed is gone.
    #[test]
    fn addressing_is_origin_aware() {
        let origin = Point::new(40, 100);
        let size = Size::new(16, 8);
        // 16x8 mono = 128 px = 16 bytes.
        let mut buf =
            Framebuf::<BinaryColor, _>::tile(alloc::vec![0u8; 16].leak());
        buf.retarget(Rect::new(origin, size));

        // The origin itself is local (0, 0).
        assert_eq!(buf.flat_index(origin), 0);
        assert_eq!(buf.point_to_subpart(origin), Some((0, 0)));

        // Stride is the buffer's own width, so one row down is +width.
        assert_eq!(buf.row_stride(), 16);
        assert_eq!(buf.flat_index(origin + Point::new(0, 1)), 16);
        assert_eq!(buf.flat_index(origin + Point::new(3, 2)), 2 * 16 + 3);

        // Anything outside is rejected — including points that WOULD be valid
        // under the old zero-origin assumption (this is the regression).
        assert_eq!(buf.point_to_subpart(Point::new(0, 0)), None);
        assert_eq!(buf.point_to_subpart(origin + Point::new(-1, 0)), None);
        assert_eq!(buf.point_to_subpart(origin + Point::new(0, -1)), None);
        assert_eq!(buf.point_to_subpart(origin + Point::new(16, 0)), None);
        assert_eq!(buf.point_to_subpart(origin + Point::new(0, 8)), None);

        // `local_bounds` clips to the buffer, in absolute coordinates.
        let clipped =
            buf.local_bounds(Rect::new(Point::new(32, 96), Size::new(16, 8)));
        assert_eq!(clipped, Rect::new(origin, Size::new(8, 4)));
    }

    /// WS6.4e: the inherent [`Framebuf::fill_solid`] and the `DrawTarget`
    /// override must be the same fill, because the second is now a delegation
    /// to the first — and because the layer split's blitter will call the
    /// inherent one on the strength of that.
    #[test]
    fn the_draw_target_fill_delegates_to_the_inherent_one() {
        use embedded_graphics::prelude::DrawTarget;

        let size = Size::new(20, 16);
        // Deliberately crosses storage-word boundaries and hangs off the right
        // edge, so head/whole/tail and the clip are all exercised.
        let area = Rect::new(Point::new(3, 2), Size::new(30, 7));
        let ink = BinaryColor::On;

        let mut inherent = Framebuf::<BinaryColor, _>::new(
            size,
            heap_surface::<BinaryColor>(size),
        )
        .unwrap();
        Framebuf::fill_solid(&mut inherent, area, ink);

        let mut through_eg = Framebuf::<BinaryColor, _>::new(
            size,
            heap_surface::<BinaryColor>(size),
        )
        .unwrap();
        DrawTarget::fill_solid(&mut through_eg, &area.into(), ink).unwrap();

        let inherent: Vec<_> = inherent.data().to_vec();
        assert_eq!(inherent, through_eg.data());
        assert!(
            inherent.iter().any(|u| *u != 0),
            "the fill must have written something"
        );
    }
}
