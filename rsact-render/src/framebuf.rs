//! Packed pixel storage, independent of any backend.
//!
//! [`PackedColor`] says how a color packs into a storage word,
//! [`FramebufStorage`] is a caller-owned buffer, and [`Framebuf`] addresses one
//! through the other. The embedded-graphics impls are in `eg/framebuf.rs`.
//!
//! **No `embedded_graphics` import may appear here, not even in a doctest** — a
//! doctest compiles as its own crate, so naming an optional dependency breaks
//! `cargo test --features std`.

use crate::{
    color::Color,
    geometry::{Point, Rect},
};

pub trait PackedColor {
    type Storage: Clone + Send + Sync + 'static;

    /// Pixels per storage unit: 8 for a 1-bit color in a `u8`, 1 for a color
    /// with a word of its own. A const so the capacity proof can use it inside
    /// `const { assert!(..) }`.
    const PPS: usize;

    /// Method form of [`PPS`](PackedColor::PPS). Do not override.
    fn pps() -> usize {
        Self::PPS
    }

    fn into_storage(&self) -> Self::Storage;

    fn as_color(packed: &Self::Storage, offset: usize) -> Self;
    fn set_color(packed: &mut Self::Storage, offset: usize, color: Self);

    /// A storage word holding `pps` copies of `color` — `0x00`/`0xFF` for mono,
    /// the pixel itself where a word holds one. [`Framebuf::fill_solid`]
    /// `slice::fill`s whole words with it; edge words go through `set_color`.
    fn solid_storage(color: Self) -> Self::Storage;
}

// A buffer too small for the frame policy it is lent to is rejected by
// `RasterRenderer::attach` — at compile time for a fixed-size array, as an
// `Err` for a runtime-length slice. See `FramebufStorage::UNITS`.

/// Units of `C::Storage` needed to hold a `w × h` region, **including row
/// padding**.
///
/// Rows pad to whole units, which is what makes sub-byte packing correct: a
/// 122-pixel 1-bpp row occupies 16 bytes, not 15.25.
///
/// ```text
/// units_for::<Rgb565>(240, 24)      == 5760   // one unit per pixel
/// units_for::<BinaryColor>(122, 24) == 384    // 1-bpp: 16 bytes per row
/// ```
///
/// (`text` and not a doctest: this module must not name embedded-graphics.)
pub const fn units_for<C: PackedColor>(w: u32, h: u32) -> usize {
    crate::renderer::region_units(w, h, C::PPS)
}

/// A caller-owned buffer a renderer can draw into: capacity plus access.
///
/// Lent by moving in and taken back by moving out, rather than borrowed — a
/// renderer cannot carry a lifetime parameter, and DMA needs ownership anyway.
pub trait FramebufStorage<C: PackedColor> {
    /// Capacity when the **type** knows it — `Some(N)` for a fixed-size array,
    /// making a policy violation a compile error — and `None` for a slice,
    /// checked at `attach` instead.
    ///
    /// **`None` means "ask the value", never "unbounded".**
    const UNITS: Option<usize>;

    fn units(&self) -> &[C::Storage];
    fn units_mut(&mut self) -> &mut [C::Storage];

    /// This buffer's real capacity. Always available, unlike
    /// [`UNITS`](Self::UNITS).
    fn unit_count(&self) -> usize {
        self.units().len()
    }
}

macro_rules! native_framebuf_storage {
    ($($storage:ty),* $(,)?) => {$(
        // `&mut [T; N]` rather than `[T; N]`: the extent stays in the type but
        // the loan is a pointer. `attach`/`detach` move `B` by value, so an
        // owned array would memcpy the framebuffer twice per region.
        impl<C: PackedColor<Storage = $storage>, const N: usize> FramebufStorage<C>
            for &mut [$storage; N]
        {
            const UNITS: Option<usize> = Some(N);

            fn units(&self) -> &[$storage] { &self[..] }
            fn units_mut(&mut self) -> &mut [$storage] { &mut self[..] }
        }

        // Runtime-sized: checked at `attach` instead of at compile time.
        impl<C: PackedColor<Storage = $storage>> FramebufStorage<C>
            for &mut [$storage]
        {
            const UNITS: Option<usize> = None;

            fn units(&self) -> &[$storage] { self }
            fn units_mut(&mut self) -> &mut [$storage] { self }
        }

        // NOTE: no impl for `Box<[T]>` — reach the one above via
        // `&mut boxed[..]`. Owned storage would be moved per hand-off.
    )*};
}

