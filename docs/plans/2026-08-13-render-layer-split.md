# Render layer split — architecture plan

**Status: in progress on `ws6.4e-framebuf-unbind`.**
WS6.4e ✓ · PR A ✓ · PR B ✓ · PR C ✓ · PR D ✓ · attachment rework ✓ · ownership + fallibility review ✓ — **complete**

`EGRenderer` fuses three jobs — coordinating clips and regions, running
rasterization algorithms, and owning pixel storage. This splits them:

```
Widget ──▶ Renderer ─────────▶ Rasterizer ──────────▶ Blitter
           (L1, trait)         (L2, geometry→spans)   (L3, spans→pixels)

           clip stack          clip = span bound      owns storage + region
           region cursor       AA coverage            fill_span / blend_span
           culling             glyph loop (WS15)      addressing, rotation
           absolute coords ───────────────────────▶   rebases via bounds()
```

| | region | clip | the loan |
|---|---|---|---|
| L1 `Renderer` | resets stack, forwards | **stack + composition** | maps `detach`/`attach` |
| L2 `Rasterizer` | never learns it exists | span bound (via `RasterCtx`) | — |
| L3 `Blitter` | **windowing** | (`RasterCtx` guarantees it) | **holds it** |

Neither clip nor region crosses all three layers. That is what makes a **GPU**
expressible as L1 alone: it fuses rasterizer and blitter *in hardware*, and so
fuses clip and region back into scissor-on-an-attachment — precisely the pair
this stack splits. It is the only such case. `RecordingRenderer` and
`NullRenderer` are L1 for a different reason: a recorder logs *primitives*, which
do not exist below L1.

**L2/L3 is a crate-internal seam.** `RasterCtx::new` is `pub(crate)`, so a
downstream crate can write a `Blitter` but cannot drive a `Rasterizer`. `Renderer`
is the published backend seam, so the extension promise binds once, at L1.

Three concrete renderers after the split:

```rust
RasterRenderer<EgRasterizer,       FramebufBlitter<Rgb565, &'static mut [u16]>, Tiles<240, 24>>
RasterRenderer<TinySkiaRasterizer, PixmapBlitter,                              Unbounded>
RasterRenderer<RsactRasterizer,    …>   // planned
```

`EGRenderer` and `TinySkiaRenderer` cease to exist.

---

## Planned, not built

Each must be *expressible* without rebuilding the architecture. For each: what
reserves it, and the constraint its implementation must respect — the constraints
are the part worth keeping, because they are what stops the feature being
designed twice.

| Item | Reserved by | Constraint |
|---|---|---|
| **`RsactRasterizer`** (rsact's own, AA + blending) | the `Rasterizer` trait; `RasterCtx::blend`; rasterizer *receives* the blitter (an AA-into-scratch path needs two blitters live in one call) | On a non-blending target, thresholding coverage at 128 leaves a **gapped** hairline — both pixels of each 45° step fall below it. Aliasing is an algorithm choice, so the rasterizer must be able to ask whether blending is real. The query is deliberately not designed yet; `blend_span`'s degrading default is not a substitute for it |
| **`DirectBlitter`** (straight to panel, no storage) | `Blitter`'s vocabulary contains no storage — `bounds()`, `fill_span()`, `begin_region()` (the address window); `capacity()` returns `Option` so "no bound" is expressible | **(a)** A paged mono panel cannot be driven this way: one SSD1306 GDDRAM byte is 8 vertically stacked pixels, a span is one row tall, and 4-wire SPI is write-only — so page-packed targets need a `Framebuf`. **(b)** The dominant cost is per-span window setup (`CASET`/`RASET`/`RAMWR`, measured F ≈ 10–30 µs), not overdraw, so it wants scanline-ordered emission or a 1-row tile. **(c)** Priming makes the overdraw floor ≥2× region area. Supersedes `output/mod.rs:20-22`, which records a direct renderer taking a `DrawTarget` as its own L1 parameter — write the supersession into that note |
| **DMA2D / hardware fill** | `fill_rect` is a `Blitter` method; a blitter can wrap another | Not a decorator over `T: Blitter` — a register fill needs `OMAR`/`OOR`/`OPFCCR` and the trait exposes no base pointer. It wraps the **concrete** `FramebufBlitter`. Second: DMA2D is asynchronous behind the D-cache on F7/H7, so a hardware `fill_rect` followed by a CPU `blend_span` on an overlapping span races — the fence cannot wait for `detach` |
| **GPU backend** | `Renderer` stays a trait | — |
| **Display rotation** (WS6.8) | addressing lives *inside* the blitter; `local`/`pixel_index`/`span_range` are shared **helpers**, not trait methods | `bounds()` stays in unrotated absolute space, so L1's clips and L2's spans never learn about rotation. A provided trait method would hard-code `y * width + x` and foreclose both rotation and page packing |
| **Packed policies / e-paper** (WS6.5) | Appendix A + `align_region` | 12-bpp RGB444 (2 px per 3 bytes) and multi-plane e-paper (two independent 1-bpp RAM planes) are **out of scope** — state it, or `Packing` is reopened by the next panel |
| **Offscreen layers** | a blitter with its own `bounds()` | This, not a clip variant, is where coordinate rebasing returns |

---

## The sketch

Bodies are omitted only where they would be rasterization algorithms or storage
arithmetic.

```rust
//! Three-layer render split. NOT COMPILED — a design sketch.
//!
//! Existing types assumed: `Color`, `Point`, `Size`, `Rect`, `Angle`, `Path`,
//! `CornerRadii`, `DrawStyle<C>` (`fill`, `stroke`, `stroke_width`,
//! `stroke_alignment`), `DrawImage<'_, C>`, `RenderResult`, `PackedColor`,
//! `Framebuf<C, B>` / `FramebufStorage<C>` (WS6.4e's renames), `FramePolicy`,
//! `Unbounded`, `Tiles<W, H>`, `Attachment<S>` / `Attached` / `Detached`.
//!
//! Spellings that differ from the obvious guess: `Angle::FULL_CIRCLE` (not
//! `Angle::full()`), `Size::area()` (there is no `Rect::area()`), and `Vec` —
//! neither `tinyvec` nor `heapless` is a dependency.

use core::ops::Range;

// ===========================================================================
// Shared vocabulary
// ===========================================================================

/// One horizontal run of pixels. **Absolute** coordinates.
///
/// `{y, x, w}` rather than `{y, x: Range}` because `Range` is not `Copy` and
/// every defaulted method reads the span twice.
///
/// **Spans never wrap a row.** The one case where wrapping would win — a rect
/// spanning the blitter's full width — is `fill_rect`, which the blitter
/// coalesces using its own stride. A wrapping span would be the rasterizer
/// asserting it knows that stride, which is L3's private fact and wrong the
/// moment a region is retargeted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Span {
    pub y: i32,
    pub x: i32,
    pub w: u32,
}

impl Span {
    pub const fn new(y: i32, x: i32, w: u32) -> Self;
    pub const fn x_range(&self) -> Range<i32>;
    pub const fn len(&self) -> usize;
    pub const fn is_empty(&self) -> bool;

    /// Clip to `rect`, returning the surviving span **and the offset into any
    /// per-pixel data indexed from the original start**.
    ///
    /// That offset is why this is a method rather than three inline copies:
    /// forgetting it shifts a coverage array by a few pixels, which yields a
    /// plausible image rather than a failure.
    pub fn clip_to(&self, rect: &Rect) -> Option<(Span, usize)>;
}

/// Absolute → blitter-local addressing. **Helpers, not a contract**: a plain
/// row-major framebuf uses them, a rotating or page-packed blitter maps its own
/// way. Free functions rather than provided trait methods, so that overriding
/// is not a special case.
pub const fn local(bounds: &Rect, p: Point) -> Point;
pub const fn pixel_index(bounds: &Rect, p: Point) -> usize;
pub const fn span_range(bounds: &Rect, span: Span) -> Range<usize>;

