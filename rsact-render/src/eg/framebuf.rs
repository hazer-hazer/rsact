use crate::{
    color::Color,
    geometry::{Point, Rect, Size},
    output::{MapColor, RenderTarget, pixel::Pixel},
    renderer::region_units,
};
use alloc::boxed::Box;
use embedded_graphics::{
    geometry::Dimensions,
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

// ─────────────────────────────── WS6.4.0(iii): tile capacity, checked at attach
//
// The chain, with no `generic_const_exprs`:
//
//   buffer type ─────────────▶ Framebuffer::UNITS ────┐
//                                                  ├─▶ EGRenderer::attach
//   Renderer::Policy + PPS ──▶ policy_units ───────┘
//
// so a surface too small for the policy its renderer declares is rejected
// before anything paints into it — a **compile error** when the buffer is a
// fixed-size array (`UNITS` is `Some`), an assert at the hand-off when it is a
// runtime-length slice (`UNITS` is `None`).
//
// It is deliberately NOT in `UI::start_frame`, where it lived through 6.4d's
// first shape. That required the `Renderer` trait to expose a capacity, which
// forced every renderer — including ones with no surface at all — to describe
// storage just so the ones that have it could be checked. The comparison now
// happens in the one place that legitimately knows both numbers.

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
///
/// The arithmetic itself lives in [`region_units`] — this is the colour-typed
/// wrapper. Deliberately a delegation and not a copy: the same formula is what
/// [`policy_units`] runs a policy through, and two spellings of it could drift
/// into a check that passes while the buffer is too small.
///
/// [`policy_units`]: crate::region::policy_units
pub const fn units_for<C: PackedColor>(w: u32, h: u32) -> usize {
    crate::renderer::region_units(w, h, C::PPS)
}

/// A caller-owned buffer the renderer can draw into — **capacity plus access**.
///
/// rsact never holds one of these. The user hands it to their concrete renderer
/// through that backend's own inherent API (roadmap 6.4.0, "surface
/// ownership"); the renderer borrows it for as long as the user chooses and
/// gives it back with `detach`. That loan is a move in and a move out rather
/// than a `&'a mut [T]` field, because `WidgetCtx: 'static` (`el/ctx.rs:5`)
/// rules out a renderer with a lifetime parameter — and it is also the shape
/// DMA wants, since a borrow the core could still write through is UB.
///
/// **This was two traits**, `Framebuffer` (capacity) and `Surface: PixelBuf`
/// (access). The split existed for exactly one type — `AsBytes<[u8; N]>`, a
/// wire-format view that could state a capacity but could not hand out
/// `&mut [u16]` without an alignment guarantee a byte array does not carry. That
/// type was never constructed anywhere, so the split cost two names, two bounds
/// and two impls per buffer to describe a case that did not exist. See the note
/// where `AsBytes` was removed for what reviving it would take.
pub trait Framebuffer<C: PackedColor> {
    /// Capacity in storage units when the **type** knows it, `None` when only
    /// the value does.
    ///
    /// `Some(N)` for a fixed-size array, which is what lets a policy violation
    /// be a compile error. `None` for a slice or a boxed slice, whose length is
    /// a runtime fact — those are checked at `attach` instead.
    ///
    /// **`None` means "ask the value", never "unbounded".** This was
    /// `const UNITS: usize` with `usize::MAX` for the heap case, which read as
    /// "my surface always covers the frame" — the sentinel for *no constraint*.
    /// The result was a total bypass: an **empty** `Box<[u16]>` satisfied
    /// `assert_policy_fits::<Tiles<240, 240>>` at compile time and constructed a
    /// tiled renderer over zero bytes of storage. A capacity a type cannot state
    /// must be absent, not infinite; [`unit_count`](Self::unit_count) is where
    /// the real number lives.
    const UNITS: Option<usize>;

    fn units(&self) -> &[C::Storage];
    fn units_mut(&mut self) -> &mut [C::Storage];

    /// This buffer's real capacity, in storage units.
    ///
    /// Always available, unlike [`UNITS`](Self::UNITS) — a slice knows its own
    /// length even when its type does not. The backend compares this against
    /// its frame policy on every `attach`, so a runtime-sized buffer is checked
    /// exactly once, at the moment it is lent, rather than never.
    fn unit_count(&self) -> usize {
        self.units().len()
    }
}

macro_rules! native_framebuffer {
    ($($storage:ty),* $(,)?) => {$(
        // The embedded case: extent is in the type, so the check is a `const`.
        impl<C: PackedColor<Storage = $storage>, const N: usize> Framebuffer<C>
            for [$storage; N]
        {
            const UNITS: Option<usize> = Some(N);

            fn units(&self) -> &[$storage] { self }
            fn units_mut(&mut self) -> &mut [$storage] { self }
        }

        // A borrowed slice — how an app places a buffer in a *particular* memory
        // region (SDRAM, DTCM, a `#[link_section]` pool, a `StaticCell`) and
        // lends it out. `'static` in practice, because `WidgetCtx: 'static`
        // rules out a renderer with a lifetime parameter; nothing here demands
        // it, so a shorter borrow works wherever the renderer is local.
        //
        // Extent is a runtime fact, so `UNITS` is `None` and the capacity check
        // happens at `attach`.
        impl<C: PackedColor<Storage = $storage>> Framebuffer<C>
            for &mut [$storage]
        {
            const UNITS: Option<usize> = None;

            fn units(&self) -> &[$storage] { self }
            fn units_mut(&mut self) -> &mut [$storage] { self }
        }

        // The heap case — a host, a simulator, a desktop target, or any
        // embedded target with a global allocator. Same runtime extent, same
        // `None`. Not `std`-gated: `Box` is `alloc`, which this crate always
        // has.
        impl<C: PackedColor<Storage = $storage>> Framebuffer<C>
            for Box<[$storage]>
        {
            const UNITS: Option<usize> = None;

            fn units(&self) -> &[$storage] { self }
            fn units_mut(&mut self) -> &mut [$storage] { self }
        }
    )*};
}

// Keyed by storage type, so each impl targets a distinct `Self` and coherence
// holds without any negative reasoning.
native_framebuffer!(u8, u16, u32);

// NOTE (WS6.4d): `pub struct AsBytes<B>(pub B)` lived here — a raw byte buffer
// viewed as storage for a wider colour, i.e. an RGB565 tile handed to SPI as
// bytes. It was removed as **unused and unusable**, but the idea is real and
// this records what reviving it takes.
//
// It only ever implemented the capacity half of the buffer contract, never the
// access half, so nothing could draw into one: `units_mut` would have to hand
// out `&mut [u16]` over a `[u8; N]`, and a byte array carries no guarantee it is
// 2-aligned. Nothing in the workspace ever constructed one, so the type was a
// capacity claim about a buffer that could not be a buffer — and its existence
// was the sole reason `Framebuffer` and `Framebuffer` were two traits rather than one
// (see `Framebuffer`).
//
// The newtype itself was NOT gratuitous, and would be needed again:
// `impl<C: PackedColor<Storage = u8>> Framebuffer<C> for [u8; N]` and
// `impl<C: PackedColor<Storage = u16>> Framebuffer<C> for [u8; N]` are `E0119`
// conflicting impls, because Rust does no negative reasoning over associated
// types and cannot see that a colour's `Storage` is only ever one of them.
//
// To bring it back, the wrapper has to make alignment true rather than assumed —
// `#[repr(align(4))]` on the newtype, or a constructor that fails on a
// misaligned slice — and only then can it implement `Framebuffer`. That belongs
// with the transport work (roadmap 6.7), where an actual caller would exist to
// state what alignment its DMA engine needs.

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
pub const fn assert_region_fits<C: PackedColor, B: Framebuffer<C>>(
    w: u32,
    h: u32,
) {
    // A buffer whose extent is only a runtime fact cannot be proved here — and
    // must not be silently *assumed* to fit, which is what the old `usize::MAX`
    // sentinel did. It is checked against the same requirement at `attach`.
    if let Some(units) = B::UNITS {
        assert!(
            units_for::<C>(w, h) <= units,
            "region does not fit the pixel buffer — see the instantiation in \
             this error for the colour, buffer type and region size"
        );
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

/// A packed pixel buffer that addresses an arbitrary rect of the screen.
///
/// **WS6.4d: the rect is not fixed.** `viewport` is the region this buffer
/// currently stands for, in *absolute* screen coordinates, and its width is the
/// stride — so one allocation of `N` storage units serves any region needing at
/// most `N` (see [`Self::retarget`]). A full-frame buffer is the degenerate
/// case: origin zero, size the screen, retargeted never.
///
/// Absolute coordinates throughout is what makes this cheap: [`Framebuf::
/// flat_index`] resolves a point against `viewport`, so every write, read, fill
/// and flush follows the origin without a single caller translating by hand.
pub struct PackedFramebuf<C: Color + PackedColor, B: Framebuffer<C>> {
    viewport: Rect,
    pixels: B,
    color: core::marker::PhantomData<C>,
}

// `Dimensions`, not `OriginDimensions`: embedded-graphics' `clipped`/`cropped`
// intersect against this box, and rsact hands them ABSOLUTE rects. Reporting
// origin-zero was correct only while the buffer always was the whole frame; a
// tile at (0, 24) would have had its every write clipped away.
impl<C: Color + PackedColor, B: Framebuffer<C>> Dimensions
    for PackedFramebuf<C, B>
{
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        self.viewport.into()
    }
}

impl<
    C: Color + PackedColor + embedded_graphics::prelude::PixelColor,
    B: Framebuffer<C>,
> DrawTarget for PackedFramebuf<C, B>
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

        Ok(())
    }
}