// Keyed by storage type, so each impl targets a distinct `Self`.
native_framebuf_storage!(u8, u16, u32);

// TODO (transport): a byte-buffer view, so an RGB565 tile can go to SPI as
// bytes. Needs a newtype — the two `FramebufStorage` impls for `[u8; N]` are
// E0119 conflicting — and that newtype must guarantee alignment before it can
// hand out `&mut [u16]` over a byte array.

/// A packed pixel buffer addressing an arbitrary rect of the screen.
///
/// Its `viewport` is that rect, in **absolute** coordinates, and its width is
/// the stride — so one allocation of `N` units serves any region needing at most
/// `N`. Every method takes absolute coordinates and resolves them against it.
pub struct Framebuf<C: Color + PackedColor, B: FramebufStorage<C>> {
    viewport: Rect,
    pixels: B,
    color: core::marker::PhantomData<C>,
}

impl<C: Color + PackedColor, B: FramebufStorage<C>> Framebuf<C, B> {
    pub fn data(&self) -> &[C::Storage] {
        self.pixels.units()
    }

    pub fn data_mut(&mut self) -> &mut [C::Storage] {
        self.pixels.units_mut()
    }

    /// The absolute rect this buffer currently covers.
    pub fn viewport(&self) -> Rect {
        self.viewport
    }

    // fn pack(&self, pack: usize) -> &C::Storage;
    // fn pack_mut(&mut self, pack: usize) -> &mut C::Storage;

    pub fn pixel(&self, point: Point) -> Option<C> {
        self.point_to_subpart(point)
            .map(|(pack, offset)| C::as_color(&self.data()[pack], offset))
    }

    // fn reset_pixel(&mut self, point: Point) {
    //     self.point_to_subpart(point).map(|(pack, offset)| {
    //         C::set_color(&mut self.data_mut()[pack], offset, None);
    //     });
    // }

    pub fn set_pixel(&mut self, point: Point, color: C) {
        self.point_to_subpart(point).map(|(pack, offset)| {
            C::set_color(&mut self.data_mut()[pack], offset, color);
        });
    }

    /// Fill a rectangle with one color, without the per-pixel bit-twiddling a
    /// pixel-at-a-time fan-out costs.
    ///
    /// Each row splits into a partial head word, a run of **whole** storage
    /// words, and a partial tail word. The whole words are `slice::fill`ed; only
    /// the two edge words go through `set_color`. Whole words lie entirely
    /// inside their row, so filling them cannot corrupt a neighbour that shares
    /// an edge byte — shared bytes are always partial, hence bit-precise.
    ///
    /// Inherent rather than a `DrawTarget::fill_solid` override, so a blitter
    /// gets the fast path without depending on embedded-graphics; that override
    /// delegates here.
    ///
    /// `area` is **absolute** and clipped to this buffer, so a rect that misses
    /// it entirely is a no-op.
    pub fn fill_solid(&mut self, area: Rect, color: C) {
        // Row start computed once and stepped by `row_stride`, so a non-zero
        // buffer origin costs nothing here.
        let area = self.local_bounds(area);
        if area.size.width == 0 || area.size.height == 0 {
            return;
        }

        let pps = C::pps();
        let solid = C::solid_storage(color);
        let stride = self.row_stride();
        let w = area.size.width as usize;
        let h = area.size.height as usize;
        let mut start = self.flat_index(area.top_left);

        for _ in 0..h {
            let end = start + w;
            // Round the pixel range INWARD to whole storage-word boundaries.
            let head_end = start.div_ceil(pps) * pps;
            let tail_start = (end / pps) * pps;

            if head_end >= tail_start {
                // The row spans fewer than one whole word — all per-pixel.
                for i in start..end {
                    C::set_color(
                        &mut self.pixels.units_mut()[i / pps],
                        i % pps,
                        color,
                    );
                }
            } else {
                for i in start..head_end {
                    C::set_color(
                        &mut self.pixels.units_mut()[i / pps],
                        i % pps,
                        color,
                    );
                }
                self.pixels.units_mut()[head_end / pps..tail_start / pps]
                    .fill(solid.clone());
                for i in tail_start..end {
                    C::set_color(
                        &mut self.pixels.units_mut()[i / pps],
                        i % pps,
                        color,
                    );
                }
            }

            start += stride;
        }
    }