// ===========================================================================
// Layer 3 — Blitter: where pixels physically land
// ===========================================================================

/// Accepts already-clipped, already-rasterized pixel work. Named in the
/// Skia/AGG sense. It is **not** the buffer itself — that is a
/// `FramebufStorage`, which a `FramebufBlitter` borrows.
///
/// The single required *drawing* method is a **row run**, because that is what
/// a packed framebuffer does best (`slice::fill` inside one row) and what a
/// display window does best (one SPI burst). This inverts today's
/// `draw_iter`-required arrangement, which forces every fill algorithm to
/// destructure output it already had in span form. WS6.3b is the precedent: it
/// already overrides `fill_solid` at framebuf and renderer level for exactly
/// this reason.
///
/// **Associated consts, and dyn-compatibility spent to get them (E0038).** An
/// earlier draft had none, to keep `dyn Blitter<Color = C>` available "in case
/// the rasterizer × blitter cross-product needs collapsing". Maintainer's
/// correction: an application uses **one** renderer + rasterizer + blitter
/// combination, so there is no cross-product — only the inlining a monomorphized
/// call gets, which is what an embedded target actually wants. The consts buy a
/// compile-time capacity proof; the erasure bought nothing anyone was going to
/// spend.
pub trait Blitter {
    type Color: Color;

    /// Units the **type** guarantees, or `None` when only the value knows.
    /// `Some(n)` for a fixed-size array, which is what makes a too-small target
    /// a **compile error**. Same "never unbounded" rule as
    /// `FramebufStorage::UNITS`.
    const UNITS: Option<usize> = None;

    /// Pixels per unit — `8` for 1-bpp, `1` otherwise. A const because the check
    /// it feeds is a compile-time one, for every blitter: comparing a policy's
    /// budget against a capacity is meaningless unless they count the same
    /// thing, and both sides are consts.
    const PIXELS_PER_UNIT: usize = 1;

    /// The absolute rect it currently accepts writes for.
    /// After `begin_region(r)`, this is `r`.
    fn bounds(&self) -> Rect;

    /// Units it can hold, or `None` for no storage bound at all
    /// (`DirectBlitter`, a GPU attachment). Two states, because at the *value*
    /// level "the type cannot say" does not arise — that case belongs to the
    /// compile-time proof, which reads the storage type directly.
    ///
    /// **A unit is one `C::Storage` element** — the same vocabulary
    /// `FramebufStorage::unit_count` and `region_units` already use, so the
    /// value drops straight into `assert_policy_fits`. For a colour that does
    /// not pack, one unit is one pixel, so `PixmapBlitter` reports
    /// `width * height`.
    fn capacity(&self) -> Option<usize>;

    /// Pixels this target packs into one unit of `capacity`. `1` for anything
    /// that does not pack, hence the default.
    ///
    /// It exists so the capacity proof can run **generically**: comparing a
    /// policy's budget against `capacity()` is meaningless unless the two count
    /// the same thing, and a method (not an associated const) keeps the trait
    /// dyn-compatible.
    fn pixels_per_unit(&self) -> usize { 1 }

    // ── required ───────────────────────────────────────────────────────────

    /// `span` is guaranteed inside `bounds()` by the caller (`RasterCtx`).
    fn fill_span(&mut self, span: Span, color: Self::Color);

    /// Aim at `region`: retarget the framebuf, open the panel's address window,
    /// bind an attachment — **and prime it**. Unconditional, including for a
    /// blitter already spanning the frame, so `bounds() == region` always and a
    /// caller carries one rect.
    ///
    /// **Required, not defaulted**: a no-op default would not retarget, and
    /// every addressing helper assumes it did.
    ///
    /// **Priming belongs here, atomically with the retarget.** A region is
    /// scratch with no history (WS6.4c(1)), so it must start at the true
    /// background or a tile flushes with holes; the region clip is pushed by L1
    /// *after* this returns, so the fill cannot go through the clipped path. The
    /// background is a colour-level fact (`C::default_background()`), which is
    /// why L3 can supply it without a theme.
    ///
    /// There is no `end_region`. Every implementation of it in the current
    /// codebase is a no-op, and the one real job it could have — a completion
    /// barrier for asynchronous writes — belongs where the loan goes back.
    fn begin_region(&mut self, region: Rect) -> RenderResult;

    // ── defaulted; override where the layout or hardware helps ─────────────

    /// Also the vertical-run case (`width == 1`): borders, separators,
    /// scrollbar tracks.
    ///
    /// **A vertically packed framebuf must override this.** The row default is
    /// correct there but ~8× pessimal: on page-packed storage a 1×8 run is one
    /// byte, and the default turns a 64 px separator into 64 read-modify-writes
    /// of the same 8 bytes.
    fn fill_rect(&mut self, rect: Rect, color: Self::Color) { /* rows */ }

    /// Distinct colours — images, gradients. `colors.len() == span.len()`,
    /// debug-asserted.
    fn fill_run(&mut self, span: Span, colors: &[Self::Color]) { /* per px */ }

    /// Anti-aliased run. `coverage.len() == span.len()`; 0 = untouched.
    ///
    /// Exercised from day one: `TinySkiaRasterizer` emits coverage from a
    /// `Mask`. The default thresholds at 128, which is a **last resort** and
    /// not a story for 1-bpp — see the `RsactRasterizer` row above.
    ///
    /// Deliberately no `read_pixel`: blending is the only reason to read, so
    /// the capability and its use stay in one method instead of two that can
    /// disagree.
    fn blend_span(&mut self, span: Span, color: Self::Color, coverage: &[u8]) {
        /* threshold at 128 */
    }

    /// Worth overriding: thin-stroke algorithms (Bresenham, Wu, circle outline)
    /// emit nothing but these, and a framebuffer's pixel write skips the range
    /// setup a length-1 span pays for.
    fn pixel(&mut self, p: Point, color: Self::Color) {
        self.fill_span(Span::new(p.y, p.x, 1), color)
    }
}

/// A blitter over a `Framebuf` the caller owns.
///
/// **It is a colour buffer and nothing else: capacity plus addressing.** No
/// viewport, no policy, no knowledge of a rasterizer or a renderer. The rect it
/// answers for arrives with `begin_region`, which is the only thing that ever
/// aims it — an earlier shape took a `viewport: Size` to choose between "aimed
/// at the frame" and "aimed at nothing", a branch that was a fossil of the era
/// when `begin_region` skipped full-frame surfaces. The field was written and
/// never read.
///
/// **It always has its target.** There is no attached/detached type-state here:
/// the state an application wants — "the renderer is between frames and I am
/// holding the pixels" — belongs to the *renderer*, and this type is what moves
/// in and out of it. (An earlier shape put the type-state here; see
/// `RasterRenderer` for what that cost.)
///
/// **No `P` parameter**: a policy is the application's declaration about
/// frames, not a property of a thing that merely has a size.
///
/// This type IS the loan. It owns `B`, so handing the blitter back hands back
/// the buffer — WS6.7's DMA-soundness requirement (a borrow the core can still
/// write through is UB, which is why `embedded-dma`'s `ReadBuffer` is
/// `unsafe`). `into_storage` unwraps it where the raw slice is what the
/// transport wants.
pub struct FramebufBlitter<C, B>
where
    C: Color + PackedColor,
    B: FramebufStorage<C>,
{
    framebuf: Framebuf<C, B>,
    viewport: Size, // the display's, not the region's
}

impl<C, B> FramebufBlitter<C, B> /* + the struct's bounds */ {
    /// Infallible: a colour buffer of any size is a valid colour buffer, and
    /// whether it is big enough is a question about a policy this type has
    /// never heard of. Starts aimed at nothing.
    pub fn new(storage: B) -> Self;
    pub fn into_storage(self) -> (B, Rect);
    pub fn framebuf(&self) -> &Framebuf<C, B>;
}