impl<C: Color + PackedColor, B: Framebuffer<C>> Framebuf<C>
    for PackedFramebuf<C, B>
{
    fn data(&self) -> &[C::Storage] {
        self.pixels.units()
    }

    fn data_mut(&mut self) -> &mut [C::Storage] {
        self.pixels.units_mut()
    }

    fn viewport(&self) -> Rect {
        self.viewport
    }
}

impl<C: Color + PackedColor, B: Framebuffer<C>> PackedFramebuf<C, B> {
    /// Wrap the caller's `buffer`, aimed at `size` from the origin.
    ///
    /// **The buffer is the caller's.** This does not allocate and does not keep
    /// it — [`into_buffer`](Self::into_buffer) hands it back. rsact is the
    /// borrower here, which is what lets an embedded app keep its tiles in a
    /// `StaticCell` pool and pass `&'static mut` slices through channels.
    pub fn new(size: Size, buffer: B) -> Self {
        // TODO: Not really, unused space is possible, just choose least
        // sufficient framebuf size
        assert!(
            size.area() as usize % C::pps() == 0,
            "PackedFramebuf area must be divisible by {} to store pixels packed",
            C::pps()
        );
        Self {
            viewport: Rect::new(Point::zero(), size),
            pixels: buffer,
            color: core::marker::PhantomData,
        }
    }

