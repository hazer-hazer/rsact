use crate::{
    color::{Color, RgbColor},
    eg::primitives,
    framebuf::{Framebuf, FramebufStorage, PackedColor},
    geometry::*,
    image::DrawImage,
    output::pixel::Pixel,
    path::{Path, PathSegment},
    primitives::{
        arc::Arc, circle::Circle, ellipse::Ellipse, line::Line,
        rounded_rect::RoundedRect, sector::Sector,
    },
    region::{FramePolicy, Unbounded, policy_units},
    renderer::{
        Attached, Attachment, Detached, RenderResult, Renderer, ViewportKind,
    },
    style::{DrawStyle, StrokeAlignment},
};
use alloc::vec::Vec;
use core::marker::PhantomData;
use embedded_graphics::{
    Drawable,
    draw_target::DrawTargetExt,
    geometry::OriginDimensions,
    pixelcolor::Rgb888,
    prelude::{Dimensions, DrawTarget, PixelColor},
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, StyledDrawable},
};

/// Proxy to draw on Renderer as on embedded_graphics DrawTarget, works by
/// mapping any color into embedded_graphics Rgb888.
pub struct DrawTargetProxy<'a, R: Renderer> {
    renderer: &'a mut R,
}

impl<'a, R: Renderer> DrawTargetProxy<'a, R> {
    pub fn new(renderer: &'a mut R) -> Self {
        Self { renderer }
    }
}

impl<'a, R: Renderer> OriginDimensions for DrawTargetProxy<'a, R> {
    fn size(&self) -> embedded_graphics::prelude::Size {
        self.renderer.size().into()
    }
}

impl<'a, C: Color, R: Renderer<Color = C>> DrawTarget
    for DrawTargetProxy<'a, R>
{
    type Color = Rgb888;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::prelude::Pixel<Self::Color>>,
    {
        // WS6.4b(ii), the cheap half: drop pixels the renderer would reject
        // anyway, BEFORE paying `Renderer::pixel` for each one.
        //
        // This is the only path text takes — `embedded-text` / u8g2 rasterise
        // glyphs and hand them here one pixel at a time — and it is where the
        // per-pixel cost that survives WS6.4b's part-level cull lives: a label
        // straddling a region boundary is not culled in either region, so every
        // glyph pixel is offered twice and each one pays a color conversion plus
        // a viewport dispatch into the framebuffer to be discarded.
        //
        // It is a cheaper *write filter*, not the loop bound (ii) ultimately
        // wants: the glyph iteration upstream still runs, because the line/glyph
        // loop belongs to `embedded-text`, not to us. Owning that loop — which is
        // also what `font/fixed.rs`'s `Clip`/`Ellipsis` TODO needs — is what makes
        // the first/last-visible-glyph arithmetic possible, and it is filed as the
        // remaining part of (ii).
        //
        // Read once, outside the loop: `clip_bounds` borrows the renderer
        // immutably and `pixel` needs it mutably.
        let clip = self.renderer.clip_bounds();
        pixels
            .into_iter()
            .filter(|p| clip.is_none_or(|clip| clip.contains(Point::from(p.0))))
            .try_for_each(|p| {
                self.renderer.pixel(
                    p.0.into(),
                    C::from_rgba(crate::color::Rgba {
                        r: p.1.r(),
                        g: p.1.g(),
                        b: p.1.b(),
                        a: 255,
                    }),
                )
            })
    }
}

impl Into<embedded_graphics::primitives::StrokeAlignment> for StrokeAlignment {
    fn into(self) -> embedded_graphics::primitives::StrokeAlignment {
        match self {
            Self::Inside => {
                embedded_graphics::primitives::StrokeAlignment::Inside
            },
            Self::Center => {
                embedded_graphics::primitives::StrokeAlignment::Center
            },
            Self::Outside => {
                embedded_graphics::primitives::StrokeAlignment::Outside
            },
        }
    }
}

impl<C: Color + PixelColor> DrawStyle<C> {
    pub fn into_primitive_style(self) -> PrimitiveStyle<C> {
        let mut builder = PrimitiveStyleBuilder::new()
            .stroke_width(self.stroke_width)
            .stroke_alignment(self.stroke_alignment.into());
        if let Some(fill) = self.fill {
            builder = builder.fill_color(fill);
        }
        if let Some(stroke) = self.stroke {
            builder = builder.stroke_color(stroke);
        }
        builder.build()
    }
}

/// Renderer backed by embedded_graphics, drawing into a single owned
/// `Framebuf` under a clip/crop viewport stack.
///
/// Preserves the PackedColor framebuffer optimization, alpha-channel blending,
/// and anti-aliasing. Layer compositing was removed (see [`crate::surface`]).
// NOTE (WS6.4d): this used to carry a TODO to adopt `crate::surface::Canvas`,
// the shared surface + viewport-stack helper. That module is gone instead, and
// the direction reversed: `Canvas<T>` constructed its surface (`T::new(size)`),
// which is precisely the allocation a renderer must not perform, and it could
// not express a DETACHED surface at all. Both backends now hold
// `Option<surface>` plus an inline `viewport_stack`, which is the same few
// lines twice rather than an abstraction that fights the ownership rule.
/// `B` is the **user's** surface. The renderer borrows it: it never allocates
/// one, and [`detach`](Self::detach) hands it back. That is what makes the
/// framebuffer the application's property while keeping `W::Renderer` free of a
/// lifetime parameter — `WidgetCtx: 'static` (`el/ctx.rs:5`) rules out a
/// `&'a mut [T]` field, so the loan is expressed by moving the buffer in and
/// out. On embedded that is exactly the shape a `StaticCell` tile pool wants:
/// `&'static mut Tile` values passed through channels.
///
/// `P` is the [frame policy](FramePolicy) this renderer obeys — the largest
/// region rsact may ask it to paint, and the only thing rsact knows about its
/// storage. `B`'s capacity is checked against `P` when a buffer is
/// [attached](Self::attach): at compile time for a fixed-size array
/// ([`FramebufStorage::UNITS`] is `Some`), at the hand-off for a runtime-length slice.
pub struct EGRenderer<
    C: Color + PackedColor,
    B: FramebufStorage<C>,
    P: FramePolicy = Unbounded,
    A: Attachment<Framebuf<C, B>> = Attached,
> {
    viewport_stack: Vec<ViewportKind>,
    /// The lent surface — and **only** in the [`Attached`] state, where its type
    /// is `Framebuf<C, B>`. In [`Detached`] it is `()`: not an absent
    /// buffer but no field at all, so there is nothing to unwrap and no
    /// "drawing while detached" case for any method to handle. See
    /// [`Attachment`].
    canvas: A::Slot,
    main_viewport: Size,
    policy: PhantomData<P>,
}

impl<C: Color + PackedColor, B: FramebufStorage<C>>
    EGRenderer<C, B, Unbounded>
{
    /// Full-frame: `buffer` covers the whole display, so no region can ever be
    /// too large and the planner never chunks.
    ///
    /// The buffer is the caller's — a `Box<[u16]>` on a host, a `[u16; N]` or a
    /// `&'static mut [u16]` pointing at SDRAM on a device. **Nothing here
    /// allocates**, and nothing here chooses *where* the memory lives; that is
    /// the application's decision and it is not one rsact can make well.
    ///
    /// This is the constructor that pins [`Unbounded`], which is why it needs no
    /// policy annotation. For a surface smaller than the frame, see
    /// [`tiled`](EGRenderer::tiled).
    pub fn new(viewport: Size, buffer: B) -> Self {
        EGRenderer::<C, B, Unbounded, Detached>::parked(viewport).attach(buffer)
    }
}

