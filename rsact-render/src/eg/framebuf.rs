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
    /// 8 of it can be stored inside a single byte).
    ///
    /// WS6.4.0(iii): an associated **const** so it is usable from a `const fn`
    /// — [`units_for`] needs it inside a `const { assert!(..) }`, and a trait
    /// *method* cannot be called in a const context on stable. The method form
    /// below is kept, defaulted, so every existing `C::pps()` call site is
    /// untouched.
    const PPS: usize;

    /// Method form of [`PPS`](PackedColor::PPS). Do not override.
    fn pps() -> usize {
        Self::PPS
    }

    fn into_storage(&self) -> Self::Storage;

    fn as_color(packed: &Self::Storage, offset: usize) -> Self;
    fn set_color(packed: &mut Self::Storage, offset: usize, color: Self);

    /// WS6.3b: the storage word holding `pps` copies of `color` — a whole word
    /// entirely of that colour. Used by the fast `fill_solid` to `slice::fill`
    /// the run of storage words fully inside a rect (mono: `0x00`/`0xFF`; RGB
    /// where `pps == 1`: just the pixel word). Partial edge words still go
    /// through `set_color`, so this need only cover full words.
    fn solid_storage(color: Self) -> Self::Storage;
}

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

// ─────────────────────── WS6.4.0(iii): tile capacity, checked at compile time
//
// The chain, all static, with no `generic_const_exprs`:
//
//   buffer type ─────────▶ PixelBuf::UNITS ─────▶ Renderer::SURFACE_UNITS
//   region + PackedColor::PPS ─▶ units_for ──▶ assert_region_fits
//
// so a frame policy whose largest region cannot fit the renderer's surface is a
// **compile error**, not a runtime check. 6.4d wires the assert into
// `UI::start_frame`; the pieces land here because they are pure additions with
// no dependency on tiles existing yet.

/// Units of `C::Storage` needed to hold a `w × h` region, **including row
/// padding**.
///
/// Rows pad to a whole number of storage units, which is what makes sub-byte
/// packing correct: a 122-pixel 1-bpp row occupies 16 bytes, not 15.25.
/// Area-based arithmetic gets this wrong, and that is exactly why
/// `PackedFramebuf::new` asserts `area % pps == 0` today and panics on a real
/// 122×250 mono e-paper panel — roadmap 6.5 replaces that check with this.
///
/// ```
/// # use rsact_render::eg::framebuf::units_for;
/// # use embedded_graphics::pixelcolor::{BinaryColor, Rgb565};
/// // 16-bit colour: one storage unit per pixel, nothing to pad.
/// assert_eq!(units_for::<Rgb565>(240, 24), 5760);
/// // 1-bpp: each row rounds up to a whole byte — 122px -> 16 bytes.
/// assert_eq!(units_for::<BinaryColor>(122, 24), 384);
/// ```
pub const fn units_for<C: PackedColor>(w: u32, h: u32) -> usize {
    // `div_ceil` written out: keeps this a plain const fn on stable.
    let row_units = ((w as usize) + C::PPS - 1) / C::PPS;
    row_units * (h as usize)
}

/// A caller-owned buffer able to hold `UNITS` storage units of colour `C`.
///
/// Implemented for plain arrays so capacity is part of the *type* and can be
/// compared against a frame policy at compile time. rsact never holds one of
/// these — the user hands it to their concrete renderer through that backend's
/// own inherent API (roadmap 6.4.0, "surface ownership"). This trait exists only
/// so the size can be *checked*.
pub trait PixelBuf<C: PackedColor> {
    const UNITS: usize;
}

macro_rules! native_pixel_buf {
    ($($storage:ty),* $(,)?) => {$(
        impl<C: PackedColor<Storage = $storage>, const N: usize> PixelBuf<C>
            for [$storage; N]
        {
            const UNITS: usize = N;
        }
    )*};
}

// Keyed by storage type, so each impl targets a distinct `Self` and coherence
// holds without any negative reasoning.
native_pixel_buf!(u8, u16, u32);

/// A raw byte buffer viewed as storage for a wider colour — the DMA/wire-format
/// case (an RGB565 tile handed to SPI as bytes).
///
/// A newtype rather than a second `impl … for [u8; N]`, and that is **forced**:
/// `impl<C: PackedColor<Storage = u8>> PixelBuf<C> for [u8; N]` and
/// `impl<C: PackedColor<Storage = u16>> PixelBuf<C> for [u8; N]` are `E0119`
/// conflicting impls, because Rust does no negative reasoning over associated
/// types and cannot see that a colour's `Storage` is only ever one of them.
/// Wrapping also reads as documentation at the call site: `AsBytes` is precisely
/// what you hand to the transport.
///
/// ```
/// # use rsact_render::eg::framebuf::{AsBytes, PixelBuf, units_for};
/// # use embedded_graphics::pixelcolor::Rgb565;
/// // The same 240x24 RGB565 tile, expressed two ways — protocol-agnostic.
/// assert_eq!(<[u16; 5760] as PixelBuf<Rgb565>>::UNITS, 5760);
/// assert_eq!(<AsBytes<[u8; 11520]> as PixelBuf<Rgb565>>::UNITS, 5760);
/// assert_eq!(units_for::<Rgb565>(240, 24), 5760);
/// ```
pub struct AsBytes<B>(pub B);