    // To flush a detached buffer, walk rows at `row_stride()` — NOT at
    // `viewport().size.width`, which is short of it whenever the width is not a
    // whole number of storage units — and convert with `PackedColor::as_color`.

    /// Flat pixel index of `point`, in this buffer's own 0-based space.
    ///
    /// **The only place addressing is written** — go through this or through
    /// [`row_stride`](Self::row_stride), never a second `y * width + x`, or a
    /// later change to the origin lands one path in the wrong row and leaves
    /// the other correct.
    ///
    /// `point` must be inside the viewport: bounds-check with
    /// [`point_to_subpart`](Self::point_to_subpart) or clip with
    /// [`local_bounds`](Self::local_bounds) first.
    pub fn flat_index(&self, point: Point) -> usize {
        let local = point - self.viewport().top_left;
        local.y as usize * self.row_stride() + local.x as usize
    }

    /// Flat-index distance between vertically adjacent pixels: the viewport's
    /// width **padded to a whole storage unit**.
    ///
    /// So a row never straddles a unit — the invariant the whole module rests
    /// on. It is what [`units_for`] counts, what makes
    /// [`fill_solid`](Self::fill_solid)'s whole-word run safe to `slice::fill`
    /// without touching a neighboring row, and what lets a driver send a
    /// detached buffer to a panel row-wise without repacking. For a color with
    /// a word of its own (`PPS == 1`) it is simply the width.
    pub fn row_stride(&self) -> usize {
        let pps = C::pps();
        (self.viewport().size.width as usize).div_ceil(pps) * pps
    }

    /// `area` clipped to this buffer. A zero-sized result means nothing to do.
    pub fn local_bounds(&self, area: Rect) -> Rect {
        area.intersection(&self.viewport())
    }

    pub fn point_to_subpart(&self, point: Point) -> Option<(usize, usize)> {
        if !self.viewport().contains(point) {
            return None;
        }
        let index = self.flat_index(point);
        Some((index / C::pps(), index % C::pps()))
    }

    pub fn draw_buffer(&self, f: impl FnOnce(&[C::Storage])) {
        f(self.data())
    }
}

impl<C: Color + PackedColor, B: FramebufStorage<C>> Framebuf<C, B> {
    /// Wrap the caller's `buffer`, **aimed at nothing** — a color buffer has
    /// capacity, not a shape. That is what makes it small: a 240×240 RGB565
    /// frame is 112.5 KiB, a buffer for any region up to a 240×24 tile 11.25.
    ///
    /// Does not allocate; [`into_buffer`](Self::into_buffer) hands it back.
    /// Infallible — whether it is big enough is a question about a region or a
    /// frame policy, answered by `begin_region` and `attach`.
    pub fn new(buffer: B) -> Self {
        Self {
            viewport: Rect::zero(),
            pixels: buffer,
            color: core::marker::PhantomData,
        }
    }

    /// Give the buffer back to its owner.
    pub fn into_buffer(self) -> B {
        self.pixels
    }

    /// Storage units this buffer can hold, independent of shape.
    pub fn capacity_units(&self) -> usize {
        self.pixels.unit_count()
    }