impl<C: Color + PackedColor, B: FramebufStorage<C>, P: FramePolicy>
    EGRenderer<C, B, P>
{
    /// **WS6.4d: tiled.** `buffer` is *smaller* than the display, and is
    /// re-aimed at each region by [`Renderer::begin_region`].
    ///
    /// The acceptance target, constructible: a 240×240 RGB565 frame needs 57600
    /// storage units (112.5 KiB), while `[u16; 5760]` is 11.25 KiB and paints
    /// the same frame. `main_viewport` stays the display's size — that is what
    /// rsact lays out and culls against; only the surface shrinks.
    ///
    /// `P` names the largest region rsact may hand it, and `buffer` is checked
    /// against `P` here (see [`attach`](Self::attach)) — at compile time when
    /// the buffer is a fixed-size array. Starts aimed at nothing; the first
    /// `begin_region` supplies a region.
    ///
    /// A type alias keeps the call site short, which is how an app usually
    /// writes it:
    ///
    /// ```ignore
    /// type Screen =
    ///     EGRenderer<Rgb565, &'static mut [u16], Tiles<240, 24>>;
    /// let renderer = Screen::tiled(Size::new_equal(240), tile);
    /// ```
    pub fn tiled(viewport: Size, buffer: B) -> Self {
        EGRenderer::<C, B, P, Detached>::parked(viewport).attach(buffer)
    }

    /// Take the surface back, with the region that was painted into it.
    ///
    /// Consumes the renderer and returns it [`Detached`] — the state where it
    /// has no surface *field*, so nothing can paint into a buffer the caller is
    /// holding. That is the invariant this type-state exists for; it used to be
    /// a runtime `Option` plus a logged no-op, which meant a scheduling mistake
    /// silently ate a frame, once per frame, forever.
    ///
    /// # The returned rect is the region that was painted
    ///
    /// And it is the *only* rect a caller needs: `begin_region` retargets
    /// unconditionally (see it for why), so the buffer's extent and the painted
    /// region are the same rectangle for every surface — a tile and a
    /// full-frame framebuffer alike. It is both what to index the buffer at
    /// (rows are strided at its width) and what to send.
    ///
    /// This used to be two rects. A full-frame surface was not retargeted, so
    /// its extent stayed the whole frame while the region was a sub-rect, and
    /// every caller had to carry both and intersect them. That asymmetry saved
    /// one background fill per region and cost an API; the trade is described on
    /// `begin_region`.
    ///
    /// Before anything is painted the rect is whatever `attach` aimed at: the
    /// whole frame for a frame-sized buffer, empty for a tile.
    pub fn detach(self) -> (EGRenderer<C, B, P, Detached>, B, Rect) {
        let Self { viewport_stack, canvas, main_viewport, .. } = self;
        let at = canvas.viewport();
        let parked = EGRenderer {
            viewport_stack,
            canvas: (),
            main_viewport,
            policy: PhantomData,
        };
        (parked, canvas.into_buffer(), at)
    }

    /// Exchange surfaces in place, returning the painted one and its rect.
    ///
    /// Sugar over [`detach`](Self::detach) + [`attach`], for callers with two or
    /// more buffers — and, unlike those two, it keeps `&mut self`, because the
    /// renderer is never observably without a surface. It is **not** the
    /// primitive: `swap` demands two buffers at the instant of the exchange, so
    /// a single-buffer pool deadlocks under it — the renderer waits for a free
    /// buffer that only its own held buffer could become. Release-then-acquire
    /// has no such cycle:
    ///
    /// ```ignore
    /// // works with one buffer, and with N
    /// frame.render(&mut renderer);
    /// let (parked, tile, dirty) = renderer.detach();
    /// ready.send((tile, dirty)).await;              // publish first…
    /// renderer = parked.attach(free.receive().await); // …then acquire
    /// ```
    pub fn swap(&mut self, next: B) -> (B, Rect) {
        // Runtime check only. The `const` half depends purely on `B`, `C` and
        // `P` — none of which change here, so it already fired for this
        // instantiation when the renderer was built. What can differ per call is
        // a runtime-length buffer's actual length: two `Box<[u32]>` values, or
        // two `&'static mut [u16]`s from different pools, are the same type with
        // different extents.
        Self::check_runtime_capacity(&next);
        let at = self.canvas.viewport();
        let fresh = Self::wrap(self.main_viewport, next);
        let prev = core::mem::replace(&mut self.canvas, fresh);
        (prev.into_buffer(), at)
    }

    /// Aim a freshly-lent buffer.
    ///
    /// A buffer big enough for the whole frame is aimed at the whole frame; a
    /// smaller one is aimed at nothing until `begin_region` supplies a region.
    ///
    /// **This is a bug fix, not bookkeeping.** `attach` used to build a *tile*
    /// framebuf unconditionally, so a full-frame renderer that detached and
    /// reattached (the ordinary flush loop) came back aimed at `Rect::zero()` —
    /// and since `begin_region` returns early for a full-frame surface, nothing
    /// ever re-aimed it. Every frame after the first reported a zero-sized dirty
    /// rect and flushed nothing.
    fn wrap(viewport: Size, buffer: B) -> Framebuf<C, B> {
        let full_frame =
            crate::framebuf::units_for::<C>(viewport.width, viewport.height);
        if buffer.unit_count() >= full_frame {
            Framebuf::new(viewport, buffer)
        } else {
            Framebuf::tile(buffer)
        }
    }

    /// The half of the capacity contract that depends only on types, asserted
    /// at monomorphization.
    ///
    /// Fires **once per instantiation, at compile time**, so it belongs on the
    /// path every construction goes through (`attach`) and nowhere else. A
    /// violation is a compile error naming the color, the buffer and the
    /// policy.
    fn assert_static_capacity() {
        // `UNITS == None` (a runtime-length slice) falls through to the runtime
        // check rather than being assumed to fit — the distinction the old
        // `usize::MAX` sentinel erased.
        const {
            // A bounded policy's unit budget is only meaningful if its packing
            // matches the color actually being stored: a 1-bpp color under a
            // `PIXELS_PER_UNIT = 1` policy would demand eight times the storage
            // it needs, and the reverse would silently under-demand. Unbounded
            // policies do no capacity arithmetic, so their packing is moot.
            if P::MAX_REGION.is_some() {
                assert!(
                    P::PIXELS_PER_UNIT == C::PPS,
                    "this frame policy's pixel packing disagrees with the \
                     renderer's color — see the instantiation in this error"
                );
            }
            if let (Some(units), Some(needed)) =
                (<B as FramebufStorage<C>>::UNITS, policy_units::<P>())
            {
                assert!(
                    needed <= units,
                    "this surface is too small for the renderer's frame \
                     policy — see the instantiation in this error for the \
                     color, buffer type and policy"
                );
            }
        }
    }

    /// The half that depends on the buffer's *value*.
    ///
    /// Only meaningful for a `B` whose extent is a runtime fact — a boxed slice,
    /// a `&'static mut [u16]` from a `StaticCell` pool. For a fixed-size array
    /// this is a redundant repeat of a compile-time proof, and costs one integer
    /// compare per hand-off.
    ///
    /// # Panics
    ///
    /// If `buffer` is too small for `P`. Deliberately not a logged degradation:
    /// the condition is a static property of the application's memory plan, it
    /// is discovered at the hand-off rather than mid-paint, and the only
    /// available fallback — never render again — is a silent brick rather than a
    /// degraded picture.
    fn check_runtime_capacity(buffer: &B) {
        if let Some(needed) = policy_units::<P>() {
            let have = buffer.unit_count();
            assert!(
                have >= needed,
                "[rsact] surface too small for this renderer's frame policy: \
                 it holds {have} storage units, the policy's largest region \
                 needs {needed}"
            );
        }
    }
}

