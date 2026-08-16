//! Packed pixel storage, independent of any backend.
//!
//! [`PackedColor`] says how a color packs into a storage word,
//! [`FramebufStorage`] is a caller-owned buffer, and [`Framebuf`] is the
//! addressing arithmetic over the two. The embedded-graphics half — the
//! `PackedColor` impls for its color types, and the `DrawTarget` impl that lets
//! a `Framebuf` *be* a draw target — lives in `eg/framebuf.rs`.
//!
//! **No `embedded_graphics` import may appear here, not even in a doctest.** A
//! doctest compiles as its own crate against whatever features the test command
//! enabled, so one naming an optional dependency breaks `cargo test --features
//! std`. That is why [`units_for`]'s example is a `text` block with a unit test
//! behind it.

use crate::{
    color::Color,
    geometry::{Point, Rect},
};

pub trait PackedColor {
    type Storage: Clone + Send + Sync + 'static;

    /// Pixels per storage unit — 8 for a 1-bit color in a `u8`, 1 for a color
    /// with a word of its own.
    ///
    /// A const rather than only a method: the capacity proof needs it inside
    /// `const { assert!(..) }`, and a trait method cannot be called there.
    const PPS: usize;

    /// Method form of [`PPS`](PackedColor::PPS). Do not override.
    fn pps() -> usize {
        Self::PPS
    }

    fn into_storage(&self) -> Self::Storage;

    fn as_color(packed: &Self::Storage, offset: usize) -> Self;
    fn set_color(packed: &mut Self::Storage, offset: usize, color: Self);

    /// A storage word holding `pps` copies of `color` (mono: `0x00`/`0xFF`;
    /// one-pixel-per-word colors: the pixel itself).
    ///
    /// [`Framebuf::fill_solid`] `slice::fill`s with it. Only whole words are
    /// filled that way, so partial edge words are not this method's problem.
    fn solid_storage(color: Self) -> Self::Storage;
}

// ──────────────────────────────────── Tile capacity, checked at attach
//
// Without `generic_const_exprs`, the two halves meet at the hand-off:
//
//   buffer type ─────────────▶ FramebufStorage::UNITS ─┐
//                                                     ├─▶ RasterRenderer::attach
//   Renderer::Policy + PPS ──▶ policy_units ──────────┘
//
// A surface too small for the policy its renderer declares is rejected before
// anything paints into it: a compile error for a fixed-size array (`UNITS` is
// `Some`), an `attach` error for a runtime-length slice (`UNITS` is `None`).
//
// `attach` rather than `UI::start_frame` because it is the one place that knows
// both numbers. Checking in the frame loop would need `Renderer` to expose a
// capacity, forcing renderers with no surface at all to describe storage.

/// Units of `C::Storage` needed to hold a `w × h` region, **including row
/// padding**.
///
/// Rows pad to a whole number of storage units, which is what makes sub-byte
/// packing correct: a 122-pixel 1-bpp row occupies 16 bytes, not 15.25.
///
/// ```text
/// units_for::<Rgb565>(240, 24)      == 5760   // one unit per pixel
/// units_for::<BinaryColor>(122, 24) == 384    // 1-bpp: 16 bytes per row
/// ```
///
/// (A `text` block, not a doctest: this module must not name
/// embedded-graphics. Both equalities are asserted in [`region_units`]'s
/// doctest and in this module's tests.)
///
/// A delegation to [`region_units`] rather than a copy of it — that is also
/// what [`policy_units`] runs a policy through, and two spellings could drift
/// into a check that passes while the buffer is too small.
///
/// [`policy_units`]: crate::region::policy_units
/// [`region_units`]: crate::renderer::region_units
pub const fn units_for<C: PackedColor>(w: u32, h: u32) -> usize {
    crate::renderer::region_units(w, h, C::PPS)
}