impl<C, B> Blitter for FramebufBlitter<C, B> {
    type Color = C;
    fn bounds(&self) -> Rect;
    fn capacity(&self) -> Option<usize> { Some(self.framebuf.capacity_units()) }
    fn fill_span(&mut self, span: Span, color: C);            // span_range + packing
    fn fill_rect(&mut self, rect: Rect, color: C);            // whole-word runs;
                                                              // coalesces all rows
                                                              // when full width
    fn blend_span(&mut self, span: Span, color: C, cov: &[u8]); // real RMW
    fn begin_region(&mut self, region: Rect) -> RenderResult;    // retarget + prime
}

/// A blitter over a tiny-skia `Pixmap` — the desktop/simulator sink.
///
/// `Pixmap::take` + `from_vec` re-stride the storage, which is how
/// `begin_region` reshapes it per region (the trick WS6.4d already uses), so
/// its bound is a byte budget exactly as for a framebuffer. The loan is the
/// pixels, which the simulator takes back from `detach` and displays.
pub struct PixmapBlitter { /* pixels: Vec<u8>, capacity, region, viewport */ }

impl Blitter for PixmapBlitter {
    type Color = tiny_skia::Color;
    fn bounds(&self) -> Rect;
    fn capacity(&self) -> Option<usize>;   // width * height — one unit per pixel
    fn fill_span(&mut self, span: Span, color: Self::Color);
    fn blend_span(&mut self, span: Span, color: Self::Color, cov: &[u8]); // real RMW
    fn begin_region(&mut self, region: Rect) -> RenderResult;  // re-stride + prime
}

// ===========================================================================
// The clip gate
// ===========================================================================

/// The **only** way a rasterizer touches a blitter.
///
/// Both fields private and the constructor `pub(crate)`, so a `Rasterizer` impl
/// has no path to `T`, and `clip ⊆ blitter.bounds()` holds by construction.
///
/// | | enforced? |
/// |---|---|
/// | writing outside the blitter | **impossible** — private field |
/// | writing outside the clip | **impossible** — every method intersects |
/// | retargeting mid-primitive | **impossible** — `begin_region` is not here |
/// | *bounding your loops* by the clip | advisory — spray-and-clip is correct, slow |
///
/// The last row cannot be closed by types but can be measured: count
/// fully-clipped-away spans and surface it in debug.
pub struct RasterCtx<'a, T: Blitter + ?Sized> {
    blitter: &'a mut T,
    clip: Rect,
}

impl<'a, T: Blitter + ?Sized> RasterCtx<'a, T> {
    /// Only the renderer builds one — this is where `clip ⊆ bounds` is made true.
    pub(crate) fn new(blitter: &'a mut T, clip: Rect) -> Self {
        let clip = clip.intersection(&blitter.bounds());
        Self { blitter, clip }
    }

    /// Advisory: bound your loops with this and nothing is thrown away.
    pub fn clip(&self) -> Rect { self.clip }

    /// For delegating to another primitive — every default body needs it.
    pub fn reborrow(&mut self) -> RasterCtx<'_, T>;

    pub fn span(&mut self, span: Span, color: T::Color);
    pub fn pixel(&mut self, p: Point, color: T::Color);
    pub fn rect(&mut self, rect: Rect, color: T::Color);

    /// Clipping slices per-pixel data in step — `Span::clip_to` returns the
    /// offset so that arithmetic exists once.
    pub fn run(&mut self, span: Span, colors: &[T::Color]);
    pub fn blend(&mut self, span: Span, color: T::Color, coverage: &[u8]);
}

// ===========================================================================
// Layer 2 — Rasterizer: geometry → spans
// ===========================================================================

/// Shared scan conversion. Every `Rasterizer` default body is a one-line
/// delegation here, so a default is real drawing code rather than a stub, and
/// it is shared rather than monomorphized per rasterizer.
///
/// It is also the home for primitives embedded-graphics does not have.
/// `raster::polygon` is **not new code**: `eg/primitives/polygon.rs` already has
/// `bounds()`, `lines()` (the stroke) and `contains()` (winding number), plus a
/// fill that scans the bounding box testing `contains` per pixel — and it is
/// **unreachable today**, because `Renderer::polygon` logs and skips instead of
/// calling it. Moving it here supplies the default and fixes the no-op at once.
/// Its two `TODO`s travel with it; they are not done.
///
/// The fill stays `O(w·h·edges)` on the move. A scanline fill emitting spans is
/// the natural rewrite under this protocol, but mixing an algorithm change into
/// a mechanical PR is how a refactor stops being reviewable — and the relocated
/// version already draws where today's draws nothing.
pub mod raster {
    pub fn fill<T: Blitter + ?Sized>(cx: &mut RasterCtx<'_, T>, r: Rect, c: T::Color);
    pub fn line<T: Blitter + ?Sized>(cx: &mut RasterCtx<'_, T>, from: Point, to: Point,
        style: &DrawStyle<T::Color>);
    pub fn arc<T: Blitter + ?Sized>(/* … */);
    pub fn ellipse<T: Blitter + ?Sized>(/* … */);
    pub fn polygon<T: Blitter + ?Sized>(/* … */);
    pub fn path<T: Blitter + ?Sized>(/* … */);   // flatten → polygon
    /* rect, rounded_rect, circle, sector, image — each exactly decomposed */
}

/// Stateless about *where* it draws — the blitter arrives per call.
///
/// That is required, not symmetry: a blitter that cannot read its own pixels
/// needs AA composited into a one-scanline scratch and *then* emitted, so two
/// blitters are live inside one primitive call. It also lets caches — a
/// coverage line, a `Mask`, a glyph atlas — survive a blitter swap.
///
/// **One method per primitive, never a `PrimitiveKind` match.** A new variant
/// breaks every downstream `match` on a version bump, and the `_ =>` wildcard
/// that silences it turns every future primitive into a permanent silent no-op.
/// This repo demonstrates the visible alternative: `polygon` is a logged no-op
/// in both eg impls precisely because it is a named method. The `cx` repetition
/// is the price; it is load-bearing (it is *why* the clip cannot be escaped) and
/// confined to the trait definition.
///
/// **No primitive is ever unsupported.** Every method has a default that
/// *draws*. Consequences:
///
/// - A new rasterizer is `impl Rasterizer for X {}` plus the overrides it has
///   better algorithms for.
/// - A **new primitive** must arrive with an exact decomposition onto the
///   existing set — and `path` makes that always possible, since every 2D shape
///   is a path. A primitive that cannot be expressed as one is a new
///   *capability*, not new geometry, and does not go on this trait.
/// - A default must never be a **lookalike** (a squircle drawn as a rounded
///   rect): that renders a plausible wrong image, the worst failure mode in
///   this design. Exact-or-via-`path`, never approximate.
///
/// **"Exact" means geometry, not pixels.** A default calls back through `self`,
/// so it inherits *that rasterizer's* quality automatically — `circle` drawn via
/// `self.arc` is anti-aliased exactly when the rasterizer is. What must agree
/// between rasterizers is therefore only **parameter semantics**: which
/// direction `sweep` runs and where angle zero sits, whether
/// `StrokeAlignment::Inside` is inside the *path*, what a corner radius
/// measures. Document those here as they are settled. Rasterizer *parity* is
/// explicitly not a goal — differing output is the reason there is more than
/// one.
///
/// The trait therefore requires **nothing**. `fill` and `pixel` are listed
/// first because every override chain bottoms out in them, and `fill` is
/// style-free because clears, backgrounds and region priming must not pay for
/// style resolution.
pub trait Rasterizer<T: Blitter + ?Sized> {
    fn fill(&mut self, cx: &mut RasterCtx<'_, T>, rect: Rect, color: T::Color) {
        raster::fill(cx, rect, color)
    }
    fn pixel(&mut self, cx: &mut RasterCtx<'_, T>, p: Point, color: T::Color) {
        cx.pixel(p, color)
    }
    fn line(&mut self, cx: &mut RasterCtx<'_, T>, from: Point, to: Point,
        style: &DrawStyle<T::Color>) { raster::line(cx, from, to, style) }
    fn rect(&mut self, cx: &mut RasterCtx<'_, T>, rect: Rect,
        style: &DrawStyle<T::Color>) { raster::rect(cx, rect, style) }
    fn rounded_rect(&mut self, cx: &mut RasterCtx<'_, T>, rect: Rect,
        corners: CornerRadii, style: &DrawStyle<T::Color>) { /* raster:: */ }
    fn arc(&mut self, cx: &mut RasterCtx<'_, T>, top_left: Point, diameter: u32,
        start: Angle, sweep: Angle, style: &DrawStyle<T::Color>) { /* raster:: */ }
    fn sector(&mut self, cx: &mut RasterCtx<'_, T>, top_left: Point, diameter: u32,
        start: Angle, sweep: Angle, style: &DrawStyle<T::Color>) { /* raster:: */ }
    fn ellipse(&mut self, cx: &mut RasterCtx<'_, T>, bounding_box: Rect,
        style: &DrawStyle<T::Color>) { /* raster:: */ }
    fn polygon(&mut self, cx: &mut RasterCtx<'_, T>, points: &[Point],
        style: &DrawStyle<T::Color>) { /* raster:: */ }
    fn path(&mut self, cx: &mut RasterCtx<'_, T>, path: &Path,
        style: &DrawStyle<T::Color>) { /* raster:: */ }
    fn image(&mut self, cx: &mut RasterCtx<'_, T>, image: DrawImage<'_, T::Color>)
        { /* per-row cx.run */ }

