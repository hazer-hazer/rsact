//! Packed pixel storage, independent of any backend.
//!
//! **WS6.4e: this module used to be `eg/framebuf.rs`.** Nothing in it was ever
//! specific to embedded-graphics — `PackedColor` describes how a color packs
//! into a storage word, [`FramebufStorage`] describes a caller-owned buffer,
//! and [`Framebuf`] is addressing arithmetic over the two. embedded-graphics
//! was merely its only *current* user, and living under `eg/` meant the
//! forthcoming `FramebufBlitter` (the render layer split's L3) could not use it
//! without dragging that dependency in.
//!
//! What stayed behind in `eg/framebuf.rs` is exactly what needs the crate: the
//! [`PackedColor`] impls for embedded-graphics' color types, and the
//! `Dimensions`/`DrawTarget` impls that let a [`Framebuf`] *be* an
//! embedded-graphics draw target.
//!
//! Two consequences worth knowing before editing this file:
//!
//! - **No `embedded_graphics` import may appear here**, not even in a doctest —
//!   a doctest compiles as its own crate against whatever features the test
//!   command enabled, so one naming an optional dependency breaks
//!   `cargo test --features std`. That is why [`units_for`]'s worked example is
//!   a `text` block with a real unit test behind it.
//! - The renames that came with the move: `PackedFramebuf` → [`Framebuf`] (the
//!   `Framebuf` *trait* was dissolved in WS6.4d(9), so the name is free and
//!   correct — this is the only framebuffer type), and the `Framebuffer` trait
//!   → [`FramebufStorage`], which is what it is about: capacity plus access
//!   over a caller-owned buffer.

use crate::{
    color::Color,
    geometry::{Point, Rect, Size},
};

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
    /// entirely of that color. Used by the fast `fill_solid` to `slice::fill`
    /// the run of storage words fully inside a rect (mono: `0x00`/`0xFF`; RGB
    /// where `pps == 1`: just the pixel word). Partial edge words still go
    /// through `set_color`, so this need only cover full words.
    fn solid_storage(color: Self) -> Self::Storage;
}