impl<C: Color + PackedColor, B: FramebufStorage<C>, P: FramePolicy>
    EGRenderer<C, B, P, Detached>
{
    /// A renderer with no surface yet — the state a buffer is attached *to*.
    ///
    /// Useful on its own: an app whose tiles come from a channel can build the
    /// renderer at boot and wait for the first buffer, which the old
    /// `Option`-based shape could express only as "constructed but secretly
    /// broken".
    pub fn parked(viewport: Size) -> Self {
        Self {
            viewport_stack: vec![ViewportKind::root()],
            canvas: (),
            main_viewport: viewport,
            policy: PhantomData,
        }
    }

    /// Lend the renderer a surface. Consumes the parked renderer and returns an
    /// [`Attached`] one — the only state that can draw.
    ///
    /// See [`EGRenderer::detach`] for why this pair, rather than `swap`, is the
    /// primitive.
    ///
    /// # Panics
    ///
    /// If `buffer` is too small for policy `P` — see `check_capacity`.
    pub fn attach(self, buffer: B) -> EGRenderer<C, B, P, Attached> {
        EGRenderer::<C, B, P, Attached>::assert_static_capacity();
        EGRenderer::<C, B, P, Attached>::check_runtime_capacity(&buffer);
        let canvas =
            EGRenderer::<C, B, P, Attached>::wrap(self.main_viewport, buffer);
        EGRenderer {
            viewport_stack: self.viewport_stack,
            canvas,
            main_viewport: self.main_viewport,
            policy: PhantomData,
        }
    }
}

impl<C: Color + PackedColor + PixelColor, B: FramebufStorage<C>, P: FramePolicy>
    EGRenderer<C, B, P>
{
    fn current_viewport(&self) -> ViewportKind {
        self.viewport_stack.last().copied().unwrap()
    }

    // NOTE (WS6.4d): `current_canvas() -> Option<&mut _>` lived here, warning
    // and returning `None` when nothing was attached, and every drawing path
    // opened with `let Some(canvas) = self.current_canvas() else { … }`. The
    // `Attached` type-state deleted all of it: this impl block only exists for
    // a renderer that HAS a surface, so `self.canvas` is one — not an
    // `Option<one>`.

    /// Obtain the raw framebuffer data for hardware output.
    pub fn draw_buffer(&self, f: impl FnOnce(&[<C as PackedColor>::Storage])) {
        self.canvas.draw_buffer(f);
    }

    // NOTE (layer split, PR A): `pixel_alpha` and its helper
    // `viewport_to_canvas` lived here. `pixel_alpha` read the destination pixel,
    // `mix`ed the incoming color into it by an `f32` coverage, and wrote the
    // result — the whole of anti-aliasing on this backend, one pixel at a time.
    // Deleted with the AA primitives it served (D1).
    //
    // Two facts it encoded are NOT lost, because both outlive it:
    //
    //   - Blending is a **read-modify-write per pixel**. It defeats
    //     write-combining, and it is why a region must be primed with the true
    //     background before painting (roadmap 6.4 constraint (b)) — otherwise the
    //     first AA edge in a region blends against whatever the last one left.
    //     `begin_region` still primes, and still for that reason.
    //   - Any path touching the canvas **directly** must apply the viewport
    //     transform by hand; the write paths get it free from `DrawTargetExt`.
    //     `viewport_to_canvas` was that transform, and WS6.4.0(i-1) fixed a real
    //     bug where the read skipped it under `Cropped` and so blended against
    //     an unrelated pixel. The hazard survives the deletion: `Cropped` is
    //     itself deleted in PR C, and until then `renderer_begin_region` is the
    //     one direct-canvas path — it primes the WHOLE retargeted buffer, so it
    //     is transform-independent by construction.
    //
    // In the split, blending is `Blitter::blend_span`: coverage arrives as a
    // `&[u8]` run rather than an `f32` per pixel, and the read-modify-write is
    // the blitter's private business. The `TODO: Real alpha-channel` this
    // carried belongs there too — it was never about anti-aliasing, but about
    // surface transparency, which is an offscreen-layer question.

    pub fn draw_pixels(
        &mut self,
        pixels: impl IntoIterator<Item = Pixel<C>>,
    ) -> Result<(), ()> {
        let viewport = self.current_viewport();
        let canvas = &mut self.canvas;
        let eg_pixels = pixels
            .into_iter()
            .map(|p| embedded_graphics::prelude::Pixel(p.0.into(), p.1));
        match viewport {
            ViewportKind::Fullscreen => canvas.draw_iter(eg_pixels),
            ViewportKind::Clipped(area) => {
                canvas.clipped(&area.into()).draw_iter(eg_pixels)
            },
            ViewportKind::Cropped(area) => {
                canvas.cropped(&area.into()).draw_iter(eg_pixels)
            },
        }
        .unwrap();
        Ok(())
    }

    // Renderer common implementations
    /// WS6.4d: aim the surface at `region` and pre-fill it.
    ///
    /// Two things happen, and neither is conditional. The buffer is retargeted
    /// — `region`'s own width becomes the stride, so any region fitting the
    /// capacity is addressable — and then it is **filled with the background**.
    ///
    /// # Why this is unconditional, when it used to skip full-frame surfaces
    ///
    /// Re-striding is harmless in itself: after `retarget`, the buffer is a
    /// valid region-shaped surface and the units past `w · h` are simply
    /// unused. Nothing reads them, because the contract is that each region is
    /// flushed when it is painted.
    ///
    /// What a full-frame surface used to buy by *not* retargeting was not
    /// safety, it was the **fill**. A region is a merged damage rect, so it
    /// contains dead space no widget paints — the gap between two merged rects,
    /// container padding, the area under a transparent `Flex`. On a retained
    /// framebuffer those pixels were already correct from the previous frame, so
    /// nothing had to be written. Retarget and they alias unrelated parts of the
    /// buffer, so they must be cleared.
    ///
    /// That is a cost question, not a correctness one, and the cost is one
    /// background fill per region — which the planner's merge test **already
    /// prices**: it charges merging for repainting the dead space. Retention
    /// made that charge an over-estimate. So paying it makes the model exact and
    /// buys a uniform contract: `covers == region` for every surface, one rect
    /// instead of two, and no full-frame special case in `attach` — which is
    /// where the "flushed nothing from frame two onward" bug lived.
    ///
    /// The fill is also what makes anti-aliasing correct rather than merely tidy
    /// (roadmap 6.4 constraint (b)): `pixel_alpha` blends against the
    /// *destination*, so without it the first AA edge in a region would blend
    /// against whatever the previous region left there.
    ///
    /// # One dependency, recorded because it is currently unreachable
    ///
    /// The fill uses [`Color::default_background`], not the **page's**
    /// background — and `Page::clear` uses `PageStyle::background_color`. Those
    /// agree today only because `PageStyle`'s setter is commented out
    /// (`page/mod.rs`), so every page's background *is* `default_background`.
    /// Uncomment it and a themed page's gaps paint white on every path. The fix
    /// belongs with that setter: the background to prime a region with is the
    /// page's, so it has to reach the renderer.
    ///
    /// [`Color::default_background`]: crate::color::Color::default_background
    fn renderer_begin_region(&mut self, region: Rect) -> RenderResult {
        let canvas = &mut self.canvas;
        canvas.retarget(region);
        // Straight at the canvas, not through `Renderer::fill_solid`: the
        // region clip is pushed by the caller *after* this returns, and the
        // whole retargeted buffer is what needs priming.
        //
        // WS6.4e: the inherent fill rather than the `DrawTarget` one. Same
        // algorithm — the trait method delegates here — but this is the call a
        // non-embedded-graphics backend will make, so it is the one to write.
        Framebuf::fill_solid(canvas, region, C::default_background());
        Ok(())
    }

    // WS6.4b: narrowed by the active viewport so the top of the stack IS the
    // effective clip (`ViewportKind::nested_in` documents why that matters).
    // `EGRenderer` still keeps its viewport stack inline instead of using the
    // same one-line composition as the tiny-skia backend's; see the note on the
    // struct for why they are duplicated rather than shared.
    fn renderer_push_clip(&mut self, area: Rect) {
        let nested =
            ViewportKind::Clipped(area).nested_in(self.current_viewport());
        self.viewport_stack.push(nested);
    }

    // `Fullscreen` falls back to the SURFACE rect rather than reporting "no
    // bound": that is what makes WS6.4b's culling pay on an ordinary full-frame
    // render, not only under tiles — off-screen content (scrolled-away rows) is
    // exactly the case where the clip is currently a write filter and the paint
    // happens anyway.
    fn renderer_clip_bounds(&self) -> Option<Rect> {
        Some(
            self.current_viewport()
                .clip_bounds()
                .unwrap_or(Rect::new(Point::zero(), self.main_viewport)),
        )
    }

    // Never pops the root viewport: an unbalanced `pop_clip` must degrade, not
    // leave the renderer with no viewport at all (`current_viewport` unwraps).
    fn renderer_pop_clip(&mut self) {
        if self.viewport_stack.len() > 1 {
            self.viewport_stack.pop();
        }
    }

    fn renderer_image<'a>(&mut self, image: DrawImage<'a, C>) -> RenderResult {
        embedded_graphics::image::Image::new(
            image.image(),
            image.position().into(),
        )
        .draw(self)?;
        Ok(())
    }
}