    /// Exact: a full sweep. `EgRasterizer` **overrides** it —
    /// `eg/primitives/circle.rs` has its own algorithm and taking the default
    /// would silently discard it.
    fn circle(&mut self, cx: &mut RasterCtx<'_, T>, top_left: Point,
        diameter: u32, style: &DrawStyle<T::Color>)
    {
        self.arc(cx, top_left, diameter, Angle::zero(), Angle::FULL_CIRCLE, style)
    }

    // `glyphs` arrives with WS15. Its default is not geometry: the font layer
    // supplies a coverage bitmap, so the default blits it row by row through
    // `cx.blend`. Until then text keeps its per-pixel path through the existing
    // `DrawTargetProxy` and the layering is clean everywhere else.
}

/// embedded-graphics algorithms, **as-is, no anti-aliasing**.
///
/// **Overrides** `line`, `rect`, `rounded_rect`, `circle`, `arc`, `sector`,
/// `ellipse` — the seven eg has primitives for — each body being the delegation
/// PR A leaves behind, with the receiver changed to `BlitTarget(cx)`.
/// **Inherits** `raster::` for `polygon` and `path`, which eg does not have.
/// `image` may go either way; today's `renderer_image` is the eg version.
///
/// **Do not override `fill`.** The default reaches `cx.rect` →
/// `Blitter::fill_rect` → `Framebuf::fill_solid`, which is WS6.3b's whole-word
/// path. Routing it through eg's `Rectangle` adds a hop for nothing.
///
/// No `C` parameter — the colour comes from `T::Color`, and a `PhantomData<C>`
/// would add a monomorphization axis with no code difference.
pub struct EgRasterizer;

/// tiny-skia's rasterizer as an L2 citizen. Holds one reusable coverage `Mask`,
/// refilled per primitive.
///
/// **The mask is keyed on `cx.clip()`, not on the region** — L2 never learns a
/// region exists, and the clip is the largest rect it may write anyway.
/// Reallocate only when the clip grows past `mask_for`.
///
/// **Generic over the blitter with no colour pinned**, which is the point: a
/// `Mask` is colourless, so this fills coverage and hands it to
/// `cx.blend(span, style_color, &coverage_row)` — making tiny-skia's
/// anti-aliasing available over an Rgb565 framebuffer, not only over a `Pixmap`.
///
/// It never touches `PixmapMut`, `fill_path` or `stroke_path` — those are
/// tiny-skia's *fused* API, and fusing is what this design undoes.
/// `Mask::fill_path(&path, rule, anti_alias, transform)` is the primitive
/// underneath them, and this repo already calls it (`tiny_skia/mod.rs:333`).
pub struct TinySkiaRasterizer { mask: Option<Mask>, mask_for: Rect }

/// Planned: rsact's own embedded-optimized rasterizer with blending and AA.
/// Starts as `impl Rasterizer for RsactRasterizer {}` and grows override by
/// override. Will hold one scanline of coverage, bounded by the clip width.
pub struct RsactRasterizer { /* coverage: Vec<u8> */ }

impl<T: Blitter + ?Sized> Rasterizer<T> for EgRasterizer { /* … */ }
impl<T: Blitter + ?Sized> Rasterizer<T> for TinySkiaRasterizer { /* … */ }

// ===========================================================================
// Layer 1 — Renderer: what rsact-ui receives
// ===========================================================================

/// Unchanged by this plan: `size()` keeps its name, `clip_bounds` stays
/// `Option<Rect>`, `type Policy` stays until Appendix A, and the geometry set
/// is today's twelve methods.
///
/// `PrimitiveKind` survives as a **value** vocabulary — recording, replay,
/// `Canvas`'s command list — never as this trait's dispatch. A new variant
/// would be a breaking change for every third-party backend.
pub trait Renderer { /* unchanged — see renderer.rs */ }

// ── No `ClipStack` type ────────────────────────────────────────────────────
//
// Two things hold a clip stack after the split, and they are not
// interchangeable:
//
//   1. `RasterRenderer` — the L1 renderer every rasterizer plugs into;
//   2. `RecordingRenderer` — necessarily L1, because it logs *primitives* and
//      `DrawOp::Clip`, none of which exist below L1, and it logs the
//      **requested** rect while storing the **narrowed** one.
//
// (`NullRenderer` has none; a future `GpuRenderer` would have its own.)
//
// Extracting a shared type was considered and rejected: the invariant that
// actually produced a bug — nested clips must compose, or a widget clip inside
// a region clip lets drawing escape its tile — is already centralised in
// `ViewportKind::nested_in`, and with `Cropped` deleted that composition IS
// `Rect::intersection`. A shared type would wrap one line, and the two holders
// do different things on mutation anyway.
//
// **`ViewportKind` is deleted with it.** `Cropped` has no live constructor and
// no planned one; absolute positioning is a *bounds* extension
// (`paint_bounds`/`ext_draw`, WS6.4c(G)), not a coordinate space, and offscreen
// layers would rebase at L3 where `local()` already lives. That leaves
// `Fullscreen` and `Clipped(Rect)`, and `Fullscreen` already behaves as
// `Clipped(surface_rect)` everywhere — `renderer_clip_bounds` substitutes the
// surface rect for it deliberately, because WS6.4b needed culling to pay on an
// ordinary full-frame render and not only under tiles. So each stack becomes a
// plain `Vec<Rect>` seeded with the surface rect.
//
// Two facts for the implementer: `DrawOp::Clip(Rect)` (`record.rs:33`) already
// stores a plain rect, so the golden format is unaffected; and `record.rs:314`'s
// `.unwrap_or_else(ViewportKind::root)` becomes `.unwrap_or(surface_rect)`.
//
// What must survive as documented, tested properties on each holder:
//
//   push  — stores `area ∩ top`, so the top IS the effective clip (this is what
//           makes reading it for culling exact rather than approximate);
//   pop   — never pops the root, so an unbalanced pop degrades rather than
//           leaving the renderer with no clip;
//   reset — a region is the ROOT of the stack, so no clip can escape it.

/// The `Renderer` built around a `Rasterizer`, with the blitter as the
/// interchangeable sink.
///
/// # The renderer is long-lived; the TARGET is what comes and goes
///
/// `A` is the attachment type-state, and it is on **this** type rather than on
/// the blitter.
///
/// A renderer retains state a caller pays to build: the clip stack, and a
/// rasterizer's caches (`TinySkiaRasterizer` holds a coverage `Mask` and a
/// `PathStroker`; `RsactRasterizer` will hold a scanline). So it is created once
/// and lives for the application. What is *lent* is the caller's paint target —
/// a framebuffer, a pixmap, a `DrawTarget`, a GPU attachment — and a `Blitter`
/// is exactly the thing that wraps one. So `attach` takes a blitter and `detach`
/// gives it back, and a blitter is always in the one state where it has its
/// target.
///
/// **The first shape of this put the type-state on the blitter**
/// (`FramebufBlitter<C, B, Attached>`), which forced a *specialized* attach and
/// detach pair on `RasterRenderer` per blitter kind — four impl blocks for two
/// blitters, each re-stating the capacity proof, and one of them was written
/// without the runtime half, so a 240-unit slice satisfied `Tiles<240, 24>` in
/// silence. That duplication was structural: `DirectBlitter` and DMA2D would
/// each have added another pair and another chance to forget. It also could not
/// express a target that is not storage at all — a direct-to-`DrawTarget`
/// blitter has no buffer to hand back, only itself.
pub struct RasterRenderer<R, T, P = Unbounded, A: Attachment<T> = Attached> {
    rasterizer: R,
    blitter: A::Slot,   // T attached, () detached
    clips: Vec<Rect>,   // survives a detach: it is the renderer's, not the target's
    viewport: Size,
    policy: PhantomData<fn() -> P>, // marker-only: no dropck, no auto-traits
}

impl<R, T, P> RasterRenderer<R, T, P, Detached> {
    /// A renderer with no target yet — what an app builds at boot. No bound at
    /// all, so it can be built before its blitter type is known to satisfy
    /// `Blitter`.
    pub fn new(rasterizer: R, viewport: Size) -> Self;
}

impl<R, T: Blitter, P: FramePolicy> RasterRenderer<R, T, P, Detached> {
    /// **One implementation, every blitter**, and the whole capacity proof.
    /// Checked BOTTOM-UP: the blitter states what it holds, the policy states
    /// what the application wants asked of it.
    ///
    /// | the target | when it is checked |
    /// |---|---|
    /// | `&'static mut [u16; 5760]` | **compile time** — `UNITS` is `Some` |
    /// | `&'static mut [u16]`, a `Pixmap` | here, as an `Err` |
    /// | direct-to-panel | never — `capacity()` is `None`, i.e. *unbounded* |
    ///
    /// The packing agreement is always a compile error: both sides are consts.
    ///
    /// **Never panics.** Whether a memory-plan mistake should abort is the
    /// application's call; `AttachError` hands back both halves.
    pub fn attach(self, blitter: T)
        -> Result<RasterRenderer<R, T, P, Attached>, AttachError<R, T, P>>;
}

impl<R, T: Blitter, P: FramePolicy> RasterRenderer<R, T, P, Attached> {
    /// The parked renderer, spelled on the ATTACHED type — that is the type an
    /// application names (`W::Renderer`), so a caller never writes `Detached`.
    pub fn parked(rasterizer: R, viewport: Size)
        -> RasterRenderer<R, T, P, Detached>;