// ─────────────────────────────── WS6.4.0(iii): tile capacity, checked at attach
//
// The chain, with no `generic_const_exprs`:
//
//   buffer type ─────────────▶ FramebufStorage::UNITS ─┐
//                                                     ├─▶ RasterRenderer::attach
//   Renderer::Policy + PPS ──▶ policy_units ──────────┘
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
///
/// ```text
/// units_for::<Rgb565>(240, 24)      == 5760   // one unit per pixel
/// units_for::<BinaryColor>(122, 24) == 384    // 1-bpp: 16 bytes per row
/// ```
///
/// (A `text` block rather than a doctest because this module must not name
/// embedded-graphics — see the module docs. The equalities are asserted in
/// [`region_units`]'s own doctest and in this module's tests.)
///
/// The arithmetic itself lives in [`region_units`] — this is the color-typed
/// wrapper. Deliberately a delegation and not a copy: the same formula is what
/// [`policy_units`] runs a policy through, and two spellings of it could drift
/// into a check that passes while the buffer is too small.
///
/// [`policy_units`]: crate::region::policy_units
/// [`region_units`]: crate::renderer::region_units
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
pub trait FramebufStorage<C: PackedColor> {
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

macro_rules! native_framebuf_storage {
    ($($storage:ty),* $(,)?) => {$(
        // ── Statically-sized, and BORROWED ────────────────────────────────
        //
        // `&mut [T; N]` rather than `[T; N]`: the extent is still in the type,
        // so a policy violation is still a compile error, but the loan is a
        // pointer. An owned array was an anti-pattern hiding in plain sight —
        // `attach` and `detach` move `B` by value, so `[u16; 5760]` memcpy'd
        // 11.25 KiB **on every hand-off**, i.e. twice per region on the exact
        // path that exists to avoid copying a framebuffer.
        //
        // On a device this is what a `StaticCell`/`ConstStaticCell` yields, which
        // is where the buffer wants to live anyway.
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

        // NOTE (WS6.4d): there was a third impl, for `Box<[T]>`, and it earned
        // its removal twice. It is redundant — a boxed slice reaches the second
        // impl through `&mut boxed[..]`, so nothing needs a `Box`-shaped
        // implementation — and before that it was the vehicle for the
        // `UNITS = usize::MAX` bypass, since a heap buffer has no compile-time
        // extent to state. Owned storage of any kind is the wrong shape here:
        // the renderer BORROWS a surface, and a trait implemented for owned
        // buffers invites moving one per hand-off.
    )*};
}

// Keyed by storage type, so each impl targets a distinct `Self` and coherence
// holds without any negative reasoning.
native_framebuf_storage!(u8, u16, u32);

// NOTE (WS6.4d): `pub struct AsBytes<B>(pub B)` lived here — a raw byte buffer
// viewed as storage for a wider color, i.e. an RGB565 tile handed to SPI as
// bytes. It was removed as **unused and unusable**, but the idea is real and
// this records what reviving it takes.
//
// It only ever implemented the capacity half of the buffer contract, never the
// access half, so nothing could draw into one: `units_mut` would have to hand
// out `&mut [u16]` over a `[u8; N]`, and a byte array carries no guarantee it is
// 2-aligned. Nothing in the workspace ever constructed one, so the type was a
// capacity claim about a buffer that could not be a buffer — and its existence
// was the sole reason capacity and access were two traits rather than one (see
// `FramebufStorage`).
//
// The newtype itself was NOT gratuitous, and would be needed again:
// `impl<C: PackedColor<Storage = u8>> FramebufStorage<C> for [u8; N]` and
// `impl<C: PackedColor<Storage = u16>> FramebufStorage<C> for [u8; N]` are
// `E0119` conflicting impls, because Rust does no negative reasoning over
// associated types and cannot see that a color's `Storage` is only ever one of
// them.
//
// To bring it back, the wrapper has to make alignment true rather than assumed —
// `#[repr(align(4))]` on the newtype, or a constructor that fails on a
// misaligned slice — and only then can it implement `FramebufStorage`. That
// belongs with the transport work (roadmap 6.7), where an actual caller would
// exist to state what alignment its DMA engine needs.

// NOTE (WS6.4d): `assert_region_fits<C, B>(w, h)` lived here — a `const fn`
// asserting a `w × h` region fits buffer `B`. Removed as **dead**: its only
// callers were its own doctests. The check it performed is now
// `RasterRenderer::assert_static_capacity`, which runs the same comparison against
// the renderer's declared `FramePolicy` rather than against a rectangle a caller
// passes by hand — one fewer way to state the same requirement, and the one
// that cannot disagree with what the planner will actually emit.

// NOTE (WS6.4d): `pub trait Framebuf<C>` lived here — `data`/`data_mut`/
// `viewport` as required methods, with `pixel`, `set_pixel`, `flat_index`,
// `row_stride`, `local_bounds` and `point_to_subpart` defaulted on top. It had
// exactly one real implementor, `PackedFramebuf`, plus one in a test
// (`OffsetBuf`, which existed only to give the origin term a second value back
// when `PackedFramebuf`'s viewport was pinned at the origin — no longer true
// since `retarget`).
//
// A trait serving one type's own methods is not an abstraction; it is those
// methods with an extra name and an extra import. The name cost was real too:
// `Framebuf` sat two characters from `Framebuffer`, both public in this module,
// and rustc was already emitting "similarly named trait" hints on a typo.
//
// The methods are now inherent on the struct, unchanged — and WS6.4e took the
// freed name for it. `flat_index` is still the single source of truth for
// addressing, and its doc still says why.

/// A packed pixel buffer that addresses an arbitrary rect of the screen.
///
/// **WS6.4d: the rect is not fixed.** `viewport` is the region this buffer
/// currently stands for, in *absolute* screen coordinates, and its width is the
/// stride — so one allocation of `N` storage units serves any region needing at
/// most `N` (see `Self::retarget`). A full-frame buffer is the degenerate
/// case: origin zero, size the screen, retargeted never.
///
/// Absolute coordinates throughout is what makes this cheap:
/// [`flat_index`](Self::flat_index) resolves a point against `viewport`, so
/// every write, read, fill and flush follows the origin without a single caller
/// translating by hand.
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