/// A caller-owned buffer the renderer can draw into — **capacity plus access**.
///
/// rsact never holds one. The user lends it to a renderer and takes it back
/// with `detach`. The loan is a move in and a move out rather than a
/// `&'a mut [T]` field, because rsact-ui's `WidgetCtx` is `'static` and so rules
/// out a renderer with a lifetime parameter — and because DMA needs it: a
/// borrow the core could still write through is UB.
pub trait FramebufStorage<C: PackedColor> {
    /// Capacity in storage units when the **type** knows it, `None` when only
    /// the value does.
    ///
    /// `Some(N)` for a fixed-size array, which is what makes a policy violation
    /// a compile error; `None` for a slice, whose length is a runtime fact and
    /// is checked at `attach` instead.
    ///
    /// **`None` means "ask the value", never "unbounded".** Spelling the
    /// unknown case as `usize::MAX` instead reads as "always big enough" and
    /// bypasses the proof entirely — an empty `Box<[u16]>` then satisfies
    /// `assert_policy_fits::<Tiles<240, 240>>` at compile time.
    const UNITS: Option<usize>;

    fn units(&self) -> &[C::Storage];
    fn units_mut(&mut self) -> &mut [C::Storage];

    /// This buffer's real capacity, in storage units. Always available, unlike
    /// [`UNITS`](Self::UNITS) — a slice knows its own length even when its type
    /// does not.
    fn unit_count(&self) -> usize {
        self.units().len()
    }
}

macro_rules! native_framebuf_storage {
    ($($storage:ty),* $(,)?) => {$(
        // ── Statically-sized, and BORROWED ────────────────────────────────
        //
        // `&mut [T; N]` rather than `[T; N]`: the extent stays in the type, so
        // a policy violation is still a compile error, but the loan is a
        // pointer. `attach`/`detach` move `B` by value, so an owned array would
        // memcpy the whole framebuffer twice per region — on the path that
        // exists to avoid copying it. `&'static mut` is also what a
        // `StaticCell` yields, which is where a device's buffer wants to live.
        impl<C: PackedColor<Storage = $storage>, const N: usize> FramebufStorage<C>
            for &mut [$storage; N]
        {
            const UNITS: Option<usize> = Some(N);

            fn units(&self) -> &[$storage] { &self[..] }
            fn units_mut(&mut self) -> &mut [$storage] { &mut self[..] }
        }

        // ── Runtime-sized ─────────────────────────────────────────────────
        //
        // The same loan without a compile-time extent, for a buffer whose size
        // is decided at run time: a `Vec`/`Box` on a host, a runtime-carved
        // region of SDRAM on a device. Checked at `attach` instead.
        impl<C: PackedColor<Storage = $storage>> FramebufStorage<C>
            for &mut [$storage]
        {
            const UNITS: Option<usize> = None;

            fn units(&self) -> &[$storage] { self }
            fn units_mut(&mut self) -> &mut [$storage] { self }
        }

        // NOTE: no impl for `Box<[T]>`. A boxed slice reaches the impl above
        // through `&mut boxed[..]`, and a trait implemented for owned buffers
        // would invite moving one per hand-off.
    )*};
}

// Keyed by storage type, so each impl targets a distinct `Self` and coherence
// holds without any negative reasoning.
native_framebuf_storage!(u8, u16, u32);

// TODO (transport, roadmap 6.7): a byte-buffer view, so an RGB565 tile can be
// handed to SPI as bytes. It needs a newtype — `FramebufStorage<C: …Storage =
// u8>` and `<C: …Storage = u16>` for `[u8; N]` are E0119 conflicting impls,
// since Rust cannot see that a color's `Storage` is only ever one of them — and
// that newtype must make alignment true rather than assumed (`#[repr(align)]`,
// or a constructor that rejects a misaligned slice) before it can hand out
// `&mut [u16]` over a byte array.