impl<C: Color + PackedColor + PixelColor, B: FramebufStorage<C>, P: FramePolicy>
    DrawTarget for EGRenderer<C, B, P>
{
    type Color = C;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::prelude::Pixel<Self::Color>>,
    {
        self.draw_pixels(pixels.into_iter().map(|p| Pixel(p.0.into(), p.1)))
    }

    /// WS6.3b: route a solid rect fill to the framebuffer's fast `fill_solid`
    /// (whole-word writes) instead of the default fan-out to per-pixel
    /// `draw_iter`. Without this, `EGRenderer::fill_solid` → styled `Rectangle` →
    /// this DrawTarget's default `fill_solid` → `draw_iter`, and the framebuffer
    /// fast path is never reached. Mirrors `draw_pixels`' viewport dispatch; the
    /// eg `clipped`/`cropped` adapters forward `fill_solid` to the canvas (with
    /// clip / translation), so those paths stay correct and also get the speedup.
    ///
    /// WS6.4e: the unclipped arm is spelled `DrawTarget::fill_solid(canvas, ..)`
    /// rather than `canvas.fill_solid(..)` because `Framebuf` now has an
    /// *inherent* `fill_solid` too, and inherent methods win method resolution.
    /// Both are the same algorithm; only the argument types differ.
    fn fill_solid(
        &mut self,
        area: &embedded_graphics::primitives::Rectangle,
        color: Self::Color,
    ) -> Result<(), Self::Error> {
        let viewport = self.current_viewport();
        let canvas = &mut self.canvas;
        match viewport {
            ViewportKind::Fullscreen => {
                DrawTarget::fill_solid(canvas, area, color)
            },
            ViewportKind::Clipped(clip) => {
                canvas.clipped(&clip.into()).fill_solid(area, color)
            },
            ViewportKind::Cropped(crop) => {
                canvas.cropped(&crop.into()).fill_solid(area, color)
            },
        }
    }
}

impl<C: Color + PackedColor + PixelColor, B: FramebufStorage<C>, P: FramePolicy>
    Dimensions for EGRenderer<C, B, P>
{
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        embedded_graphics::primitives::Rectangle::new(
            embedded_graphics::geometry::Point::zero(),
            self.main_viewport.into(),
        )
    }
}

// NOTE (WS6.4d): `output` / `output_regions` lived here and were removed with
// the `RenderTarget` seam. `output_regions` in particular never made sense once
// the surface became a loan: a tile IS one region, so "flush these several
// regions at once" describes a full framebuffer being flushed under a damage
// list — the pre-tiling flow, not this one. The loop is one region at a time,
// and each iteration's buffer goes out on the caller's own transport.