    /// WS6.4d: allocate `units` storage units with **no fixed shape**, for a
    /// buffer that will be [`retarget`](Self::retarget)ed per region.
    ///
    /// This is the tiled constructor, and the acceptance target it exists for:
    /// a 240×240 RGB565 frame is 57600 units (112.5 KiB), while a buffer able to
    /// hold any region up to a 240×24 tile is 5760 (11.25 KiB).
    ///
    /// Starts aimed at nothing (a zero-sized viewport at the origin), because
    /// there is no meaningful default region — `begin_region` supplies one.
    pub fn tile(buffer: B) -> Self {
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
    /// contiguous by construction and any shape fitting the capacity works. This
    /// is what lets a frame policy be a byte budget rather than a rectangle.
    ///
    /// **Internal bookkeeping, not an API the user drives.** `begin_region`
    /// calls it; nothing outside the backend should. Re-aiming a buffer is how
    /// absolute coordinates land in storage smaller than the frame — a
    /// consequence of where rsact is painting, never a request the caller
    /// makes. (The caller's operations on a surface are `attach` and `detach`:
    /// lend it, take it back, flush it, lend the next one.)
    ///
    /// Contents are *not* cleared: a tile arrives holding whatever the last one
    /// left in it, which is exactly why every region must paint its own
    /// background before drawing (roadmap 6.4 constraint (b)).
    ///
    /// # Panics
    ///
    /// In debug builds, if `region` needs more units than the buffer holds. The
    /// planner guarantees it never does, and the capacity check at `attach`
    /// guarantees the planner's own bound fits — this is the backstop for a
    /// renderer driven outside that path.
    pub(crate) fn retarget(&mut self, region: Rect) {
        debug_assert!(
            region_units(region.size.width, region.size.height, C::PPS)
                <= self.capacity_units(),
            "[BUG] region {region:?} needs {} storage units, buffer holds {}",
            region_units(region.size.width, region.size.height, C::PPS),
            self.capacity_units(),
        );
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
    use super::{Framebuf, PackedColor, PackedFramebuf};

    /// Host-side test surfaces. Local to the tests on purpose — see the note in
    /// `eg/renderer.rs`'s test module: the library exports no allocating helper
    /// because it must not choose where a framebuffer lives.
    fn heap_surface<C: crate::color::Color + PackedColor>(
        size: Size,
    ) -> alloc::boxed::Box<[<C as PackedColor>::Storage]> {
        alloc::vec![
            C::default_background().into_storage();
            size.area() as usize / C::pps()
        ]
        .into_boxed_slice()
    }
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

        let mut framebuf = PackedFramebuf::new(
            Size::new(WIDTH, HEIGHT),
            heap_surface::<Rgb888>(Size::new(WIDTH, HEIGHT)),
        );

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

        let mut framebuf = PackedFramebuf::new(
            Size::new(WIDTH, HEIGHT),
            heap_surface::<BinaryColor>(Size::new(WIDTH, HEIGHT)),
        );

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
        let framebuf = PackedFramebuf::<Rgb888, _>::new(
            Size::new(W, H),
            heap_surface::<Rgb888>(Size::new(W, H)),
        );

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
        let framebuf = PackedFramebuf::<Rgb888, _>::new(
            Size::new(W, H),
            heap_surface::<Rgb888>(Size::new(W, H)),
        );

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
        let framebuf = PackedFramebuf::<Rgb888, _>::new(
            Size::new(W, H),
            heap_surface::<Rgb888>(Size::new(W, H)),
        );

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

            let mut fast = PackedFramebuf::<Rgb888, _>::new(
                size,
                heap_surface::<Rgb888>(size),
            );
            let mut slow = PackedFramebuf::<Rgb888, _>::new(
                size,
                heap_surface::<Rgb888>(size),
            );
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

            let mut fast = PackedFramebuf::<BinaryColor, _>::new(
                size,
                heap_surface::<BinaryColor>(size),
            );
            let mut slow = PackedFramebuf::<BinaryColor, _>::new(
                size,
                heap_surface::<BinaryColor>(size),
            );
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
        use super::units_for;

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