/// A packed pixel buffer that addresses an arbitrary rect of the screen.
///
/// The rect is not fixed: `viewport` is the region the buffer currently stands
/// for, in **absolute** screen coordinates, and its width is the stride — so one
/// allocation of `N` units serves any region needing at most `N` (see
/// `retarget`). A full-frame buffer is the degenerate case.
///
/// Working in absolute coordinates is what makes that cheap:
/// [`flat_index`](Self::flat_index) resolves a point against `viewport`, so no
/// caller translates by hand.
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

    // A framebuffer knows how to *be* read, not where its contents should go:
    // to flush a detached buffer, walk rows at `viewport().size.width` and
    // convert with `PackedColor::as_color` — what a DMA burst does with a
    // `CASET`/`RASET` window, and what the host tests do to compare frames.

    /// Flat pixel index of `point`, in this buffer's own 0-based space.
    ///
    /// **The single source of truth for addressing.** Every path from a
    /// coordinate to a storage index goes through this, or through
    /// [`row_stride`] to step between rows; open-coding `y * width + x` a second
    /// time means a later change to the origin lands one path in the wrong row
    /// and leaves the other correct — a plausible image rather than an obvious
    /// failure.
    ///
    /// `point` must be inside [`viewport`]: bounds-check with
    /// [`point_to_subpart`] or clip with [`local_bounds`] first.
    ///
    /// [`row_stride`]: Self::row_stride
    /// [`viewport`]: Self::viewport
    /// [`point_to_subpart`]: Self::point_to_subpart
    /// [`local_bounds`]: Self::local_bounds
    pub fn flat_index(&self, point: Point) -> usize {
        let viewport = self.viewport();
        let local = point - viewport.top_left;
        local.y as usize * viewport.size.width as usize + local.x as usize
    }

    /// Flat-index distance between vertically adjacent pixels — i.e. one row.
    /// A buffer's own width *is* its stride, so this derives from [`viewport`]
    /// like [`flat_index`] does.
    ///
    /// [`viewport`]: Self::viewport
    /// [`flat_index`]: Self::flat_index
    pub fn row_stride(&self) -> usize {
        self.viewport().size.width as usize
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
    /// capacity, not a shape, and only `retarget` gives it
    /// one. That is the point: a 240×240 RGB565 frame is 57600 units
    /// (112.5 KiB), while a buffer holding any region up to a 240×24 tile is
    /// 5760 (11.25 KiB).
    ///
    /// Does not allocate and does not keep the buffer —
    /// [`into_buffer`](Self::into_buffer) hands it back, so an embedded app can
    /// keep its tiles in a `StaticCell` pool and lend out `&'static mut` slices.
    ///
    /// Infallible: any buffer is a valid buffer of its own size, and "is it big
    /// enough" is a question about a region or a frame policy, answered by
    /// `FramebufBlitter::begin_region` and `RasterRenderer::attach`.
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

    /// Storage units this buffer can hold — its capacity, independent of shape.
    pub fn capacity_units(&self) -> usize {
        self.pixels.unit_count()
    }

    /// Re-aim the buffer at `region` (absolute screen coordinates).
    ///
    /// The region's **own width becomes the stride**, so the sub-rect is
    /// contiguous by construction and any shape fitting the capacity works —
    /// which is what lets a frame policy be a byte budget rather than a
    /// rectangle.
    ///
    /// Contents are *not* cleared: a tile arrives holding whatever the last one
    /// left in it, so every region must paint its own background first.
    ///
    /// Unchecked. `FramebufBlitter::begin_region` is the only caller and the one
    /// with a `Result` to return, so it refuses an oversized region there rather
    /// than panicking here.
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

    /// A 1-bpp color with **no embedded-graphics anywhere**. If this stops
    /// compiling, `PackedColor` or `Framebuf` has grown a dependency it must
    /// not have.
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

    /// The worked example on [`units_for`], as a real check. Rows pad to whole
    /// storage units, so a 122-pixel 1-bpp row costs 16 bytes and not 15.25 —
    /// the arithmetic a `size.area() / pps` allocation gets wrong.
    #[test]
    fn units_pad_per_row_not_per_area() {
        assert_eq!(units_for::<Mono>(122, 24), 16 * 24);
        assert_eq!(units_for::<Mono>(128, 24), 16 * 24);
        // Area arithmetic would say 122 * 24 / 8 = 366.
        assert_ne!(units_for::<Mono>(122, 24), 122 * 24 / 8);
    }

    /// A mono panel whose width is not a whole number of bytes is
    /// constructible. Rejecting `area % pps != 0` instead would refuse a real
    /// 122×250 e-paper panel, whose 30500 pixels are not divisible by 8.
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

    /// Capacity is not this type's question: a buffer too small for its region
    /// is refused by `FramebufBlitter::begin_region`, and one too small for the
    /// frame policy by `RasterRenderer::attach`.
    #[test]
    fn capacity_is_not_this_types_question() {
        let mut buf = alloc::vec![0u8; 10];
        let fb = Framebuf::<Mono, _>::new(&mut buf[..]);
        assert_eq!(fb.capacity_units(), 10);
    }
}