impl<C: Color + PackedColor + PixelColor, B: FramebufStorage<C>, P: FramePolicy>
    Renderer for EGRenderer<C, B, P>
{
    type Color = C;

    /// Whatever policy this renderer was built with. It is the only thing rsact
    /// learns about the surface — and it learns it as a *type*, so the buffer
    /// itself never crosses into rsact-ui. The surface is checked against this
    /// where both are known: [`EGRenderer::attach`].
    type Policy = P;

    fn size(&self) -> Size {
        self.main_viewport
    }

    fn begin_region(&mut self, region: Rect) -> RenderResult {
        self.renderer_begin_region(region)
    }

    fn push_clip(&mut self, area: Rect) {
        self.renderer_push_clip(area)
    }

    fn pop_clip(&mut self) {
        self.renderer_pop_clip()
    }

    fn clip_bounds(&self) -> Option<Rect> {
        self.renderer_clip_bounds()
    }

    fn fill_solid(&mut self, rect: Rect, color: Self::Color) -> RenderResult {
        self.rect(
            rect,
            &DrawStyle {
                fill: Some(color),
                stroke: None,
                stroke_width: 0,
                stroke_alignment: StrokeAlignment::Inside,
            },
        )
    }

    fn pixel(&mut self, point: Point, color: Self::Color) -> RenderResult {
        embedded_graphics::Pixel(point.into(), color).draw(self)
    }

    fn line(
        &mut self,
        from: Point,
        to: Point,
        style: &DrawStyle<C>,
    ) -> RenderResult {
        primitives::line::draw(self, &Line::new(from, to), style)
    }

    fn rect(
        &mut self,
        rect: Rect,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let eg_rect: embedded_graphics::primitives::Rectangle = rect.into();
        eg_rect
            .draw_styled(&style.into_primitive_style(), self)
            .ok()
            .unwrap();
        Ok(())
    }

    fn rounded_rect(
        &mut self,
        rect: Rect,
        corners: CornerRadii,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        primitives::rounded_rect::draw(
            self,
            &RoundedRect::new(rect, corners),
            style,
        )
    }

    fn circle(
        &mut self,
        top_left: Point,
        diameter: u32,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        primitives::circle::draw(self, &Circle::new(top_left, diameter), style)
    }

    fn arc(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        primitives::arc::draw(
            self,
            &Arc::new(top_left, diameter, start, sweep),
            style,
        )
    }

    fn ellipse(
        &mut self,
        bounding_box: Rect,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        primitives::ellipse::draw(
            self,
            &Ellipse::new(bounding_box.top_left, bounding_box.size),
            style,
        )
    }

    fn sector(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        primitives::sector::draw(
            self,
            &Sector::new(top_left, diameter, start, sweep),
            style,
        )
    }

    fn polygon(
        &mut self,
        _points: &[Point],
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // TODO: I don't want to allocate a vector for conversion between my
        // Point and EG Point, so better use custom primitive Polygon.
        //
        // TODO(unimplemented): polygon rendering for the embedded-graphics
        // backend. Skip (logged) instead of `todo!()` so drawing a polygon
        // degrades to nothing rather than aborting the device.
        //
        // The body already exists and is already unreachable — see
        // `eg::primitives::polygon::draw`, which needs a `Renderer` rather
        // than a `DrawTarget` and so cannot be called from here without
        // recursing. The layer split's `raster::polygon` is what gives it a
        // caller: `Rasterizer::polygon`'s default draws, and no primitive is
        // ever unsupported (D3).
        log::warn!(
            "polygon() is not implemented for the embedded-graphics renderer; \
             skipping"
        );
        Ok(())
    }

    fn path(
        &mut self,
        path: &Path,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        let mut current_pos = Point::zero();
        for segment in path.segments() {
            match segment {
                PathSegment::MoveTo(p) => {
                    current_pos = *p;
                },
                PathSegment::LineTo(p) => {
                    self.line(current_pos, *p, style)?;
                    current_pos = *p;
                },
                PathSegment::ArcTo { center: _, radius, start, sweep } => {
                    let diameter = radius * 2;
                    let top_left = Point::new(
                        current_pos.x - *radius as i32,
                        current_pos.y - *radius as i32,
                    );
                    self.arc(top_left, diameter, *start, *sweep, style)?;
                },
                PathSegment::Close => {},
            }
        }
        Ok(())
    }

    fn image<'a>(&mut self, image: DrawImage<'a, Self::Color>) -> RenderResult {
        self.renderer_image(image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        geometry::{Point, Rect, Size},
        renderer::{NullColor, NullRenderer, Renderer},
    };
    use embedded_graphics::pixelcolor::Rgb888;

    /// Host-side test surfaces. **Deliberately local to the tests**: rsact-render
    /// exports no allocating helper, because the library must never choose where
    /// a framebuffer lives — an embedded app puts it in SDRAM, DTCM or a
    /// `#[link_section]` pool, and a blessed `heap_surface()` would both presume
    /// a global allocator and make the wrong thing the obvious one.
    /// A `&'static mut` loan — the shape both `FramebufStorage` impls describe, and
    /// the only one a renderer reachable through `WidgetCtx` (`: 'static`) can
    /// hold. `Vec::leak` in a test is a `StaticCell` on a device.
    fn surface_units<C: Color + PackedColor>(
        units: usize,
    ) -> &'static mut [<C as PackedColor>::Storage] {
        alloc::vec![C::default_background().into_storage(); units].leak()
    }

    fn surface<C: Color + PackedColor>(
        size: Size,
    ) -> &'static mut [<C as PackedColor>::Storage] {
        surface_units::<C>(size.area() as usize / C::pps())
    }

    /// WS6.3b: EGRenderer's fast `fill_solid` (routing to the framebuffer's
    /// whole-word writes) must land the SAME pixels as the per-pixel path —
    /// proving the DrawTarget override + viewport dispatch forward correctly,
    /// not just the framebuffer method in isolation.
    #[test]
    fn eg_renderer_fill_solid_matches_per_pixel() {
        let size = Size::new(20, 16);
        let rect = Rect::new(Point::new(3, 2), Size::new(9, 7));
        let color = Rgb888::new(10, 200, 30);

        let mut fast =
            EGRenderer::<Rgb888, _>::new(size, surface::<Rgb888>(size));
        Renderer::fill_solid(&mut fast, rect, color).unwrap();

        // Reference: fill the same rect one pixel at a time (the draw_iter path).
        let mut slow =
            EGRenderer::<Rgb888, _>::new(size, surface::<Rgb888>(size));
        for p in rect.points() {
            Renderer::pixel(&mut slow, p, color).unwrap();
        }

        fast.draw_buffer(|f| {
            slow.draw_buffer(|s| {
                assert_eq!(f, s, "EGRenderer fill_solid != per-pixel fill");
            })
        });
    }

    /// **WS6.4d: the acceptance test for tiled rendering.** A surface a
    /// fraction of the frame's size must produce the *same pixels* as a full
    /// framebuffer.
    ///
    /// This is what 6.4 exists for, stated as an equality: 240×240 RGB565 is
    /// 112.5 KiB and does not fit a Blue Pill, while a 240×24 tile is 11.25 KiB
    /// and does. The whole design is only worth anything if the two agree
    /// exactly, and "agree" has to mean pixels — WS6.4a's op-log invariance
    /// checks that each region *issues* the right draw calls, which is a
    /// different claim and cannot see an addressing mistake. Getting the origin
    /// sign wrong, or the stride, produces a plausible image and an intact op
    /// log.
    ///
    /// Both paths are read the way a **transport** reads them (WS6.4d): raw
    /// units plus the rect they belong at, walked row by row at the region's own
    /// width. Deliberately not through any rsact-provided flush — using rsact's
    /// addressing to read back what rsact's addressing wrote would prove
    /// nothing, and there is no such flush any more in any case.
    #[test]
    fn a_tiled_surface_paints_the_same_pixels_as_a_full_one() {
        use crate::{region::Tiles, style::DrawStyle};
        use alloc::vec;

        const W: u32 = 64;
        const H: u32 = 64;
        let viewport = Size::new(W, H);

        /// A full-frame pixel map, so the two paths are compared on what the
        /// display would actually receive.
        struct Map {
            px: alloc::vec::Vec<Option<Rgb888>>,
        }

        /// Blit raw storage units onto the map at `region` — what an ST7789
        /// does after `CASET`/`RASET`, and the only way a caller ever reads a
        /// detached buffer. Rows are strided at the **region's own width**,
        /// which is the property a wrong origin or stride would break.
        fn blit(map: &mut Map, units: &[u32], region: Rect) {
            let w = region.size.width as usize;
            for row in 0..region.size.height as usize {
                for col in 0..w {
                    let x = region.top_left.x + col as i32;
                    let y = region.top_left.y + row as i32;
                    if x < 0 || y < 0 || x as u32 >= W || y as u32 >= H {
                        continue;
                    }
                    map.px[y as usize * W as usize + x as usize] =
                        Some(<Rgb888 as PackedColor>::as_color(
                            &units[row * w + col],
                            0,
                        ));
                }
            }
        }
        let blank = || Map { px: vec![None; (W * H) as usize] };

        // Content chosen to cross region boundaries and to exercise both write
        // paths: `fill_solid`'s whole-word runs and the per-pixel fan-out.
        fn content<R: Renderer<Color = Rgb888>>(r: &mut R) {
            Renderer::fill_solid(
                r,
                Rect::new(Point::new(6, 10), Size::new(50, 30)),
                Rgb888::new(200, 30, 30),
            )
            .unwrap();
            Renderer::rect(
                r,
                Rect::new(Point::new(2, 2), Size::new(60, 60)),
                &DrawStyle::default()
                    .stroke(Rgb888::new(20, 220, 40))
                    .stroke_width(2),
            )
            .unwrap();
            Renderer::line(
                r,
                Point::new(0, 0),
                Point::new(63, 63),
                &DrawStyle::default()
                    .stroke(Rgb888::new(10, 40, 250))
                    .stroke_width(1),
            )
            .unwrap();
            for i in 0..40i32 {
                Renderer::pixel(
                    r,
                    Point::new(i, 63 - i),
                    Rgb888::new(250, 250, 10),
                )
                .unwrap();
            }
        }

        // Reference: one full-size surface, one pass, one flush.
        let mut full =
            EGRenderer::<Rgb888, _>::new(viewport, surface::<Rgb888>(viewport));
        content(&mut full);
        let mut full_map = blank();
        let (_, full_units, full_at) = full.detach();
        blit(&mut full_map, &full_units, full_at);

        // Tiled: a 64x8 surface — 512 units against the frame's 4096, an eighth
        // — repainted and flushed region by region.
        const TILE_UNITS: usize = (W * 8) as usize; // Rgb888: one unit per pixel
        let tile_units = TILE_UNITS;
        let mut tiled = EGRenderer::<Rgb888, _, Tiles<W, 8>>::tiled(
            viewport,
            surface_units::<Rgb888>(TILE_UNITS),
        );
        assert!(
            tile_units * 8 == (W * H) as usize,
            "the point of the test is that the surface is a FRACTION of the frame"
        );

        let mut tiled_map = blank();
        let mut spare = surface_units::<Rgb888>(TILE_UNITS);
        for band in 0..8 {
            let region = Rect::new(Point::new(0, band * 8), Size::new(W, 8));
            tiled.begin_region(region).unwrap();
            tiled.push_clip(region);
            content(&mut tiled);
            tiled.pop_clip();
            tiled.end_region().unwrap();
            // Publish, then acquire — the ordering the loan API exists for,
            // and the one a single-buffer pool needs.
            let (parked, units, at) = tiled.detach();
            blit(&mut tiled_map, &units, at);
            tiled = parked.attach(spare);
            spare = units;
        }

        // Not vacuous: both paths must have painted a substantial frame. An
        // all-`None` comparison passes trivially, and this test's whole value
        // is that it would catch an addressing bug — which is also the kind of
        // bug that could leave a map empty.
        let painted = |m: &Map| m.px.iter().filter(|p| p.is_some()).count();
        assert!(
            painted(&full_map) > (W * H) as usize / 3,
            "the reference frame painted only {} of {} pixels",
            painted(&full_map),
            W * H
        );
        assert_eq!(
            painted(&full_map),
            painted(&tiled_map),
            "the two paths painted different numbers of pixels"
        );

        let mismatches: alloc::vec::Vec<usize> = (0..(W * H) as usize)
            .filter(|&i| full_map.px[i] != tiled_map.px[i])
            .collect();
        assert!(
            mismatches.is_empty(),
            "{} of {} pixels differ between a full framebuffer and a tiled \
             one; first at ({}, {}): full {:?} vs tiled {:?}",
            mismatches.len(),
            W * H,
            mismatches[0] % W as usize,
            mismatches[0] / W as usize,
            full_map.px[mismatches[0]],
            tiled_map.px[mismatches[0]],
        );
    }

    /// WS6.4d: the renderer **borrows** its surface — it never allocates one and
    /// always gives it back.
    ///
    /// This is the ownership rule stated as a test. On a device the buffer lives
    /// in the application's `StaticCell` pool and moves through channels; rsact
    /// holds it only for as long as it is painting. `WidgetCtx: 'static`
    /// (`el/ctx.rs:5`) rules out expressing that as a `&'a mut [T]` field, so
    /// the loan is a move in and a move out — which is also exactly the shape
    /// DMA wants, since a borrow the core could still write through is UB
    /// (roadmap 6.7).
    #[test]
    fn the_renderer_gives_the_surface_back() {
        let viewport = Size::new(16, 16);
        let mut r =
            EGRenderer::<Rgb888, _>::new(viewport, surface::<Rgb888>(viewport));

        // Paint something, then take the buffer back and inspect it — the owner
        // can read what was painted, which is what "ship this tile" means.
        let ink = Rgb888::new(9, 9, 9);
        Renderer::fill_solid(&mut r, Rect::new(Point::zero(), viewport), ink)
            .unwrap();
        let (parked, buffer, dirty) = r.detach();
        assert_eq!(buffer.len(), (16 * 16) as usize);
        assert_eq!(
            dirty,
            Rect::new(Point::zero(), viewport),
            "a full-frame surface reports the whole frame as its dirty region"
        );
        assert!(
            buffer.iter().all(|u| *u == ink.into_storage()),
            "the owner got back a buffer that does not hold what was painted"
        );

        // `parked` has no drawing methods AT ALL — painting between a detach
        // and the next attach is not a logged no-op any more, it does not
        // compile. That is the invariant the type-state removed; there is
        // nothing to assert here because there is nothing to call.
        let mut r = parked.attach(surface::<Rgb888>(viewport));

        // A swap keeps `&mut self`, because the renderer is never observably
        // without a surface — and hands back the one that was in place.
        let (swapped, at) = r.swap(surface::<Rgb888>(viewport));
        assert_eq!(swapped.len(), (16 * 16) as usize);
        assert_eq!(at, Rect::new(Point::zero(), viewport));
    }

    /// WS6.4d: after `begin_region`, the rect a buffer covers **is** the region
    /// it was asked to paint — for every surface, not only for tiles.
    ///
    /// This is what lets a caller carry one rectangle instead of two. It held
    /// only for tiles until `begin_region` stopped exempting full-frame
    /// surfaces from retargeting, and the asymmetry was invisible in every test
    /// because each used one surface kind at a time.
    #[test]
    fn what_a_buffer_covers_is_the_region_it_painted() {
        use crate::region::Tiles;

        let viewport = Size::new(64, 64);
        let region = Rect::new(Point::new(8, 24), Size::new(16, 8));

        // A surface spanning the whole frame — the case that used to differ.
        let mut full =
            EGRenderer::<Rgb888, _>::new(viewport, surface::<Rgb888>(viewport));
        full.begin_region(region).unwrap();
        let (_, _, covers) = full.detach();
        assert_eq!(
            covers, region,
            "a frame-sized buffer must report the region, not the frame"
        );

        // And a tile, where it always held.
        let mut tile = EGRenderer::<Rgb888, _, Tiles<16, 8>>::tiled(
            viewport,
            surface_units::<Rgb888>(16 * 8),
        );
        tile.begin_region(region).unwrap();
        let (_, _, covers) = tile.detach();
        assert_eq!(covers, region);
    }

    /// WS6.4d: a region's units are laid out at **its own width**, so a region
    /// narrower than the frame is contiguous rows of `region.width`.
    ///
    /// This is the contract every caller's blit depends on, and it is asserted
    /// here because a differential test cannot reach it: chunking preserves
    /// width, so tiling a full frame yields full-width bands where "region
    /// width" and "frame width" are the same number, and a narrower region comes
    /// from damage that is identical whatever the policy — so a caller's helper
    /// using the wrong one is wrong the same way on both sides of any
    /// whole-versus-tiled comparison and the difference cancels.
    ///
    /// Verified by mutation, both ways: pinning `row_stride` to the frame width
    /// leaves the `rsact-ui` harness green and fails this — as an out-of-bounds
    /// write while painting, which is why the region is deliberately narrow AND
    /// tall. A stride error that stayed in bounds would be caught by the
    /// per-unit assertion below instead.
    #[test]
    fn a_narrow_region_is_laid_out_at_its_own_width() {
        use crate::region::Tiles;

        let viewport = Size::new(64, 64);
        // Deliberately narrow AND tall, so a frame-width stride would run off
        // the end of the region's data instead of merely landing askew.
        let region = Rect::new(Point::new(40, 8), Size::new(5, 9));

        let mut r = EGRenderer::<Rgb888, _, Tiles<8, 16>>::tiled(
            viewport,
            surface_units::<Rgb888>(8 * 16),
        );
        r.begin_region(region).unwrap();

        // One distinguishable color per pixel of the region, in absolute
        // coordinates — `pixel` is the path that resolves them against the
        // buffer's origin.
        let color_at = |x: i32, y: i32| {
            Rgb888::new(
                (x as u8).wrapping_mul(7),
                (y as u8).wrapping_mul(11),
                3,
            )
        };
        for p in region.points() {
            Renderer::pixel(&mut r, p, color_at(p.x, p.y)).unwrap();
        }

        let (_, units, at) = r.detach();
        assert_eq!(at, region);

        let stride = region.size.width as usize;
        for row in 0..region.size.height as usize {
            for col in 0..stride {
                let want = color_at(
                    region.top_left.x + col as i32,
                    region.top_left.y + row as i32,
                );
                let got = <Rgb888 as PackedColor>::as_color(
                    &units[row * stride + col],
                    0,
                );
                assert_eq!(
                    got, want,
                    "unit at row {row}, col {col} of a {}x{} region — rows must \
                     be strided at the REGION's width, not the frame's",
                    region.size.width, region.size.height,
                );
            }
        }
    }

    /// WS6.4d bug fix: a full-frame renderer that detaches and reattaches must
    /// come back aimed at the **whole frame**, not at nothing.
    ///
    /// `attach` used to wrap every buffer as a tile (viewport `Rect::zero()`),
    /// and `begin_region` returns early for a full-frame surface — so nothing
    /// ever re-aimed it. The ordinary flush loop (render, detach, ship,
    /// reattach) therefore reported a zero-sized dirty rect from the second
    /// frame onward and flushed nothing at all. Silent, and invisible to the
    /// op-log checks.
    #[test]
    fn reattaching_a_full_frame_surface_keeps_aiming_at_the_frame() {
        let viewport = Size::new(16, 16);
        let r =
            EGRenderer::<Rgb888, _>::new(viewport, surface::<Rgb888>(viewport));
        let full = Rect::new(Point::zero(), viewport);

        let (parked, buffer, first) = r.detach();
        assert_eq!(first, full);

        let r = parked.attach(buffer);
        let (_, _, second) = r.detach();
        assert_eq!(
            second, full,
            "a reattached full-frame surface must still cover the frame; a \
             zero rect here means every flush after the first sends nothing"
        );
    }

    /// WS6.4d: a renderer **declares** the largest region it will accept, and
    /// its surface is checked against that declaration — not the other way
    /// round.
    ///
    /// This closes a hole that went through two shapes. First, `Renderer::
    /// `SURFACE_UNITS` defaulted to `usize::MAX` — "my surface always covers the
    /// frame" — which was simply true while every `EGRenderer` was a full
    /// framebuffer, and became a lie the moment a 5760-unit surface could exist.
    /// Second, `PixelBuf for Box<[S]>` claimed that same `usize::MAX`, which
    /// made **every heap surface** exempt: an *empty* boxed slice satisfied a
    /// full-frame policy at compile time and constructed a tiled renderer over
    /// zero bytes.
    ///
    /// Both are gone. Capacity a type cannot state is `None` — "ask the value" —
    /// and the comparison lives in [`EGRenderer::attach`], where the policy and
    /// the buffer are both in hand.
    #[test]
    fn a_renderer_declares_the_regions_it_accepts() {
        use crate::region::{Tiles, Unbounded};

        // A full-frame renderer accepts anything, which is honest: it does
        // cover the frame.
        assert_eq!(
            <<EGRenderer<
                Rgb888,
                &'static mut [<Rgb888 as PackedColor>::Storage],
            > as Renderer>::Policy as FramePolicy>::MAX_REGION,
            None
        );

        // A tiled one reports its policy's region, and the planner converts
        // that to a unit budget with the color's own packing. Leaving the
        // packing at 1 would over-state a 1-bpp surface eightfold — a policy
        // needing 384 bytes would "fit" a 48-byte buffer.
        type Tiled<C> = EGRenderer<
            C,
            &'static mut [<C as PackedColor>::Storage],
            Tiles<240, 24>,
        >;
        assert_eq!(
            <<Tiled<Rgb888> as Renderer>::Policy as FramePolicy>::MAX_REGION,
            Some(Size::new(240, 24))
        );
        assert_eq!(policy_units::<Tiles<240, 24>>(), Some(5760));
        assert_eq!(policy_units::<Tiles<240, 12>>(), Some(2880));
        assert_eq!(policy_units::<Unbounded>(), None);

        // The hole, closed: a heap surface no longer claims infinite capacity,
        // so it cannot silently satisfy a policy it does not fit.
        assert_eq!(
            <&mut [u32] as FramebufStorage<Rgb888>>::UNITS,
            None,
            "a runtime-sized surface must not state a compile-time capacity"
        );
        assert_eq!(
            <&mut [u32; 5760] as FramebufStorage<Rgb888>>::UNITS,
            Some(5760)
        );
    }

    /// The other half of that: a surface too small for the policy is refused at
    /// the hand-off, before anything paints into it.
    #[test]
    #[should_panic(expected = "too small for this renderer's frame policy")]
    fn a_surface_too_small_for_the_policy_is_refused() {
        use crate::region::Tiles;
        // 240x24 RGB888 needs 5760 units; this holds one row.
        let _ = EGRenderer::<Rgb888, _, Tiles<240, 24>>::tiled(
            Size::new_equal(240),
            surface_units::<Rgb888>(240),
        );
    }

    /// WS6.4.0(ii-4): `NullRenderer` must be a no-op renderer for the
    /// *application's* color, not only for `NullColor`.
    ///
    /// This is what 6.4c's collect pass runs widget bodies against: it has to
    /// satisfy `Renderer<Color = W::Color>` while rasterising nothing, which the
    /// old `type Color = NullColor` hard-wiring could not express. Lives in this
    /// module because a second real `Color` impl (`Rgb888`) is in scope here.
    #[test]
    fn null_renderer_is_generic_over_color() {
        fn accepts_renderer_for<C: Color, R: Renderer<Color = C>>(
            r: &mut R,
            c: C,
        ) {
            // Every primitive is a no-op that still reports success, so a
            // collect pass never sees a spurious `Err` from the null backend.
            Renderer::pixel(r, Point::zero(), c).unwrap();
            r.push_clip(Rect::new(Point::zero(), Size::new_equal(4)));
            r.pop_clip();
            // Defaulted in ii-3; correct as a no-op for a full-frame surface.
            r.begin_region(Rect::new(Point::zero(), Size::new_equal(4)))
                .unwrap();
            r.end_region().unwrap();
        }

        let mut app = NullRenderer::<Rgb888>::default();
        accepts_renderer_for(&mut app, Rgb888::WHITE);

        // The `C = NullColor` default keeps every existing `Wtf<NullRenderer, ..>`
        // and `&mut NullRenderer` spelling compiling unannotated.
        let mut legacy = NullRenderer::default();
        accepts_renderer_for(&mut legacy, NullColor);
    }

    /// WS6.4.0(ii-1): the clip stack must balance, and an unmatched `pop_clip`
    /// must degrade rather than pop the root viewport — `current_viewport()`
    /// unwraps the top of the stack, so emptying it would turn a caller's
    /// bookkeeping slip into a panic on the render path (WS1.8: the UI logs and
    /// degrades, it does not abort).
    #[test]
    fn clip_stack_balances_and_never_pops_the_root() {
        let mut r = EGRenderer::<Rgb888, _>::new(
            Size::new(20, 16),
            surface::<Rgb888>(Size::new(20, 16)),
        );
        let root = r.viewport_stack.len();
        assert_eq!(root, 1, "a fresh renderer holds exactly the root viewport");

        r.push_clip(Rect::new(Point::new(2, 2), Size::new(8, 8)));
        assert_eq!(r.viewport_stack.len(), root + 1);
        r.push_clip(Rect::new(Point::new(3, 3), Size::new(4, 4)));
        assert_eq!(r.viewport_stack.len(), root + 2);

        r.pop_clip();
        r.pop_clip();
        assert_eq!(r.viewport_stack.len(), root, "push/pop must balance");

        // Unmatched pop: no panic, no lost root.
        r.pop_clip();
        r.pop_clip();
        assert_eq!(r.viewport_stack.len(), root, "root viewport must survive");
        // Still usable afterwards — the real point of not emptying the stack.
        Renderer::pixel(&mut r, Point::new(1, 1), Rgb888::WHITE).unwrap();
    }

    /// WS6.4b(ii): the proxy must drop pixels outside the renderer's clip before
    /// paying `Renderer::pixel` for them, and must drop **only** those.
    ///
    /// This is the path all text takes (`embedded-text` / u8g2 hand glyphs over
    /// one pixel at a time), so it is where the per-pixel cost that survives the
    /// part-level cull lives — a label straddling a region boundary is culled in
    /// neither region. Asserted on op counts because the failure modes are
    /// symmetric and both silent: filter too little and tiling pays N× per-pixel
    /// work; filter too much and glyphs lose columns.
    #[test]
    fn the_proxy_filters_pixels_the_renderer_would_reject() {
        use crate::record::{DrawOp, RecordingRenderer};

        let mut rec = RecordingRenderer::<Rgb888>::new(Size::new(64, 64));
        rec.push_clip(Rect::new(Point::new(10, 10), Size::new(10, 10)));

        let ink = Rgb888::new(255, 255, 255);
        let at = |x, y| {
            embedded_graphics::prelude::Pixel(
                embedded_graphics::prelude::Point::new(x, y),
                ink,
            )
        };
        DrawTargetProxy::new(&mut rec)
            .draw_iter([
                at(15, 15), // inside
                at(19, 19), // inside, last pixel of the clip
                at(20, 20), // outside: the clip's edge is exclusive
                at(5, 5),   // outside
            ])
            .unwrap();

        let drawn: Vec<_> = rec
            .ops()
            .into_iter()
            .filter_map(|op| match op {
                DrawOp::Pixel(point) => Some(point),
                _ => None,
            })
            .collect();
        assert_eq!(drawn, [Point::new(15, 15), Point::new(19, 19)]);
    }

    /// WS6.4b: a nested clip must be **narrowed by** its parent, not replace it.
    ///
    /// Asserted where it actually matters — on the framebuffer, not on the stack:
    /// a pixel inside the inner clip but outside the outer one must not land. It
    /// used to, because `push_clip` stored the raw area and the write filter
    /// consults only the top of the stack, so an inner clip reaching beyond its
    /// parent *widened* the effective clip. Under WS6.4d that is drawing escaping
    /// its tile; here it is the precondition for `clip_bounds` being an exact cull
    /// rect rather than a guess.
    #[test]
    fn a_nested_clip_narrows_and_never_widens() {
        let mut r = EGRenderer::<Rgb888, _>::new(
            Size::new(40, 40),
            surface::<Rgb888>(Size::new(40, 40)),
        );

        r.push_clip(Rect::new(Point::new(0, 0), Size::new(20, 20)));
        // Overlaps the parent over (10,10)..(20,20) and reaches BEYOND it.
        r.push_clip(Rect::new(Point::new(10, 10), Size::new(20, 20)));

        assert_eq!(
            r.clip_bounds(),
            Some(Rect::new(Point::new(10, 10), Size::new(10, 10))),
            "the effective clip is the intersection, not the inner rect"
        );

        // The probe color must DIFFER from the untouched framebuffer, or the
        // assertions below hold whatever the clip does: `default_background()`
        // for RGB is WHITE, so a white probe pixel proves nothing (this test was
        // written that way first and passed its "rejected" case vacuously).
        let bg = <Rgb888 as Color>::default_background();
        let ink = <Rgb888 as Color>::default_foreground();
        assert_ne!(ink, bg, "the probe color must be visible");

        // Inside the inner rect but outside the parent: must be rejected.
        Renderer::pixel(&mut r, Point::new(25, 15), ink).unwrap();
        // Inside both: must land.
        Renderer::pixel(&mut r, Point::new(15, 15), ink).unwrap();

        assert_eq!(
            r.canvas.pixel(Point::new(25, 15)),
            Some(bg),
            "a write outside the PARENT clip escaped the nested clip"
        );
        assert_eq!(r.canvas.pixel(Point::new(15, 15)), Some(ink));

        // Popping restores the parent, not the raw inner rect.
        r.pop_clip();
        assert_eq!(
            r.clip_bounds(),
            Some(Rect::new(Point::new(0, 0), Size::new(20, 20)))
        );
        r.pop_clip();
        assert_eq!(
            r.clip_bounds(),
            Some(Rect::new(Point::zero(), Size::new(40, 40))),
            "the root viewport reports the surface rect"
        );
    }
}