    /// Re-aim the buffer at `region` (absolute screen coordinates).
    ///
    /// The region's own width becomes the stride, so any shape fitting the
    /// capacity works.
    ///
    /// Contents are **not** cleared, so every region must paint its own
    /// background first. Unchecked — `FramebufBlitter::begin_region` refuses an
    /// oversized region before calling this.
    pub(crate) fn retarget(&mut self, region: Rect) {
        self.viewport = region;
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
    use super::*;
    use crate::geometry::Size;

    /// A 1-bpp color with no embedded-graphics anywhere. If this stops
    /// compiling, this module has grown a dependency it must not have.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    struct Mono(bool);

    impl Color for Mono {
        const WHITE: Self = Self(true);
        const BLACK: Self = Self(false);

        fn default_foreground() -> Self {
            Self(true)
        }

        fn default_background() -> Self {
            Self(false)
        }

        fn accents() -> [Self; 6] {
            [Self(true); 6]
        }

        fn map(&self, f: impl Fn(u8) -> u8) -> Self {
            Self(f(self.0 as u8 * 255) > 127)
        }

        fn fold(&self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self {
            Self(f(self.0 as u8 * 255, other.0 as u8 * 255) > 127)
        }

        fn from_rgba(rgba: crate::color::Rgba) -> Self {
            Self(rgba.r > 127)
        }

        fn into_rgba(&self) -> crate::color::Rgba {
            let v = self.0 as u8 * 255;
            crate::color::Rgba { r: v, g: v, b: v, a: 255 }
        }
    }

    impl PackedColor for Mono {
        type Storage = u8;

        const PPS: usize = 8;

        fn into_storage(&self) -> u8 {
            self.0 as u8
        }

        fn as_color(packed: &u8, offset: usize) -> Self {
            Mono((packed >> (7 - offset)) & 1 == 1)
        }

        fn set_color(packed: &mut u8, offset: usize, color: Self) {
            let mask = 1u8 << (7 - offset);
            if color.0 {
                *packed |= mask;
            } else {
                *packed &= !mask;
            }
        }

        fn solid_storage(color: Self) -> u8 {
            if color.0 { 0xff } else { 0x00 }
        }
    }

    /// [`units_for`]'s worked example, as a real check.
    #[test]
    fn units_pad_per_row_not_per_area() {
        assert_eq!(units_for::<Mono>(122, 24), 16 * 24);
        assert_eq!(units_for::<Mono>(128, 24), 16 * 24);
        // Area arithmetic would say 122 * 24 / 8 = 366.
        assert_ne!(units_for::<Mono>(122, 24), 122 * 24 / 8);
    }

    /// **A row never straddles a storage unit.**
    ///
    /// [`units_for`] pads each row to a whole unit and says so ("a 122-pixel
    /// 1-bpp row occupies 16 bytes, not 15.25"), but addressing was
    /// `y * width + x` — unpadded — so on a 122x250 mono panel row 1 began at
    /// byte 15 bit 2. Two things that breaks: a driver flushing the detached
    /// buffer row-wise at the documented stride reads shifted, garbled rows, and
    /// `begin_region` refuses a buffer sized for the packed layout because it
    /// compares it against the padded figure.
    ///
    /// Padded is the layout to keep: it is what a mono panel driver (SSD1680,
    /// SH1106) expects, so a detached buffer can go out over SPI unrepacked.
    #[test]
    fn a_row_never_straddles_a_storage_unit() {
        let panel = Rect::new(Point::zero(), Size::new(122, 250));
        let mut buf = alloc::vec![0u8; units_for::<Mono>(122, 250)];
        let mut fb = Framebuf::<Mono, _>::new(&mut buf[..]);
        fb.retarget(panel);

        // 122 pixels is 15.25 bytes, so a padded row is 16 bytes = 128 pixels.
        assert_eq!(fb.row_stride(), 128, "the stride is padded, in pixels");

        // Row n starts on a byte boundary, for every row.
        for y in 0..250 {
            let (unit, offset) = fb
                .point_to_subpart(Point::new(0, y))
                .expect("the first pixel of every row is addressable");
            assert_eq!(
                (unit, offset),
                (y as usize * 16, 0),
                "row {y} must start at byte {} bit 0",
                y * 16
            );
        }

        // And the last pixel of a row is inside that row's own bytes.
        let (unit, offset) = fb.point_to_subpart(Point::new(121, 0)).unwrap();
        assert_eq!((unit, offset), (15, 1), "121 = byte 15, bit 1");
    }

    /// A mono panel whose width is not a whole number of bytes is
    /// constructible — a 122×250 e-paper panel has 30500 pixels, not divisible
    /// by 8.
    #[test]
    fn a_122px_wide_mono_panel_is_addressable() {
        let panel = Rect::new(Point::zero(), Size::new(122, 250));
        let mut buf = alloc::vec![0u8; units_for::<Mono>(122, 250)];
        let mut fb = Framebuf::<Mono, _>::new(&mut buf[..]);
        fb.retarget(panel);
        assert_eq!(fb.viewport(), panel);
        // The far corner resolves.
        assert!(fb.point_to_subpart(Point::new(121, 249)).is_some());
    }

    /// Capacity is not this type's question — `begin_region` and `attach` are
    /// where a too-small buffer is refused.
    #[test]
    fn capacity_is_not_this_types_question() {
        let mut buf = alloc::vec![0u8; 10];
        let fb = Framebuf::<Mono, _>::new(&mut buf[..]);
        assert_eq!(fb.capacity_units(), 10);
    }
}