impl<C: PackedColor, const N: usize> PixelBuf<C> for AsBytes<[u8; N]> {
    // One impl covers every storage width, so there is no conflict to resolve.
    // Integer division truncates, which is the safe direction: a buffer a byte
    // short of a whole unit reports the smaller capacity and gets rejected.
    const UNITS: usize = N / core::mem::size_of::<C::Storage>();
}

/// Compile-time proof that a `w × h` region fits in buffer `B`.
///
/// Call it from a `const` block; a violation is a post-monomorphization error
/// whose instantiation names the concrete colour, buffer and dimensions. 6.4d
/// calls this from `UI::start_frame` with the frame policy's largest region, so
/// a `Frame` whose regions could overflow the surface cannot be obtained.
///
/// ```
/// # use rsact_render::eg::framebuf::assert_region_fits;
/// # use embedded_graphics::pixelcolor::Rgb565;
/// // 240x24 RGB565 needs 5760 u16 — exactly what this buffer holds.
/// const _: () = assert_region_fits::<Rgb565, [u16; 5760]>(240, 24);
/// ```
///
/// One row too tall does not compile:
///
/// ```compile_fail
/// # use rsact_render::eg::framebuf::assert_region_fits;
/// # use embedded_graphics::pixelcolor::Rgb565;
/// // 240x25 needs 6000 units; the buffer holds 5760.
/// const _: () = assert_region_fits::<Rgb565, [u16; 5760]>(240, 25);
/// ```
///
/// Nor does a 1-bpp buffer sized by area instead of by padded rows:
///
/// ```compile_fail
/// # use rsact_render::eg::framebuf::assert_region_fits;
/// # use embedded_graphics::pixelcolor::BinaryColor;
/// // 122x24 needs ceil(122/8)*24 = 384 bytes, not 122*24/8 = 366.
/// const _: () = assert_region_fits::<BinaryColor, [u8; 366]>(122, 24);
/// ```
pub const fn assert_region_fits<C: PackedColor, B: PixelBuf<C>>(
    w: u32,
    h: u32,
) {
    assert!(
        units_for::<C>(w, h) <= B::UNITS,
        "region does not fit the pixel buffer — see the instantiation in this \
         error for the colour, buffer type and region size"
    );
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
    /// TODO: this FLUSH side is still pixel-at-a-time — it streams one `Pixel`
    /// per point to the target. The DRAW side (filling INTO the framebuffer) is
    /// now fast (`fill_solid`, WS6.3b); batching contiguous scanline RUNS to the
    /// display driver here (vs per-pixel) is the remaining flush-side win, and
    /// belongs with the strip/regions output work (6.3/6.4).
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

    /// Flat pixel index of `point`, in this buffer's own 0-based space.
    ///
    /// **WS6.4.0(i-2): the single source of truth for addressing.** Every path
    /// that turns a coordinate into a storage index must route through this (or
    /// [`row_stride`] to step between rows). `point` must be inside
    /// [`viewport`] — callers bounds-check first ([`point_to_subpart`]) or clip
    /// first ([`local_bounds`]).
    ///
    /// It used to be open-coded in two places: here and in
    /// `PackedFramebuf::fill_solid`'s row loop, whose comment even noted it was
    /// "same as `point_to_subpart`". That duplication is a trap for the tiled
    /// work: giving the buffer a non-zero origin and updating only one of them
    /// lands WS6.3b's fast solid fills in the wrong row while per-pixel writes
    /// stay correct — a *plausible* image rather than an obvious failure. Fold
    /// the origin in here and both paths follow.
    ///
    /// [`row_stride`]: Framebuf::row_stride
    /// [`viewport`]: Framebuf::viewport
    /// [`point_to_subpart`]: Framebuf::point_to_subpart
    /// [`local_bounds`]: Framebuf::local_bounds
    fn flat_index(&self, point: Point) -> usize {
        let viewport = self.viewport();
        let local = point - viewport.top_left;
        local.y as usize * viewport.size.width as usize + local.x as usize
    }

    /// Flat-index distance between vertically adjacent pixels — i.e. one row.
    /// A buffer's own width *is* its stride, so this derives from [`viewport`]
    /// like [`flat_index`] does.
    ///
    /// [`viewport`]: Framebuf::viewport
    /// [`flat_index`]: Framebuf::flat_index
    fn row_stride(&self) -> usize {
        self.viewport().size.width as usize
    }

    /// `area` clipped to this buffer. A zero-sized result means nothing to do.
    fn local_bounds(&self, area: Rect) -> Rect {
        area.intersection(&self.viewport())
    }

    fn point_to_subpart(&self, point: Point) -> Option<(usize, usize)> {
        if !self.viewport().contains(point) {
            return None;
        }
        let index = self.flat_index(point);
        Some((index / C::pps(), index % C::pps()))
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

    /// WS6.3b: fill a rectangle with a single colour without the per-pixel
    /// bit-twiddling `draw_iter` fans out to (the default `fill_solid` bounces
    /// through `fill_contiguous` → `draw_iter`). Every row's pixel range is split
    /// into a partial head word, a run of WHOLE storage words, and a partial tail
    /// word: the whole words are `slice::fill`ed (mono: whole-byte 0x00/0xFF
    /// writes — the 8–32× win; RGB: a `slice::fill` run), and only the two edge
    /// words go through `set_color`. Whole words are entirely inside the row, so
    /// filling them can't corrupt a neighbouring row that shares an edge byte
    /// (mono rows straddle bytes) — those shared bytes are always partial, hence
    /// bit-precise. This is the render-side counterpart to WS6.3a's flush-side
    /// region scoping; every clear/background/block fill uses it.
    fn fill_solid(
        &mut self,
        area: &embedded_graphics::primitives::Rectangle,
        color: Self::Color,
    ) -> Result<(), Self::Error> {
        // WS6.4.0(i-2): clipping and addressing both come from `Framebuf` now,
        // not from a second open-coded copy of `y*width + x` — see
        // `Framebuf::flat_index`. The row start is computed ONCE and stepped by
        // `row_stride`, so a non-zero buffer origin (the tiled work) lands here
        // for free.
        let area = Framebuf::local_bounds(self, (*area).into());
        if area.size.width == 0 || area.size.height == 0 {
            return Ok(());
        }

        let pps = C::pps();
        let solid = C::solid_storage(color);
        let stride = Framebuf::row_stride(self);
        let w = area.size.width as usize;
        let h = area.size.height as usize;
        let mut start = Framebuf::flat_index(self, area.top_left);

        for _ in 0..h {
            let end = start + w;
            // Round the pixel range INWARD to whole storage-word boundaries.
            let head_end = start.div_ceil(pps) * pps;
            let tail_start = (end / pps) * pps;

            if head_end >= tail_start {
                // The row spans fewer than one whole word — all per-pixel.
                for i in start..end {
                    C::set_color(&mut self.pixels[i / pps], i % pps, color);
                }
            } else {
                for i in start..head_end {
                    C::set_color(&mut self.pixels[i / pps], i % pps, color);
                }
                self.pixels[head_end / pps..tail_start / pps]
                    .fill(solid.clone());
                for i in tail_start..end {
                    C::set_color(&mut self.pixels[i / pps], i % pps, color);
                }
            }

            start += stride;
        }

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

    // A tiny LCG for the fill fuzz (no `rand` dep; deterministic per seed).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(1);
            self.0
        }
        fn range(&mut self, n: u32) -> u32 {
            (self.next() % n as u64) as u32
        }
    }

    /// WS6.3b: the fast `fill_solid` (whole-word `slice::fill` + bit-precise edge
    /// words) must produce a BYTE-IDENTICAL framebuffer to the per-pixel
    /// `draw_iter` path — for RGB (`pps == 1`) and, critically, for packed mono
    /// (`pps == 8`, where partial edge bytes are shared with neighbouring rows).
    /// 300 random rects each (many with negative / oversized coords, so clipping
    /// and sub-word edges are exercised).
    #[test]
    fn fill_solid_matches_draw_iter_fuzz() {
        use embedded_graphics::{
            Pixel as EgPixel,
            prelude::{DrawTarget, Point as EgPoint, Size as EgSize},
            primitives::{PointsIter, Rectangle as EgRect},
        };

        // RGB: 20x16 (pps == 1, any area is valid).
        for seed in 0..300u64 {
            let mut rng =
                Rng(seed.wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(1));
            let size = Size::new(20, 16);
            let area = EgRect::new(
                EgPoint::new(
                    rng.range(24) as i32 - 2,
                    rng.range(20) as i32 - 2,
                ),
                EgSize::new(rng.range(24), rng.range(20)),
            );
            let color = Rgb888::new(
                rng.next() as u8,
                rng.next() as u8,
                rng.next() as u8,
            );

            let mut fast = PackedFramebuf::new(size, Rgb888::BLACK);
            let mut slow = PackedFramebuf::new(size, Rgb888::BLACK);
            fast.fill_solid(&area, color).unwrap();
            slow.draw_iter(area.points().map(|p| EgPixel(p, color)))
                .unwrap();
            assert_eq!(
                fast.data(),
                slow.data(),
                "rgb seed {seed}: fill_solid != draw_iter for {area:?}"
            );
        }

        // Mono: 24x16 (area 384 divisible by 8). `width % 8 != 0` in some rows so
        // rows straddle bytes — the partial-edge-byte correctness case.
        for seed in 0..300u64 {
            let mut rng =
                Rng(seed.wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(0xabc));
            let size = Size::new(24, 16);
            let area = EgRect::new(
                EgPoint::new(
                    rng.range(28) as i32 - 2,
                    rng.range(20) as i32 - 2,
                ),
                EgSize::new(rng.range(28), rng.range(20)),
            );
            let color = if rng.next() & 1 == 0 {
                BinaryColor::On
            } else {
                BinaryColor::Off
            };

            let mut fast = PackedFramebuf::new(size, BinaryColor::Off);
            let mut slow = PackedFramebuf::new(size, BinaryColor::Off);
            fast.fill_solid(&area, color).unwrap();
            slow.draw_iter(area.points().map(|p| EgPixel(p, color)))
                .unwrap();
            assert_eq!(
                fast.data(),
                slow.data(),
                "mono seed {seed}: fill_solid != draw_iter for {area:?}"
            );
        }
    }

    /// A buffer whose viewport has a **non-zero origin** — what a tile is.
    /// `PackedFramebuf::viewport()` is hard-wired to `Point::zero()`, so this is
    /// the only way to exercise the origin term today.
    struct OffsetBuf {
        origin: Point,
        size: Size,
        pixels: Vec<u8>,
    }

    impl super::Framebuf<BinaryColor> for OffsetBuf {
        fn data(&self) -> &[u8] {
            &self.pixels
        }
        fn data_mut(&mut self) -> &mut [u8] {
            &mut self.pixels
        }
        fn viewport(&self) -> Rect {
            Rect::new(self.origin, self.size)
        }
    }

    /// WS6.4.0(iii): row padding is the whole subtlety of `units_for`, so pin
    /// the boundaries rather than only the happy cases the doctests show.
    ///
    /// The rule is per-**row**, not per-area: a row that ends mid-storage-unit
    /// still consumes the whole unit, because the next row starts on a fresh
    /// one. Area arithmetic silently under-counts and is what makes
    /// `PackedFramebuf::new`'s `area % pps == 0` assert reject real panels.
    #[test]
    fn units_for_pads_each_row_not_the_area() {
        use super::{AsBytes, PixelBuf, units_for};
        use embedded_graphics::pixelcolor::Rgb565;

        // Exactly one storage unit wide: no padding.
        assert_eq!(units_for::<BinaryColor>(8, 1), 1);
        // One pixel over: a whole second byte, for one row.
        assert_eq!(units_for::<BinaryColor>(9, 1), 2);
        // One pixel wide, ten rows: ten bytes, 79 of the 80 bits wasted. Area
        // arithmetic would say ceil(10/8) = 2.
        assert_eq!(units_for::<BinaryColor>(1, 10), 10);
        // The e-paper case: 122 -> 16 bytes per row, not 15.25.
        assert_eq!(units_for::<BinaryColor>(122, 24), 16 * 24);
        // pps == 1 colours can never pad.
        assert_eq!(units_for::<Rgb888>(7, 3), 21);
        // Degenerate regions cost nothing.
        assert_eq!(units_for::<Rgb888>(0, 5), 0);
        assert_eq!(units_for::<BinaryColor>(5, 0), 0);

        // `AsBytes` truncates, which is the SAFE direction: a buffer one byte
        // short of a whole unit reports the smaller capacity and gets rejected
        // rather than over-promising.
        assert_eq!(<AsBytes<[u8; 11521]> as PixelBuf<Rgb565>>::UNITS, 5760);
        assert_eq!(<AsBytes<[u8; 11519]> as PixelBuf<Rgb565>>::UNITS, 5759);
    }

    /// WS6.4.0(i-2): addressing is origin-aware, in ONE place.
    ///
    /// `flat_index` / `point_to_subpart` take **absolute** coordinates and
    /// resolve them against `viewport()`, so a buffer that covers a sub-rect of
    /// the screen — a tile — indexes correctly without every caller translating
    /// by hand. This is what `fill_solid` now inherits instead of open-coding
    /// `y*width + x` against an assumed zero origin.
    #[test]
    fn addressing_is_origin_aware() {
        let origin = Point::new(40, 100);
        let size = Size::new(16, 8);
        let buf = OffsetBuf {
            origin,
            size,
            pixels: Vec::from([0u8; 16]), // 16x8 mono = 128 px = 16 bytes
        };

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
}