    /// Sugar for `parked(..).attach(..)`.
    pub fn with_blitter(rasterizer: R, viewport: Size, blitter: T)
        -> Result<Self, AttachError<R, T, P>>;

    /// The blitter is the loan token: it owns whatever the caller lent it, so
    /// ownership moves out with it (WS6.7). The painted rect comes back as
    /// `Blitter::bounds()`.
    pub fn detach(self) -> (RasterRenderer<R, T, P, Detached>, T);

    pub fn blitter(&self) -> &T;
    pub fn covers(&self) -> Rect;         // == blitter.bounds()
    pub fn rasterizer(&mut self) -> &mut R;

    fn clip(&self) -> Rect;
    fn split(&mut self) -> (&mut R, RasterCtx<'_, T>);
}

// NOTE on the two halves: `Blitter::UNITS` carries the static one up to
// `attach`, where a `const` block turns a too-small fixed-size array into a
// build failure. That is the *only* assertion left in this crate's shipping
// code, and it is a compile-time one. Everything a value can get wrong is a
// `Result` or an `Option`.

impl<R, T, P> Renderer for RasterRenderer<R, T, P, Attached>
where R: Rasterizer<T>, T: Blitter, P: FramePolicy
{
    type Color = T::Color;
    type Policy = P;

    fn size(&self) -> Size { self.viewport }

    fn begin_region(&mut self, region: Rect) -> RenderResult {
        self.blitter.begin_region(region)?; // retargets AND primes
        self.clips.clear();
        self.clips.push(region); // the region is the ROOT — nothing escapes it
        Ok(())
    }

    /// Kept, unlike on `Blitter`: this is where a batching rasterizer flushes
    /// and where a GPU ends its pass.
    fn end_region(&mut self) -> RenderResult { Ok(()) /* + rasterizer flush */ }

    fn push_clip(&mut self, area: Rect) {
        let nested = area.intersection(&self.clip());
        self.clips.push(nested)
    }
    fn pop_clip(&mut self) { if self.clips.len() > 1 { self.clips.pop(); } }

    /// `Option`, not `Rect`. There is no expressible "unbounded":
    /// `Rect::intersection` uses non-saturating `+` and `u32::MAX as i32 == -1`,
    /// so a `Rect::MAX` sentinel intersects to `Rect::zero()` — "unbounded"
    /// would read as "clips everything".
    fn clip_bounds(&self) -> Option<Rect> { Some(self.clip()) }

    // Every geometry method is the same three lines — cull, split, forward:
    fn rect(&mut self, rect: Rect, style: &DrawStyle<Self::Color>) -> RenderResult {
        if !rect.intersects(&self.clip()) { return Ok(()) } // cull
        let (r, mut cx) = self.split();
        r.rect(&mut cx, rect, style);
        Ok(())
    }
    // …and eleven more identical in shape.
    //
    // This is where a forwarding macro earns its place: one private impl inside
    // this crate, mechanically identical bodies, and — unlike a macro over the
    // *trait* definition — it hides nothing from a reader of the public API.
    // `PrimitiveKind` cannot serve here either: building one to immediately
    // destructure it would allocate for `polygon` and `path`.
    //
    // The same concession should cover in-crate *proxies*: `RenderCtx`
    // (`rsact-ui/src/el/render.rs:392-597`) is 205 lines of twelve forwarders
    // each carrying an identical `if muted { return Ok(()) }`.
}

// `UI`, `Frame`, the region planner and the loan loop are untouched:
//
//     let mut frame = ui.start_frame(&mut renderer);
//     while frame.render(&mut renderer).is_some() {
//         let (parked, blitter) = renderer.detach();
//         let (buf, at) = blitter.into_storage();   // or read it in place
//         flush(&mut display, &buf, at);
//         renderer = parked.attach(FramebufBlitter::new(viewport, buf));
//     }
```

---

## Invariants

Things an implementer must not break. Most are enforced by the shapes above;
these are the ones worth checking against.

1. **Absolute coordinates until inside the blitter.** The clip is absolute; a
   second coordinate space would put every method on a seam. Matches shipping
   behaviour — `flat_index` resolves absolute against the viewport.
2. **Every position-dependent effect is a function of absolute coordinates** —
   dithering, gradients, pattern fills. Tile-relative ones seam at every region
   boundary. (WS6.4's standing invariant; unchanged.)
3. **A region is the root of the clip stack**, and `push` intersects with its
   parent. Both are what make `clip_bounds()` exact enough to cull on.
4. **`begin_region` retargets and primes atomically**, before the region clip is
   pushed.
5. **Out-of-bounds writes are unrepresentable**, not checked — `RasterCtx`'s
   private field and `pub(crate)` constructor are the mechanism, so nothing may
   hand a rasterizer a `&mut T`.
6. **No lookalike defaults.** Exact decomposition or `path`; a plausible wrong
   image is the failure mode this design refuses.
7. **`detach` returns an owned blitter, which owns the buffer** — WS6.7's DMA
   soundness depends on ownership moving out, not on the shape of what moves.
9. **rsact never panics on the user's behalf.** Construction and attachment
   return `Option`/`Result`; the only assertions in shipping code are the
   `const` ones in `attach`, which are build failures rather than runtime
   panics. Whether a memory-plan mistake should abort is the application's
   decision, and `unwrap` at the call site is that decision, written where a
   reader can see it.
10. **A blitter is aimed by `begin_region` and by nothing else.** It takes no
   viewport and holds no shape of its own; "aimed wrongly at construction" is
   not a state that exists, which retires the WS6.4d bug class rather than
   fixing one instance of it.
8. **Addressing stays overridable per blitter** (rotation, page packing), which
   is why `local`/`pixel_index`/`span_range` are free functions.

### Compile constraints found in review

Each was verified against rustc; each is a place the obvious spelling fails.

- **`A: Attachment<T>` on `RasterRenderer` is fine**, and the earlier "no
  `where` clause" constraint dissolved with the rework: the bound that was
  ill-formed was `T: Blitter`, because the *detached* `T` was a different,
  non-`Blitter` type. Now `T` is the same type in both states and only `A`
  changes, and `Attachment<S>` is implemented for any `S`.
- **No associated consts on `Blitter`** — they make it dyn-incompatible (E0038).
- **`Span` is `{y, x, w}`** — `Range` is not `Copy`, and defaults read the span
  twice (`E0382`).
- **The capacity proof is one check in `attach`, split by what is knowable.**
  `Blitter::UNITS` / `Blitter::PIXELS_PER_UNIT` are consts, so a too-small
  fixed-size array and any packing disagreement are **build failures**;
  `Blitter::capacity()` is the value, so a runtime-length target is an `Err`;
  `capacity() == None` means *unbounded* and is not checked at all. That needed
  associated consts on `Blitter`, which costs dyn-compatibility (E0038) — spent
  deliberately, since an application uses one renderer + rasterizer + blitter
  and there is no cross-product to erase.
- **A colour-derived `PACKING` const would need `Color: PackedColor`**, which
  `tiny_skia::Color` and `NullColor` fail — so packing may never be a bound on
  `Blitter`. (Appendix A.)
- **`clip_bounds` must stay `Option<Rect>`** — see the doc comment above.
- **`PhantomData<fn() -> P>`**, not `PhantomData<P>` — no dropck, no
  auto-traits.

---

## Declined

Only the ones an implementer might otherwise re-decide.

| Declined | Because |
|---|---|
| A `ClipStack` type | the invariant is already shared (`nested_in`); two holders, doing different things on mutation |
| `PrimitiveKind` as trait dispatch (either layer) | a new variant breaks every downstream `match`; the `_ =>` wildcard makes future primitives permanent silent no-ops |
| A geometry *specification* / rasterizer parity | differing output is the reason there is more than one rasterizer. Only parameter semantics are shared |
| An error channel on L2/L3 | nothing to report once every primitive draws |
| `read_pixel` on `Blitter` | blending is the only reason to read; one method keeps capability and use from disagreeing |
| `end_region` on `Blitter` | every impl is a no-op; the async fence belongs where the loan goes back |
| Per-axis region maxima | every real ceiling is a byte count or an alignment; a per-axis cap costs the shape-morphing win |
| `Renderer::size()` → `viewport()` | cosmetic churn across live forwarders |
| tiny-skia as an L1-only backend | `Mask` exposes the coverage its fused API is built on, so coverage is produced once and blended once — in our blitter. Pinning it to L1 would also lock its AA to a `Pixmap` |

---

## Where the code goes

Proposed, and the implementer may move things — but decide it once, up front,
rather than per type.

**The tree is cut by feature gate, not by layer** (maintainer decision). A
directory exists because its contents cannot compile without a backend crate;
everything unconditional is a flat file in `src/`. That is what keeps `#[cfg]`
out of the shared files entirely — enabling a feature adds a directory rather
than activating scattered attributes.

```text
rsact-render/src/
  framebuf.rs            Framebuf · FramebufStorage · PackedColor   [WS6.4e]
  blitter.rs             Blitter · Span · local/pixel_index/span_range · FramebufBlitter
  raster.rs              RasterCtx · Rasterizer            (the contract)
  scan.rs                the shared scan conversion        (the algorithms)
  renderer.rs            Renderer trait · NullRenderer · RasterRenderer
  eg/                    [feature embedded-graphics]
    color.rs · framebuf.rs · image.rs
    interop.rs           DrawTargetProxy · the style conversions
    rasterizer.rs        EgRasterizer · BlitTarget
  tiny_skia/             [feature tiny-skia]
    color.rs · geometry.rs · path.rs
    blitter.rs           PixmapBlitter
    rasterizer.rs        TinySkiaRasterizer
```

`scan` is a **sibling** of `raster`, not a child: `raster.rs` is the contract
(the clip gate and the trait), `scan.rs` is 600 lines of algorithm, and the two
are read for different reasons.

**`eg/primitives/` is gone.** It held seven files of `pub fn draw`, each taking
an rsact `Line`/`Arc`/`Circle` and converting to embedded-graphics' — a shape
left behind when PR A deleted the anti-aliased halves those modules paired with.
Inlined into `EgRasterizer`, each body is one `draw_styled` call and the
intermediate rsact primitive disappears: the arguments go straight into eg's
constructor, which is where they always ended up two hops later.

**Two `DrawTarget` adapters coexist, and they are not the same thing.** Expect to
be confused by this once:

- `BlitTarget<'a, T>(RasterCtx<'a, T>)` — **new**, L2→L3, lets eg's
  `StyledDrawable` algorithms emit into a blitter.
- `DrawTargetProxy<'a, R: Renderer>` — **existing**, above L1, is how
  `embedded-text`/u8g2 hand glyph pixels to a renderer. Unchanged by this plan;
  text still arrives as `Renderer::pixel` until WS15.

## Migration surface

Everything that stops compiling, so it can be planned rather than discovered.

**The examples were already broken before any of this** — all ten of them, and
since at least 2026-07-13, which `docs/plans/2026-07-13-ws13.4-notes.md` records
as a census ("0 OK / 10 BROKEN, pre-existing"). The causes are unrelated API
drift: no `col!`/`row!` macros, `widget::ctx` private, `UI::new_eg` gone, `.el()`
gone, `Theme::with_accent` gone, `u8g2-fonts` not a dependency, `draw_styled`
gone from the primitives. No CI job builds them, which is why.

**Maintainer decision: touch only what the refactor breaks.** The rows below are
mechanical fixes to lines the split invalidates, nothing more; repairing the rest
is its own workstream. The acceptance criterion is therefore not "the examples
run" but **no NEW example breakage, diffed against master** — capture
`cargo check -p rsact-ui --examples --features "std,simulator,embedded-graphics,tiny-icons,icons-all-sizes,tiny-skia"`
on master first and compare error sets.

| Site | What changes | PR |
|---|---|---|
| `rsact-ui/examples/{sandbox,scrollable}.rs` | name `AntiAliasingDisabled` explicitly — the witness is deleted | **A** |
| `rsact-ui/examples/primitives_aa.rs` | **deleted**, with its `[[example]]` entry — its entire subject is what PR A removes | **A** |
| `rsact-ui/examples/{icons,3d_printer,mem_usage_display_240_240}.rs` | `EGRenderer::new(…)` → `RasterRenderer::with_blitter(EgRasterizer, viewport, FramebufBlitter::new(…))` | **C** |
| `rsact-render/src/lib.rs:80` | prelude re-exports `EGRenderer` | **C** |
| `eg/renderer.rs`'s tests (`:1450`–`:1739`) | move with the code they cover; the `pixel_alpha` invariance test is deleted outright (its subject is gone) | **A**/**C** |
| `rsact-ui/src/el/ctx.rs` — `Wtf<R, …>` | nothing structural: `W::Renderer` is still one type, now a longer one. Consider a type alias in the prelude so examples name it once | **C** |
| `rsact-ui/src/ui.rs` `start_frame` | unchanged — `type Policy` stays until Appendix A | — |

## Definition of done

Per PR, so an implementer knows when to stop.

- **A** — `cargo check` clean across the feature powerset; no occurrences of
  `pixel_alpha`, `AntiAliasing`, `EgPrimitive` or `EgPrimitiveRenderer` remain;
  every suite green; **no golden re-blessed** (nothing observes AA — if one
  moves, that is a finding).
- **B** — the new modules compile and are dead: nothing outside them references
  `Blitter`, `Rasterizer` or `RasterRenderer`. Suites and goldens untouched by
  construction.
- **C** — `EGRenderer` and `ViewportKind` are gone; **the tile/schedule goldens
  are byte-identical**; the metrics and size probes are within their existing
  gates; no new example breakage (see Migration surface — they do not run today
  and repairing them is not this refactor's job).
- **D** — `TinySkiaRenderer`, `clip_mask` and `rebuild_clip_mask` are gone; the
  tiny-skia suite is green minus WS6.11's five deleted tests.

Every PR also: `cargo fmt`, the three feature powersets, the thumbv7m floor
build, and **no new warning** in `rsact-render` or `rsact-ui` (diff against
master rather than eyeballing — the crates carry pre-existing ones).

## Sequencing

**WS6.4e first** — it blocks everything here, and it is a pure move + rename
reviewed as its own PR. One thing to get right in it: make WS6.3b's fast
`fill_solid` an **inherent** `Framebuf` method with `DrawTarget::fill_solid`
delegating, or `FramebufBlitter::fill_rect` inherits a framebuf without the
8–32× win and re-forks the addressing.

| PR | Content |
|---|---|
| **A — delete eg AA** | `eg/primitives/*`'s `draw_aa` halves, `EgPrimitive`, `EgPrimitiveRenderer`, `AntiAliasing{,Enabled,Disabled}`, `pixel_alpha` + its test, and the second (AA) `Renderer` impl. Also the two checkbox page goldens |
| **B — the architecture, additive** | `Span`, `Blitter`, `FramebufBlitter`, `PixmapBlitter`, `RasterCtx`, `raster::*`, `Rasterizer`, `EgRasterizer`, `TinySkiaRasterizer`, `RasterRenderer`. Wired to nothing |
| **C — integration, atomic** | `EGRenderer` replaced by `RasterRenderer<EgRasterizer, FramebufBlitter<…>>`; `ViewportKind` deleted; `Whole<W, H>` deleted |
| **D — tiny-skia port** | `TinySkiaRenderer` replaced by `RasterRenderer<TinySkiaRasterizer, PixmapBlitter>`; `clip_mask`/`rebuild_clip_mask` and WS6.11's five tests deleted — `RasterCtx` clips before any blitter sees a span, so a backend-side clip mask has nothing left to do; the simulator takes pixels from the blitter's loan |

No half-refactored `EGRenderer` may exist in history, which is why C is one
commit-range. **D is separate** (maintainer decision) — an untouched second
backend is not a half-refactored one, and keeping `TinySkiaRenderer` whole across
C leaves its suite as a live control group.

**WS6.4e landed first, as its own commit.** Beyond the move it did three things
the rest of this depends on: WS6.3b's fast solid fill became an *inherent*
`Framebuf` method (`DrawTarget::fill_solid` delegates), `Framebuf::new`'s
`area % pps == 0` assert became a real capacity check against `units_for`
(roadmap 6.5(i)), and the module's tests now use a local 1-bpp color with no
embedded-graphics anywhere, which asserts the unbinding rather than describing
it. One hazard it creates, worth knowing before touching eg call sites:
`canvas.fill_solid(&eg_rect, c)` now resolves to the **inherent** method, so the
`DrawTarget` one must be named explicitly.

### What C decided that the plan left open

- **`eg/renderer.rs` becomes `eg/interop.rs`.** Two things in it are not a
  renderer and had to survive: `DrawTargetProxy` (how `embedded-text`/u8g2 hand
  glyph pixels *down into* a `Renderer`) and the `DrawStyle` → `PrimitiveStyle`
  conversions every `EgRasterizer` body needs. Keeping the old filename would
  have left a module called `renderer` with no renderer in it.
- **No crate-level type alias for the stack.** The plan suggested considering
  one so examples name it once. Declined: the three parameters *are* the
  architecture, hiding them behind `EgRenderer<C, B, P>` would re-create the
  fused name the split exists to remove, and every real call site already binds
  a local `type` (the tile-schedule harness binds three). A one-line alias
  remains available if it ever grates.
- **`ViewportKind`'s other two holders converted rather than waited.**
  `RecordingRenderer` and `TinySkiaRenderer` both kept `Vec<ViewportKind>`;
  both are now `Vec<Rect>`. The recorder gained a stated invariant worth having:
  it logs the **requested** rect and stores the **narrowed** one, because the op
  log measures what the widget layer asked for while `clip_bounds` must report
  what is in force. tiny-skia keeps its "no mask when the clip covers the
  surface" fast path, now spelled as a rect comparison instead of a variant.
- **`Polygon`'s geometry moved to `primitives/polygon.rs`**, unconditional.
  `bounds_of`/`contains` were behind the embedded-graphics feature, which is why
  `raster::scan` could not use them; they are free functions over `&[Point]`
  because both `Renderer::polygon` and `Rasterizer::polygon` take a slice, and
  building a `Polygon` to ask whether a pixel is inside it would allocate per
  primitive.
- **`Whole<W, H>` was `Tiles<W, H>` under a second name** — same `MAX_REGION`,
  same limits, byte for byte. Deleted, with what it taught folded into `Tiles`.

### What D confirmed

D10's argument was that tiny-skia as an L2 rasterizer is *strictly better* than
as an L1 backend, because a `Mask` is colourless. Two tests now hold that claim
up rather than asserting it:

- `coverage_reaches_the_blitter_as_coverage` — partial coverage arrives at a
  blitter as partial coverage. If the rasterizer ever starts thresholding, the
  reason it is L2 rather than a fused backend is gone, and this fails.
- `tiny_skias_anti_aliasing_works_over_a_framebuffer` — the same rasterizer, an
  `Rgb888` `FramebufBlitter`, and pixels that are neither background nor
  foreground. The fused `PixmapMut` API could never do this.

WS6.11's five clip tests are deleted rather than ported, per the roadmap: there
are no tiny-skia draw entry points left to forget a mask, and the guarantee is
now tested once where it is enforced (`RasterCtx`). Their one piece of hard-won
knowledge — a fresh tiny-skia canvas is **opaque white**, so `alpha != 0` passes
vacuously — is re-homed in `tiny_skia/mod.rs`'s module docs.

**PR A is smaller than it looks, and that is what keeps B honest.** Every non-AA
`draw` in `eg/primitives/*` is a ~10-line delegation to embedded-graphics'
`StyledDrawable` — `Arc::new(…).draw_styled(&style.into_primitive_style(),
renderer)`, and the same shape for Circle, Line, RoundedRectangle, Ellipse,
Sector. So of 1113 lines there, the AA halves plus both traits plus the witness
are the bulk; ~60 lines of delegation survive, plus `polygon.rs`. Both traits can
therefore die in PR A — they exist to hand primitives `pixel_alpha` (gone) and
`draw_pixels`; pure delegation needs only a `DrawTarget`, and `EGRenderer`
already is one. PR B's `EgRasterizer` is then those same bodies with the receiver
changed to `BlitTarget(&mut RasterCtx)` — a substitution, not a rewrite.

### Acceptance

Architectural consistency and correctness, plus one falsifiable check: **no
golden moves.**

Every existing golden is an L1 draw-op log (`checkbox_checked_64.txt` is four
primitive names and their rects) or a planner measurement
(`tile_schedule_240.txt`, `tile_plan/shape/merge/damage`). None can observe
anti-aliasing or spans — `pixel_alpha` is called *inside* `Renderer::circle`'s
body, below the seam `RecordingRenderer` sits on, and the span protocol is below
it too. They assert what the widget layer *asked* the renderer to draw, which is
exactly what must not change.

So the tile/schedule goldens must come out **byte-identical across every PR; a
golden that moves is a finding, not a re-bless.** Precedent: PR #37 landed the
`RenderCtx` seam "behaviour-neutral and proved so, every suite green with NOT ONE
golden re-blessed."

WS6.9's deferred **PNG** half becomes a post-refactor item — it is the only
golden that can see AA or spans, and worth most once `RsactRasterizer` exists.

---

## Open

1. **`EgRasterizer` is `draw_iter`-shaped internally.** The bridge is a ~40-line
   `BlitTarget<'a, T>(RasterCtx<'a, T>)` implementing `DrawTarget` with
   `draw_iter → cx.pixel`, `fill_solid → cx.rect`, `fill_contiguous → cx.run`,
   which preserves the WS6.3b win. So the span win covers **fills; outlines and
   strokes stay pixel-shaped**, because that is how embedded-graphics' own
   algorithms are written. `RsactRasterizer` is where the stroke side becomes
   span-shaped.
2. **Monomorphization.** `T` reaches the widget tree through `WidgetCtx`, so the
   tree instantiates per (rasterizer, blitter) pair. Two levers exist and neither
   is taken: `dyn Blitter<Color = C>` stays possible and would collapse the
   rasterizer half without touching a rasterizer body; deleting
   `Renderer::Policy` (Appendix A) would remove the last associated type a
   `dyn Renderer<Color = C>` must name — and *that* is what would shrink the
   widget tree, which is the bigger prize.

Related: **ISSUE-7** (`path`'s `ArcTo` uses `current_pos` instead of the
segment's `center`, never advances the cursor, and ignores `Close`) is fixed
inside `raster::path`. The WS6-wide review of this plan lives in the roadmap, not
here.

---

## Stage 2 — region alignment (WS6.5)

One real defect survives the split, fixable without any of Appendix A.

The design pads rows for packed colour but never *aligns* the region origin.
`BinaryColor` already has `PPS = 8`, so a mono tiled policy is writable today,
passes the existing packing assert, and could receive a region starting at x = 3
— whose leading byte straddles pixels 0–7 at flush, on hardware (SSD1306 over
4-wire SPI, e-paper RAM) that cannot be read back to fix it. Latent, since no
such policy ships.

```rust
/// Snap a region **outward** to one a `pps`-packed output can address.
/// Idempotent, never shrinks; `pps == 1` is the identity.
pub const fn align_region(region: Rect, pps: usize) -> Rect;
```

Three orderings must be specified with it, because each is a way to get it wrong:

1. **Capacity is tested on the aligned rect** — the merge veto currently tests
   the raw union (`region.rs:600`), and a union that fits can exceed capacity
   once aligned.
2. **Alignment may exceed the viewport.** On a 122-px-wide e-paper, damage at
   x = 118..122 aligns out to 112..128. Legal in panel RAM (16 bytes = 128
   addressable columns, the last 6 invisible), but it must be stated.
3. **The chunker aligns steps as well as origins**, and is total because
   `step ≥ cell`:

   ```rust
   let cell = /* 1 or pps, per axis */;
   let step_x = round_down_to(fit_width, cell.width).max(cell.width);
   let step_y = round_down_to(max_units / band_units(step_x), cell.height)
                    .max(cell.height);
   ```

---

## Appendix A — deferred byte/packing rework

Deferred because nothing in tree consumes it, the drift it would fix is already
guarded by a `const` assert (`eg/renderer.rs:335`), and `region.rs:312-317`
scopes the first packed policy as "a five-line ZST setting `PIXELS_PER_UNIT` to
8; the backend asserts it against the colour's own packing, so the two cannot
drift". **Trigger: the first packed surface that needs tiling, or a colour whose
storage ratio is not a whole number.**

```rust
pub enum Packing {                    // a report from the storage layer
    BytesPerPixel(u32),               // Rgb565 = 2, a packed Rgb888 = 3
    PixelsAlongX(u32),                // 1-bpp rows; e-paper's byte columns
    PixelsAlongY(u32),                // SSD1306/SH1106 pages
}
pub enum Ceiling { Unbounded, Tile(Size) }   // what the app declares
```

- **Count bytes, not `C::Storage` elements.** `PPS: usize` can only say *pixels
  per element*, never *elements per pixel*, so a 3-bytes-per-pixel colour in
  `[u8]` is inexpressible; today's workaround is `Rgb888: Storage = u32`,
  wasting 25%. Bytes *permit* the fix but do not perform it: expressing 3 needs
  `Storage = u8` plus byte-indexed addressing.
- **Padding and alignment are one fact.** A byte spanning several pixels cannot
  be half-written — simultaneously why a 122-px 1-bpp row costs 16 bytes and why
  the region must *start* on a byte boundary.
- **The axis is what a scalar cannot carry.** `region_units` is
  `ceil(w / pps) * h` (`renderer.rs:33`) — horizontal packing hard-coded. The
  example that actually differs is **SH1106's 132 columns**: a 132×8 page-row is
  132 bytes packed vertically, 136 under the current formula. (Whenever `8 | w`
  and `8 | h` the two agree, so a 128-wide SSD1306 shows nothing.) The axis earns
  its place through `align()`, not `bytes_of()`.
- **Packing belongs to the storage layer**, not the policy and not `Blitter`: it
  is the product of the colour and the layout, both storage facts. Two
  constraints — a colour-derived default requires `Color: PackedColor`, which
  `tiny_skia::Color` and `NullColor` fail; and if the axis stays *overridable*
  the drift is merely relocated (a blitter with `Color = Rgb565` declaring
  `PixelsAlongY(8)` computes 1/16 of the real bytes and compiles silently). So
  derive the ratio and declare only the axis, or assert against
  `size_of::<B::Storage>()`.
- **`MAX_REGION: Option<Size>` was a rectangle nothing treated as one.** Every
  consumer reduced it to units immediately, and a rect-shaped const read only as
  a scalar already cost a bug — a legal 16×38 merge chunked at y = 24, slicing a
  widget (`region.rs:106`). **This half is a pure deletion and could be taken
  early, independently of everything else here.**
- **Not a transfer denominator.** Storage and wire disagree on real parts
  (`Rgb666` stores 4 B/px; ST7789 18-bit mode ships 3), and "bytes" is not
  transport-ready without byte order — ST7789 wants RGB565 MSB-first, and a
  `[u16]` framebuf flushed raw on a little-endian Cortex-M comes out swapped
  (the crate carries unused `ByteOrder` markers at `color.rs:112`). That is
  WS6.7's problem.
- **No `Ceiling::Bytes` / `Budget<N>`.** Its justification was nRF52 SPIM
  `MAXCNT` as a *region* ceiling, which is a misreading: `MAXCNT` bounds one
  EasyDMA transaction, not one region, and every driver splits a `RAMWR` stream
  across many transfers while holding CS. nRF52832's `TXD.MAXCNT` is 8 bits —
  255 bytes, 127 RGB565 pixels — yet nRF52832 + ST7789 ships.
- **Deleting `Renderer::Policy` and adding `fn limits(&self, viewport)`** is
  correct once the ceiling needs the packing to become a number, and is a real
  simplification then. **Not defaulted**: `renderer.rs:220` argues the absence of
  a default is a feature, and `RenderCtx` — which exists only to forward — is the
  live victim of a silent one.