    /// WS6.3b: fill a rectangle with a single color without the per-pixel
    /// bit-twiddling a `draw_iter` fan-out costs. Every row's pixel range is
    /// split into a partial head word, a run of WHOLE storage words, and a
    /// partial tail word: the whole words are `slice::fill`ed (mono: whole-byte
    /// 0x00/0xFF writes — the 8–32× win; RGB: a `slice::fill` run), and only the
    /// two edge words go through `set_color`. Whole words are entirely inside
    /// the row, so filling them can't corrupt a neighbouring row that shares an
    /// edge byte (mono rows straddle bytes) — those shared bytes are always
    /// partial, hence bit-precise. This is the render-side counterpart to
    /// WS6.3a's flush-side region scoping; every clear/background/block fill
    /// uses it.
    ///
    /// **WS6.4e: inherent, and this is the primary form.** It was a
    /// `DrawTarget::fill_solid` override, which put the fast path behind a trait
    /// only the embedded-graphics backend implements — so the layer split's
    /// `FramebufBlitter::fill_rect` would have inherited a framebuffer without
    /// the win and re-forked the addressing to get it back. The `DrawTarget`
    /// override survives as a one-line delegation to this
    /// (`eg/framebuf.rs`), which is what keeps every existing eg path fast.
    ///
    /// `area` is **absolute** and is clipped to this buffer; a rect that misses
    /// it entirely is a no-op.
    pub fn fill_solid(&mut self, area: Rect, color: C) {
        // WS6.4.0(i-2): clipping and addressing both come from this type now,
        // not from a second open-coded copy of `y*width + x` — see
        // [`flat_index`](Self::flat_index). The row start is computed ONCE and
        // stepped by `row_stride`, so a non-zero buffer origin (the tiled work)
        // lands here for free.
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

    // NOTE (WS6.4d): `output` / `output_region` lived here — a loop turning this
    // buffer into `Pixel`s and pushing them at a `RenderTarget`. They went with
    // that trait (see `output/mod.rs`): a framebuffer knows how to *be* read,
    // not where its contents should go.
    //
    // Reading is still here, and is the only part that was ever rsact's:
    // `pixel(point)` resolves an absolute coordinate against `viewport()`, and
    // `data()` hands out the raw units. A caller flushing a detached buffer walks
    // rows at `region.size.width` and converts with `PackedColor::as_color` —
    // which is exactly what a DMA burst does with a `CASET`/`RASET` window, and
    // what the host tests do to compare frames.

    /// Flat pixel index of `point`, in this buffer's own 0-based space.
    ///
    /// **WS6.4.0(i-2): the single source of truth for addressing.** Every path
    /// that turns a coordinate into a storage index must route through this (or
    /// [`row_stride`] to step between rows). `point` must be inside
    /// [`viewport`] — callers bounds-check first ([`point_to_subpart`]) or clip
    /// first ([`local_bounds`]).
    ///
    /// It used to be open-coded in two places: here and in the fast
    /// [`fill_solid`]'s row loop, whose comment even noted it was "same as
    /// `point_to_subpart`". That duplication is a trap for the tiled work:
    /// giving the buffer a non-zero origin and updating only one of them lands
    /// WS6.3b's fast solid fills in the wrong row while per-pixel writes stay
    /// correct — a *plausible* image rather than an obvious failure. Fold the
    /// origin in here and both paths follow.
    ///
    /// [`row_stride`]: Self::row_stride
    /// [`viewport`]: Self::viewport
    /// [`point_to_subpart`]: Self::point_to_subpart
    /// [`local_bounds`]: Self::local_bounds
    /// [`fill_solid`]: Self::fill_solid
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
    /// Wrap the caller's `buffer`, aimed at `size` from the origin.
    ///
    /// **The buffer is the caller's.** This does not allocate and does not keep
    /// it — [`into_buffer`](Self::into_buffer) hands it back. rsact is the
    /// borrower here, which is what lets an embedded app keep its tiles in a
    /// `StaticCell` pool and pass `&'static mut` slices through channels.
    ///
    /// `None` if `buffer` cannot hold a `size`-shaped region.
    ///
    /// **An `Option`, not an assert.** Whether a too-small buffer should abort
    /// is the caller's decision, not this crate's — an `unwrap` at the call site
    /// is that decision, written where a reader can see it. (WS6.4e replaced an
    /// `area % pps == 0` assert here with a capacity assert; this replaces the
    /// assert itself.)
    ///
    /// The requirement is [`units_for`], the same row-padded formula the policy
    /// proof uses; three spellings of "does it fit" that could disagree is
    /// exactly how a capacity check ends up worse than no check at all. Row
    /// padding makes it very slightly stricter than the addressing needs — a
    /// 122-px mono row is 16 bytes here and 15.25 to `flat_index` — and that is
    /// the direction to be strict in: it is the requirement once regions are
    /// byte-aligned (roadmap 6.5), and identical for every color that does not
    /// pack.
    pub fn new(size: Size, buffer: B) -> Option<Self> {
        if buffer.unit_count() < units_for::<C>(size.width, size.height) {
            return None;
        }
        Some(Self {
            viewport: Rect::new(Point::zero(), size),
            pixels: buffer,
            color: core::marker::PhantomData,
        })
    }

    /// WS6.4d: hold `buffer`'s storage units with **no fixed shape**, for a
    /// buffer that will be `retarget`ed per region.
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
    /// **Unchecked, and deliberately so.** It used to carry a `debug_assert`
    /// that the region fits, which was a panic on a path that must never panic.
    /// The check moved up one layer to `FramebufBlitter::begin_region`, which
    /// **refuses** an oversized region and logs — the only caller, and the one
    /// that has a `Result` to return.
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

    /// A 1-bpp color with **no embedded-graphics anywhere** — which is the
    /// point of WS6.4e, asserted rather than described. If this stops
    /// compiling, `PackedColor` (or `Framebuf`) has grown a dependency it must
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

    /// WS6.4e / roadmap 6.5(i): a mono panel whose width is not a whole number
    /// of bytes is **constructible**. `Framebuf::new` used to assert
    /// `area % pps == 0`, so a real 122×250 e-paper panel panicked — 30500
    /// pixels is not divisible by 8.
    #[test]
    fn a_122px_wide_mono_panel_is_constructible() {
        let mut buf = alloc::vec![0u8; units_for::<Mono>(122, 250)];
        let fb = Framebuf::<Mono, _>::new(Size::new(122, 250), &mut buf[..])
            .expect("the buffer was sized with units_for");
        assert_eq!(
            fb.viewport(),
            Rect::new(Point::zero(), Size::new(122, 250))
        );
    }

    /// The check that replaced it is the one that was missing: too small is
    /// refused, at construction, before anything paints — and as a `None`, not
    /// a panic. Whether that should abort is the caller's decision.
    #[test]
    fn a_buffer_too_small_for_its_size_is_refused() {
        let mut buf = alloc::vec![0u8; 10];
        assert!(
            Framebuf::<Mono, _>::new(Size::new(122, 250), &mut buf[..])
                .is_none()
        );
    }
}
