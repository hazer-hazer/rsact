//! WS6.4d(1): turning a frame's damage rects into the regions it is painted in.
//!
//! This is the *plan* half of tiled rendering, and it is deliberately pure
//! geometry: no renderer, no buffer, no widgets. Given what changed
//! (`&[Rect]`, the damage the collect pass recorded) and what the output can
//! hold ([`RegionLimits`], derived from the frame policy), it answers "which
//! rectangles do we paint, in what order".
//!
//! # Why region *shape* is the whole game
//!
//! A tile has no history — it arrives holding whatever the last tile left in it
//! — so **everything intersecting a region repaints**, changed or not
//! (WS6.4c(1)). Region shape therefore sets the repaint set, and the difference
//! is not marginal: WS6.4a measured a tight 16×16 rect around one checkbox at
//! **3** required ops against **118** for the 240×24 full-width band containing
//! it, because the band catches every neighbour on those rows.
//!
//! Hence tight damage rects with an area-test merge, and *not* the row-bands the
//! original WS6.4 sketch proposed. Bands survive only as the degenerate case:
//! when damage covers most of the screen there is nothing left to be tight
//! about, so one screen rect chunks into bands and we are back to a classic
//! strip renderer — arrived at rather than designed in, which is what guarantees
//! tiling is never *worse* than strips.
//!
//! # The knob, and why it is an integer
//!
//! Merging two rects trades paint for transfer: the union pays one region's
//! command overhead instead of two, but repaints its dead space. The crossover
//! is the area ratio `union / (a + b)`, and WS6.4a measured it on a real frame
//! rather than guessing — adjacent rects 20 px apart came out at ×1.12, opposite
//! corners at ×23.62, overlapping at ×0.75. The gap is wide enough that anything
//! in ×1.5–×4 separates the cases, so the threshold is ×2.0 (LVGL's
//! neighbourhood) and is not delicate.
//!
//! It is stored as **percent in a `u32`**, not `f32`: this runs once per frame
//! on a Cortex-M0 with no FPU, where every `f32` compare is a soft-float call.
//! The comparison widens to `u64` so a 4K-class viewport cannot overflow it.
//!
//! ## …but the *shape* of this test is probably wrong, and that is now open
//!
//! Cost a plan as `N·F + p·Σarea` (F = fixed per-region cost, p = per-pixel).
//! Merging removes one region and adds `Δ` pixels of dead space, so it wins
//! exactly when `Δ < F/p`. That break-even is an **absolute number of pixels**,
//! not a ratio — and a ratio test scales the allowance with the size of the
//! rects being merged, which the cost model gives no reason for. Two 16×16
//! rects may add 512 px of dead space under ×2.0; two 100×100 rects may add
//! 20000 px, a third of a 240×240 screen, for the same one region saved.
//!
//! Rough numbers for the ST7789 reference target say the allowance should be
//! *small*: RGB565 at 40 MHz is ≈0.4 µs/px, and a region's fixed cost is a
//! ~11-byte command sequence plus DMA setup plus a short tree descent — call it
//! 10–30 µs, so `F/p ≈ 25–75 px`. That is nowhere near "double the area".
//! LVGL, for comparison, joins **only overlapping areas** and **only when the
//! union is strictly smaller than the sum** (`lv_refr_join_area`) — effectively
//! `Δ < 0`, i.e. it treats `F` as negligible, which is defensible when you own
//! the framebuffer and there is no per-region command sequence to pay.
//!
//! The roadmap cites ×2.0 as "LVGL's neighbourhood". That citation is wrong.
//! The replacement is not a better ratio but a measured `F/p` per target, which
//! is a `RegionPolicy` (6.5) parameter.
//!
//! The denominator double-counting any overlap is deliberate, not sloppiness:
//! `a + b` is what painting them *separately* costs, and the overlap really is
//! painted twice there. So the one test already prices WS6.4a's corollary that
//! an overlap should usually be merged away.
//!
//! **But "overlapping rects always merge" is false, and the fuzz below found
//! the counterexample** — worth stating explicitly because the roadmap phrased
//! the corollary as a rule. Two rects can overlap in 6 px and still have a
//! union 2.92× the sum of their areas: `35,0 2×15` (30 px) crossing
//! `23,4 14×3` (42 px) shares 6 px but its bounding box is 14×15 = 210. Merging
//! would paint 210 px to save one region's overhead and 6 px of double paint —
//! so the area test rejects it, and the test is *right*. What does hold
//! unconditionally is the sub-case that actually occurs: **containment always
//! merges** (if `b ⊆ a` then `union = a ≤ a + b`, for any threshold ≥ 1), which
//! is the common shape here — WS6.1's repaint roots damage a widget and the
//! ancestor rect containing it.
//!
//! A consequence to carry downstream: planned regions **may overlap**, so a
//! pixel can be painted (and transferred) twice. That is waste, never
//! corruption — painting is a pure function of position, so both passes write
//! the same value — and it is waste the area test has already priced as cheaper
//! than the alternative.
//!
//! # A tile is a budget, not a shape
//!
//! The surface constraint is a **unit count**, never a rectangle. A tile is a
//! byte buffer plus an instruction about where to blit it, and the buffer does
//! not care whether its 5760 units are laid out 240×24, 120×48 or 16×38. So
//! `Tiles<240, 24>` reads *"a buffer big enough for a 240×24 tile"*, and a
//! 16×38 damage region — 608 of those units — is emitted whole.
//!
//! Constraining the *shape* would be a constraint rsact invented rather than
//! one the hardware has. ST7789/ST7735 `CASET`/`RASET` take arbitrary column
//! and row ranges, and a region strided at its own width is contiguous by
//! construction, so any rect is one `set_window` plus one DMA burst — exactly
//! like a band. The panels that *do* constrain geometry are the ones that never
//! need tiling: SSD1306's 8-row pages and SH1106's page loop belong to 128×64
//! mono displays whose entire framebuffer is 1 KiB. e-paper's byte-aligned
//! columns are real, and they are roadmap 6.5's `RegionPolicy`, which composes
//! with a capacity bound rather than replacing it. The other ceiling that
//! exists — nRF52 SPIM's `MAXCNT` — limits a transfer's *byte count*, so a
//! capacity bound states it directly where a `W × H` bound could only
//! approximate it.
//!
//! This was not the first design. Bounding regions to `W × H` came first, and
//! it produced a bug worth recording, because it looked like a subtle economic
//! finding and was actually an artifact: two 16×16 rects 22 px apart merge on
//! area (×1.19) into 16×38, which a shape-bound 240×24 tile could not hold, so
//! chunking cut it at y = 24 and sliced the lower rect across both pieces — 8
//! required ops against 5 left alone. Under a capacity bound that merge simply
//! fits and none of it happens. The lesson generalises: *a constraint that
//! looks like it belongs to the data often belongs to the storage decision*,
//! and here it belonged to neither — it was invented.
//!
//! # The capacity veto, and its one exemption
//!
//! The veto survives for genuine overflow. When a union really does need more
//! units than the surface holds, it is rejected **before** the area test is
//! consulted, because chunking would undo it: **any merge that capacity will
//! immediately undo can only lose**, since chunking re-derives the split from
//! the union's own origin rather than from where the damage actually was.
//!
//! **With exactly one exemption: containment.** When one rect contains the
//! other the union *is* the container — a region already in the plan, already
//! chunked in exactly this way — so merging adds no area and no boundary, and
//! there is nothing for chunking to undo. Vetoing it instead orphans the inner
//! rect as a second region whose pixels are then painted twice, once alone and
//! once inside the container's chunk. This was a live bug until the explainer
//! built for WS6.4d(1) surfaced it. See [`capacity_allows`].
//!
//! There is deliberately **no region-count cap** alongside it. One existed for
//! two days and was removed once the cost model showed it could not help: with
//! `Cost = N·F + p·Σarea`, merging changes cost by `p·Δ − F`, so it wins exactly
//! when `Δ < F/p` — which is the comparison the area test already makes. At its
//! fixpoint every surviving pair has been priced and rejected, so forcing one
//! through changes cost by `p·Δ − F > 0`: **strictly worse, always, for every
//! display and every damage set.** Measured on six scattered changes, a cap of 4
//! took the frame from 1092 px to 9860 px and a cap of 2 to 50600 px, 88% of the
//! screen.
//!
//! The cases a cap seemed to be for are covered elsewhere or not at all. Panels
//! that want everything coalesced — e-paper, whose refresh costs hundreds of
//! milliseconds regardless of area — have an enormous `F` and a tiny `p`, so
//! `F/p` exceeds the whole screen and a correctly-parameterised area test merges
//! everything by itself. LVGL's `LV_INV_BUF_SIZE` looks like the same knob but
//! is not one: it is the length of a fixed C array, and its overflow behaviour
//! is to give up and invalidate the screen. rsact's damage list is a `Vec`, and
//! a crate whose widget tree is `Box<dyn Widget>` per node does not get to call
//! that its allocation problem.
//!
//! # Chunking preserves width
//!
//! Because the bound is one number, the natural cut keeps the region's full
//! width and takes as many rows as fit — `max_units / row_units(width)`. That
//! is simpler than a fixed grid and strictly better: it never introduces a
//! vertical seam, so a widget can only ever be split horizontally, and each
//! piece stays one `set_window` plus one burst. Bands still fall out as the
//! degenerate case — a collapsed full-screen region chunked by a 5760-unit
//! surface *is* ten 240×24 strips — which is what keeps the guarantee that
//! tiling is never worse than a strip renderer.

use crate::{
    geometry::{Point, Rect, Size},
    renderer::region_units,
};
use alloc::vec::Vec;

/// What the output path can accept — the frame policy's constraints, flattened
/// into the numbers the planner needs.
///
/// Separate from the policy *type* (WS6.4d(2)) on purpose: planning is ordinary
/// runtime geometry and there is no reason for it to be generic. The policy's
/// job is to prove at compile time that the surface can hold `max_region`; this
/// struct's job is to respect it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionLimits {
    /// Storage units the surface holds, or `None` when it covers the whole
    /// frame (a GPU, a host renderer, a full-size framebuffer).
    ///
    /// **A capacity, deliberately — not a shape.** A tile is a byte buffer plus
    /// an instruction about where to blit it, and the buffer does not care
    /// whether its 5760 units are laid out 240×24, 120×48 or 16×38. Constraining
    /// the *shape* would be a constraint rsact invented, not one the hardware
    /// has: ST7789/ST7735 `CASET`/`RASET` take arbitrary rects, and a region
    /// strided at its own width is contiguous by construction, so any rect is
    /// one `set_window` plus one DMA burst. (Panels that *do* constrain
    /// geometry — SSD1306's 8-row pages, e-paper's byte-aligned columns — are
    /// small enough not to need tiling at all, and their alignment rules are
    /// roadmap 6.5's `RegionPolicy`, which composes with this rather than
    /// replacing it.)
    ///
    /// It is also the shape the *other* ceiling has: nRF52 SPIM's `MAXCNT`
    /// limits a transfer's byte count, not its rectangle. A capacity bound
    /// states that directly; a `W × H` bound could only approximate it.
    ///
    /// `Option` rather than `usize::MAX` because "unbounded" is a real case, and
    /// it lines up one-to-one with [`Renderer::SURFACE_UNITS`]'s default.
    ///
    /// [`Renderer::SURFACE_UNITS`]: crate::renderer::Renderer::SURFACE_UNITS
    pub max_units: Option<usize>,

    /// How the surface packs pixels into storage units — [`Renderer::
    /// SURFACE_PIXELS_PER_UNIT`], carried here so the planner can convert a
    /// candidate region into units without knowing anything about colour.
    ///
    /// [`Renderer::SURFACE_PIXELS_PER_UNIT`]: crate::renderer::Renderer::SURFACE_PIXELS_PER_UNIT
    pub pixels_per_unit: usize,

    /// Merge two regions when `union.area * 100 <= threshold * (a.area +
    /// b.area)`, and the union fits [`RegionLimits::max_units`]. `200` is
    /// WS6.4a's measured ×2.0.
    pub merge_threshold_percent: u32,

    /// When the planned regions already *paint* this much of the viewport's
    /// area, give up on being tight and plan the whole viewport as one region
    /// (which [`RegionLimits::max_units`] then chunks into bands).
    ///
    /// Paint area, not coverage: regions may overlap, and a set that paints
    /// 95% of a viewport's worth of pixels is worth collapsing whether that is
    /// 95% of the screen once or 50% of it twice. Both readings point the same
    /// way — one region's dead space is bounded by 1/0.9 while a scattered plan
    /// pays `CASET`/`RASET`/`RAMWR` and a full tree traversal per region.
    pub full_frame_percent: u32,
}

impl RegionLimits {
    /// WS6.4a's measured merge threshold: area ratio ×2.0.
    ///
    /// **Under review** — see the module docs: the cost model says the merge
    /// criterion is *absolute* dead space against a pixels-per-region constant,
    /// not a ratio, so this is likely the wrong shape as well as the wrong
    /// value. Kept until the measurement that replaces it exists.
    pub const MERGE_THRESHOLD_PERCENT: u32 = 200;

    /// The whole-surface case: one unbounded region, no chunking. What a GPU or
    /// a full-size framebuffer wants.
    pub const fn whole() -> Self {
        Self {
            max_units: None,
            pixels_per_unit: 1,
            merge_threshold_percent: Self::MERGE_THRESHOLD_PERCENT,
            full_frame_percent: 90,
        }
    }

    /// A surface of `max_units` storage units packing `pixels_per_unit` pixels
    /// each.
    pub const fn tiled(max_units: usize, pixels_per_unit: usize) -> Self {
        Self {
            max_units: Some(max_units),
            pixels_per_unit,
            merge_threshold_percent: Self::MERGE_THRESHOLD_PERCENT,
            full_frame_percent: 90,
        }
    }

    /// Storage units `region` needs on this surface.
    pub const fn units_of(&self, region: Rect) -> usize {
        region_units(
            region.size.width,
            region.size.height,
            self.pixels_per_unit,
        )
    }

    /// Whether `region` fits the surface as-is.
    pub const fn holds(&self, region: Rect) -> bool {
        match self.max_units {
            None => true,
            Some(max) => self.units_of(region) <= max,
        }
    }
}

/// How a frame is cut into regions — **a type, not a value**.
///
/// The maintainer's requirement, verbatim (roadmap 6.4d(2)): *"FramePolicy is
/// not a dynamic value but one that applies a constraint over the framebuffer
/// that can be passed … so we are sure that user cannot pass a framebuffer
/// smaller than needed."* Hence the largest region a policy can ask for is an
/// associated const, which `UI::start_frame` compares against the renderer's
/// [`SURFACE_UNITS`] in a `const` block: **a framebuffer too small for the
/// policy is a compile error**, not a runtime check, and no `Frame` whose
/// regions could overflow the surface can be obtained.
///
/// Implement it on a zero-sized type; the two below cover the cases that exist.
///
/// # `W × H` is how you *spell* a budget, not a shape the planner obeys
///
/// A policy names a rectangle because that is what a person can picture and
/// what the compile error should say — but what it actually declares is the
/// **capacity** that rectangle implies. The planner is then free to emit any
/// region needing no more storage units than that: under `Tiles<240, 24>` a
/// 16×38 region (608 units) and a 120×48 one (5760) are both legal, and neither
/// is chunked. Constraining the shape would be a constraint rsact invented —
/// see [`RegionLimits::max_units`] for why the hardware does not have one.
///
/// [`SURFACE_UNITS`]: crate::renderer::Renderer::SURFACE_UNITS
pub trait FramePolicy {
    /// The width of the region whose capacity this policy declares.
    const MAX_W: u32;
    /// The height of the region whose capacity this policy declares.
    const MAX_H: u32;

    /// The runtime constraints, given the viewport and how the surface packs.
    ///
    /// A method rather than more consts because the knobs are viewport- and
    /// renderer-relative, and because this is where a policy gets to be
    /// opinionated without growing more type parameters. `pixels_per_unit` comes
    /// from [`Renderer::SURFACE_PIXELS_PER_UNIT`] and is what turns the
    /// declared `W × H` into a unit count.
    ///
    /// [`Renderer::SURFACE_PIXELS_PER_UNIT`]: crate::renderer::Renderer::SURFACE_PIXELS_PER_UNIT
    fn limits(viewport: Size, pixels_per_unit: usize) -> RegionLimits;
}

/// A surface that covers the whole frame: a GPU, a host renderer, a full-size
/// framebuffer. `W`/`H` are the display's own size, so the capacity proof still
/// runs — it is exactly the check that the full-framebuffer path has a full
/// framebuffer.
///
/// **A capacity claim, not a region-count claim.** This said "one region per
/// frame" and pinned the region count to one, justified as "a GPU wants one
/// walk, one scissor" — which conflated two unrelated things and measured at
/// **88% of the screen repainted for six small changes** (50600 px against
/// 1092), because a single region has to be the bounding box of all damage.
/// Region count is whatever the area test arrives at, here as everywhere.
///
/// Damage still shrinks the flush: regions are the damage rects, so an idle-ish
/// frame transfers very little even with a full framebuffer behind it.
///
/// `W`/`H` bound the emitted region rather than merely describing it, which
/// matters when they and the viewport disagree: a `Whole<240, 240>` policy
/// driving a 320×240 viewport degrades into bands instead of handing the surface
/// a frame 25% larger than it can hold. The compile-time proof only covers what
/// the *policy* asks for, so the policy has to be honest.
pub struct Whole<const W: u32, const H: u32>;

impl<const W: u32, const H: u32> FramePolicy for Whole<W, H> {
    const MAX_W: u32 = W;
    const MAX_H: u32 = H;

    fn limits(_viewport: Size, pixels_per_unit: usize) -> RegionLimits {
        RegionLimits::tiled(
            region_units(W, H, pixels_per_unit),
            pixels_per_unit,
        )
    }
}

/// A surface the size of a `W × H` region.
///
/// The embedded case. `Tiles<240, 24>` on RGB565 is an 11.25 KiB buffer against
/// the 112.5 KiB a 240×240 framebuffer costs — the WS6.4 acceptance target.
/// Read it as *"a buffer big enough for a 240×24 tile"*, not *"regions are at
/// most 240×24"*: a 16×38 damage region needs 608 of those 5760 units and is
/// emitted whole.
///
/// This is also where an app encodes its peripheral's transfer ceiling: nRF52
/// SPIM's `MAXCNT` is a hard limit independent of RAM, and only the app knows
/// it, so it belongs in the policy rather than anywhere in rsact. That ceiling
/// is a byte count, which is exactly what this declares.
pub struct Tiles<const W: u32, const H: u32>;

impl<const W: u32, const H: u32> FramePolicy for Tiles<W, H> {
    const MAX_W: u32 = W;
    const MAX_H: u32 = H;

    fn limits(_viewport: Size, pixels_per_unit: usize) -> RegionLimits {
        RegionLimits::tiled(
            region_units(W, H, pixels_per_unit),
            pixels_per_unit,
        )
    }
}

/// WS6.4.0(iii): compile-time proof that a surface can hold policy `P`'s
/// largest region.
///
/// Call it from a `const` block — `UI::start_frame` does, which is what makes a
/// framebuffer too small for its policy a **compile error** rather than a
/// runtime check, and what makes a `Frame` whose regions could overflow the
/// surface unobtainable.
///
/// Takes the two surface numbers rather than the renderer type so the proof can
/// be exercised directly, without standing up a whole `Renderer` impl — the
/// doctests below are the real test of the assertion, and they run in this
/// crate's suite.
///
/// ```
/// # use rsact_render::region::{assert_policy_fits, Tiles, Whole};
/// // 240x24 RGB565 needs 5760 u16 — exactly what the buffer holds.
/// const _: () = assert_policy_fits::<Tiles<240, 24>>(5760, 1);
/// // 1-bpp, rows padded to whole bytes: 122px -> 16 bytes, x24 = 384.
/// const _: () = assert_policy_fits::<Tiles<122, 24>>(384, 8);
/// // A full framebuffer is just the degenerate policy.
/// const _: () = assert_policy_fits::<Whole<240, 240>>(57600, 1);
/// ```
///
/// One row too tall does not compile:
///
/// ```compile_fail
/// # use rsact_render::region::{assert_policy_fits, Tiles};
/// // 240x25 needs 6000 units; the surface holds 5760.
/// const _: () = assert_policy_fits::<Tiles<240, 25>>(5760, 1);
/// ```
///
/// Nor does a mono surface sized by area instead of by padded rows — the case
/// that silently corrupts every row after the first:
///
/// ```compile_fail
/// # use rsact_render::region::{assert_policy_fits, Tiles};
/// // 122x24 at 1bpp needs ceil(122/8)*24 = 384 bytes, not 122*24/8 = 366.
/// const _: () = assert_policy_fits::<Tiles<122, 24>>(366, 8);
/// ```
pub const fn assert_policy_fits<P: FramePolicy>(
    surface_units: usize,
    pixels_per_unit: usize,
) {
    assert!(
        region_units(P::MAX_W, P::MAX_H, pixels_per_unit) <= surface_units,
        "the renderer's surface is too small for this frame policy: its \
         largest region does not fit. Shrink the policy's region or enlarge \
         the buffer — the instantiation in this error names both."
    );
}

/// Plan `damage` into the regions to paint, appending them to `out`.
///
/// `out` is a caller-owned buffer rather than a return value so a driver can
/// keep one across frames and reach a steady state with no allocation at all —
/// the shape WS6.4/WS18 want on embedded. It is **cleared** first.
///
/// The result is sorted top-to-bottom, left-to-right (a display's own scan
/// order, and stable output for goldens). Regions may overlap where merging
/// them would cost more than painting the shared pixels twice — see the module
/// docs.
///
/// # Order of operations
///
/// 1. clamp to the viewport, drop what is left with no area;
/// 2. merge to a fixpoint under the area test + the capacity veto;
/// 3. if paint coverage crosses `full_frame_percent`, collapse to the viewport;
/// 4. chunk anything the surface cannot hold (this is where bands come from);
/// 5. sort.
///
/// Steps 2–3 choose; step 4 obeys. Keeping them in that order is what makes the
/// degenerate case fall out: a nearly-full-screen damage set collapses to one
/// rect and *then* becomes strips, rather than being planned as strips up front.
pub fn plan_regions_into(
    damage: &[Rect],
    viewport: Rect,
    limits: &RegionLimits,
    out: &mut Vec<Rect>,
) {
    out.clear();
    if viewport.is_zero_sized() {
        return;
    }

    // (1) Clamp. Damage arrives in absolute coordinates from the paint pass and
    // is not guaranteed to be on-screen: a widget may sit partly outside a
    // clipped parent, and WS6.1's repaint roots union old and new positions.
    for rect in damage {
        let clamped = rect.intersection(&viewport);
        if !clamped.is_zero_sized() {
            out.push(clamped);
        }
    }
    if out.is_empty() {
        return;
    }

    // (2) Merge to a fixpoint. O(n^3) worst case, on an `n` that is the damage
    // count — single digits in practice. A smarter structure here would cost
    // more to maintain than it saves.
    merge_by_area(out, limits);

    // (3) Full-frame guard. This sums *paint* area, which double-counts any
    // surviving overlap — deliberately, since that overlap is painted twice.
    let painted: u64 = out.iter().map(|r| r.size.area() as u64).sum();
    let viewport_area = viewport.size.area() as u64;
    if painted * 100 >= limits.full_frame_percent as u64 * viewport_area {
        out.clear();
        out.push(viewport);
    }

    // (4) Chunk to capacity.
    chunk_to_capacity(out, limits);

    // (5) Scan order.
    out.sort_unstable_by_key(|r| (r.top_left.y, r.top_left.x));
}

/// [`plan_regions_into`] with a fresh `Vec` — the convenience form for tests and
/// for hosts that do not care about steady-state allocation.
pub fn plan_regions(
    damage: &[Rect],
    viewport: Rect,
    limits: &RegionLimits,
) -> Vec<Rect> {
    let mut out = Vec::new();
    plan_regions_into(damage, viewport, limits, &mut out);
    out
}

// -- internals --------------------------------------------------------------

/// Merge pairs that pass the area test until none does.
fn merge_by_area(regions: &mut Vec<Rect>, limits: &RegionLimits) {
    let mut merged_any = true;
    while merged_any {
        merged_any = false;
        'outer: for i in 0..regions.len() {
            for j in (i + 1)..regions.len() {
                if should_merge(regions[i], regions[j], limits) {
                    let merged = regions[i].union(&regions[j]);
                    regions.remove(j);
                    regions[i] = merged;
                    merged_any = true;
                    break 'outer;
                }
            }
        }
    }
}

/// The area test, in integers, **plus the capacity veto**. See the module docs
/// for the measurement behind the threshold, for why the denominator
/// double-counts overlap, and for why a union the surface cannot hold is never
/// merged.
fn should_merge(a: Rect, b: Rect, limits: &RegionLimits) -> bool {
    let union = a.union(&b);
    if !capacity_allows(a, b, union, limits) {
        return false;
    }
    let separate = a.size.area() as u64 + b.size.area() as u64;
    let merged = union.size.area() as u64;
    merged * 100 <= limits.merge_threshold_percent as u64 * separate
}

/// Whether the surface permits merging `a` and `b` into `union`.
///
/// The veto exists because a union the surface cannot hold gets chunked
/// immediately, on the *union's* grid, adding dead space and possibly a cut
/// through a widget neither rect split. **Containment is the one case where none
/// of that applies**, and it must be exempt: when `b ⊆ a` the union *is* `a`, a
/// region already in the plan and already chunked exactly this way. Merging adds
/// no area and no boundary; refusing leaves `b` as a second region whose pixels
/// are then painted twice — once in its own pass, once inside `a`'s chunk.
///
/// Found while building the WS6.4d(1) explainer: `20,20 120×90` with
/// `40,50 16×16` inside it planned as **five** regions under `Tiles<240,24>`
/// (four chunks plus the orphaned speck) where four is correct. Worth stating as
/// a rule, because containment is not a corner case here — it is the shape
/// WS6.1's repaint roots produce every time a widget and its stable ancestor are
/// both damaged.
fn capacity_allows(
    a: Rect,
    b: Rect,
    union: Rect,
    limits: &RegionLimits,
) -> bool {
    if union == a || union == b {
        return true;
    }
    limits.holds(union)
}

/// Cut every region down to at most `max` on each axis, row-major from the
/// region's own top-left so the pieces tile it exactly.
///
/// A zero on either axis would loop forever; treat it as "no limit on that
/// axis", which is the degradation that costs paint rather than hanging.
/// Cut every region down to something the surface can hold, **preserving width
/// wherever possible** so the pieces come out as bands rather than a grid.
///
/// A capacity bound is one number, so the natural cut keeps the region's full
/// width and takes as many rows as fit: `rows = max_units / row_units(width)`.
/// That is both simpler than a fixed grid and strictly better — it never
/// introduces a *vertical* seam, so a widget can only ever be split
/// horizontally, and the pieces stay one `set_window` + one DMA burst each.
///
/// The width fallback below exists for the degenerate case where the surface
/// cannot hold even a single row of this region (a very wide frame with a very
/// small buffer). Then the width is cut first, to the widest row the buffer can
/// hold, and the band arithmetic runs inside that. Keeping it total matters more
/// than optimising it: this is the branch nobody exercises until the one board
/// that hits it.
fn chunk_to_capacity(regions: &mut Vec<Rect>, limits: &RegionLimits) {
    let Some(max_units) = limits.max_units else { return };
    if regions.iter().all(|r| limits.holds(*r)) {
        return;
    }
    let pps = limits.pixels_per_unit.max(1);
    let mut chunked = Vec::with_capacity(regions.len());

    for region in regions.iter() {
        if limits.holds(*region) {
            chunked.push(*region);
            continue;
        }

        // Widest slice whose single row fits, capped at the region's own width.
        // `max_units * pps` is the pixel count one row of storage can address;
        // saturating because that product can be enormous on a host renderer.
        let step_x = region
            .size
            .width
            .min(max_units.saturating_mul(pps).min(u32::MAX as usize) as u32)
            .max(1);
        // Rows of that width that fit. `row` is at least 1 by construction, and
        // `max_units >= row` because `step_x` was chosen to make it so — but
        // clamp anyway rather than risk a zero step and an endless loop.
        let row = region_units(step_x, 1, pps).max(1);
        let step_y = (max_units / row).max(1) as u32;

        let right = region.top_left.x + region.size.width as i32;
        let bottom = region.top_left.y + region.size.height as i32;
        let mut y = region.top_left.y;
        while y < bottom {
            let mut x = region.top_left.x;
            while x < right {
                chunked.push(
                    Rect::new(Point::new(x, y), Size::new(step_x, step_y))
                        .intersection(region),
                );
                x += step_x as i32;
            }
            y += step_y as i32;
        }
    }
    *regions = chunked;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect::new(Point::new(x, y), Size::new(w, h))
    }

    const VIEWPORT: Rect = Rect::new(Point::new(0, 0), Size::new(240, 240));

    /// The default for most tests: tight rects, generous budget, no capacity
    /// limit — so what comes out is the merge policy alone.
    fn tight() -> RegionLimits {
        RegionLimits::whole()
    }

    /// A surface big enough for a `w × h` tile at one unit per pixel, spelled
    /// the way a policy spells it. What the planner is bound by is the unit
    /// COUNT — regions of any shape fitting `w * h` units are legal.
    fn tiles(w: u32, h: u32) -> RegionLimits {
        RegionLimits::tiled(region_units(w, h, 1), 1)
    }

    #[test]
    fn no_damage_plans_no_regions() {
        assert!(plan_regions(&[], VIEWPORT, &tight()).is_empty());
    }

    #[test]
    fn damage_outside_the_viewport_is_clipped_away() {
        let off_screen = rect(300, 300, 16, 16);
        assert!(plan_regions(&[off_screen], VIEWPORT, &tight()).is_empty());

        let straddling = rect(-8, -8, 16, 16);
        assert_eq!(
            plan_regions(&[straddling], VIEWPORT, &tight()),
            vec![rect(0, 0, 8, 8)],
            "the on-screen part survives, the rest is not the display's problem"
        );
    }

    #[test]
    fn far_apart_rects_stay_separate() {
        // WS6.4a's own measurement case: opposite corners, area ratio x23.62.
        let a = rect(0, 0, 16, 16);
        let b = rect(224, 224, 16, 16);
        assert_eq!(plan_regions(&[a, b], VIEWPORT, &tight()), vec![a, b]);
    }

    #[test]
    fn adjacent_rects_merge() {
        // 20 px apart, WS6.4a measured area x1.12 — comfortably under x2.0.
        let a = rect(0, 0, 16, 16);
        let b = rect(36, 0, 16, 16);
        assert_eq!(
            plan_regions(&[a, b], VIEWPORT, &tight()),
            vec![rect(0, 0, 52, 16)]
        );
    }

    #[test]
    fn a_contained_rect_always_merges() {
        // The unconditional half of WS6.4a's overlap corollary, and the shape
        // that actually occurs: WS6.1 damages a widget and the ancestor rect
        // containing it. `union == outer`, so the test passes for any threshold.
        let big = rect(0, 0, 200, 200);
        let speck = rect(10, 10, 2, 2);
        assert_eq!(plan_regions(&[big, speck], VIEWPORT, &tight()), vec![big]);
    }

    #[test]
    fn a_cross_shaped_overlap_is_left_alone() {
        // The counterexample the fuzz found (seed 16) to "overlapping rects
        // always merge", pinned so nobody re-derives the rule from the
        // double-counted denominator. 30 px + 42 px sharing 6 px, but the
        // bounding box is 14x15 = 210 -> x2.92. Merging would paint 210 px to
        // save 6 px of double paint and one region's overhead; the area test
        // says no, and the arithmetic agrees.
        let a = rect(35, 0, 2, 15);
        let b = rect(23, 4, 14, 3);
        let planned = plan_regions(&[a, b], VIEWPORT, &tight());
        assert_eq!(planned.len(), 2, "{planned:?}");
        assert!(planned[0].intersects(&planned[1]), "and they do overlap");
    }

    #[test]
    fn no_planned_region_contains_another() {
        // What survives of the disjointness claim, and it is the property that
        // matters: a region inside another is pure duplicated work, with none of
        // the compensating cheapness a partial overlap can have.
        let damage = [
            rect(0, 0, 40, 40),
            rect(20, 20, 40, 40),
            rect(200, 10, 20, 20),
            rect(100, 100, 60, 60),
            rect(110, 110, 10, 10),
        ];
        let planned = plan_regions(&damage, VIEWPORT, &tight());
        for (i, a) in planned.iter().enumerate() {
            for b in &planned[i + 1..] {
                assert!(
                    a.intersection(b) != *a && a.intersection(b) != *b,
                    "{a:?} contains (or is contained by) {b:?} in {planned:?}"
                );
            }
        }
    }

    #[test]
    fn a_merge_the_surface_cannot_hold_is_not_made() {
        // Two 30x30 rects 40 px apart. The area test likes the merge — 3000 px
        // against 1800 is x1.67, under the x2.0 threshold — but a 32x32 surface
        // holds 1024 units and the 30x100 union needs 3000, so chunking would
        // cut it straight back up on the UNION's grid.
        let a = rect(0, 0, 30, 30);
        let b = rect(0, 70, 30, 30);
        assert_eq!(
            plan_regions(&[a, b], VIEWPORT, &tight()),
            vec![rect(0, 0, 30, 100)],
            "with room to hold it, the area test merges this pair"
        );
        assert_eq!(
            plan_regions(&[a, b], VIEWPORT, &tiles(32, 32)),
            vec![a, b],
            "a 1024-unit surface cannot hold the union, so the merge is vetoed"
        );
    }

    #[test]
    fn capacity_is_a_budget_not_a_shape() {
        // The maintainer's point, as a test. A tile is a byte buffer plus an
        // instruction about where to blit it; the buffer does not care what
        // shape those bytes are. So a 16x38 union — 608 units — is emitted
        // WHOLE by a surface spelled `240x24`, even though it is 14 rows taller
        // than that rectangle. Bounding the shape instead would chunk it at
        // y=24 and slice the lower rect across both pieces for no reason.
        let a = rect(0, 0, 16, 16);
        let b = rect(0, 22, 16, 16);
        let limits = tiles(240, 24);
        let planned = plan_regions(&[a, b], VIEWPORT, &limits);

        assert_eq!(planned, vec![rect(0, 0, 16, 38)], "{planned:?}");
        assert!(planned[0].size.height > 24, "taller than the spelled tile");
        assert!(limits.holds(planned[0]), "and still fits: 608 of 5760 units");
    }

    #[test]
    fn containment_merges_even_when_the_union_does_not_fit() {
        // The capacity veto's one exemption. The union IS the container, which
        // the plan already holds and already chunks this way, so merging costs
        // nothing; vetoing would leave the speck as a second region and paint
        // its 256 px twice — once alone, once inside the container's chunk.
        let container = rect(20, 20, 120, 90);
        let speck = rect(40, 50, 16, 16);
        let limits = tiles(240, 24);

        let planned = plan_regions(&[container, speck], VIEWPORT, &limits);
        assert!(
            !planned.iter().any(|r| *r == speck),
            "the contained rect survived as its own region: {planned:?}"
        );
        // The container's own chunking, capacity-bound: 5760 units / 120 per
        // row = 48 rows, so 90 rows is two bands. A shape-bound 240x24 surface
        // would have made four.
        assert_eq!(planned.len(), 2, "{planned:?}");
        assert!(planned.iter().all(|r| r.size.width == 120));
        let painted: u32 = planned.iter().map(|r| r.size.area()).sum();
        assert_eq!(
            painted,
            container.size.area(),
            "the plan paints more than the container: {planned:?}"
        );
    }

    #[test]
    fn near_full_coverage_collapses_to_the_viewport() {
        // Four quadrants, each one pixel shy of meeting: 99.2% covered.
        let damage = [
            rect(0, 0, 119, 119),
            rect(121, 0, 119, 119),
            rect(0, 121, 119, 119),
            rect(121, 121, 119, 119),
        ];
        assert_eq!(
            plan_regions(&damage, VIEWPORT, &tight()),
            vec![VIEWPORT],
            "at near-full coverage there is nothing left to be tight about"
        );
    }

    #[test]
    fn a_full_screen_plan_chunks_into_bands() {
        // The degenerate case the original WS6.4 design started from, reached
        // rather than chosen: whole-viewport damage + a 240x24 surface.
        let limits = tiles(240, 24);
        let planned = plan_regions(&[VIEWPORT], VIEWPORT, &limits);
        assert_eq!(planned.len(), 10, "240 / 24 = 10 bands: {planned:?}");
        assert_eq!(planned[0], rect(0, 0, 240, 24));
        assert_eq!(planned[9], rect(0, 216, 240, 24));
    }

    #[test]
    fn chunks_tile_their_region_exactly() {
        // A region whose size does not divide evenly: the last chunk is clipped,
        // and the pieces still partition the region with no gap and no overlap.
        let damage = rect(10, 10, 100, 50);
        let limits = tiles(32, 32);
        let planned = plan_regions(&[damage], VIEWPORT, &limits);
        let covered: u32 = planned.iter().map(|r| r.size.area()).sum();
        assert_eq!(
            covered,
            damage.size.area(),
            "chunks must not overlap or gap"
        );
        assert!(planned.iter().all(|r| r.intersection(&damage) == *r));
        assert!(planned.iter().all(|r| limits.holds(*r)));
        // Width-preserving: a capacity bound cuts rows, never columns, so a
        // widget can only ever be split horizontally.
        assert!(
            planned.iter().all(|r| r.size.width == damage.size.width),
            "chunking introduced a vertical seam: {planned:?}"
        );
    }

    #[test]
    fn regions_come_out_in_scan_order() {
        let damage = [
            rect(200, 200, 8, 8),
            rect(8, 8, 8, 8),
            rect(200, 8, 8, 8),
            rect(8, 200, 8, 8),
        ];
        let planned = plan_regions(&damage, VIEWPORT, &tight());
        let mut sorted = planned.clone();
        sorted.sort_unstable_by_key(|r| (r.top_left.y, r.top_left.x));
        assert_eq!(planned, sorted);
    }

    #[test]
    fn the_plan_covers_every_damaged_pixel() {
        // The one property the whole pipeline rests on: a pixel that changed and
        // is not inside some planned region is a pixel that never gets repainted.
        // Checked exhaustively over a small viewport rather than argued.
        let viewport = Rect::new(Point::new(0, 0), Size::new(64, 64));
        let cases: [&[Rect]; 5] = [
            &[rect(0, 0, 1, 1)],
            &[rect(0, 0, 8, 8), rect(56, 56, 8, 8)],
            &[rect(4, 4, 20, 20), rect(10, 10, 20, 20)],
            &[rect(-4, -4, 8, 8), rect(60, 60, 8, 8)],
            &[rect(0, 0, 64, 8), rect(0, 56, 64, 8), rect(28, 28, 8, 8)],
        ];
        for limits in [RegionLimits::whole(), tiles(16, 16), tiles(64, 4)] {
            for damage in cases {
                let planned = plan_regions(damage, viewport, &limits);
                for d in damage {
                    let onscreen = d.intersection(&viewport);
                    for p in onscreen.points() {
                        assert!(
                            planned.iter().any(|r| r.contains(p)),
                            "{p:?} of {d:?} is unpainted under {limits:?}: \
                             {planned:?}"
                        );
                    }
                }
            }
        }
    }

    // A tiny LCG for the plan fuzz (no `rand` dep; deterministic per seed) —
    // same shape as the one WS6.3b's fill fuzz uses.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(1);
            self.0
        }
        fn range(&mut self, n: i32) -> i32 {
            (self.next() % n as u64) as i32
        }
    }

    /// 500 random damage sets against three policies, checking the four
    /// properties every consumer downstream is entitled to assume.
    ///
    /// Coverage is the one that matters: an uncovered damaged pixel is a stale
    /// pixel that stays stale until something else happens to damage it, which
    /// is the failure mode tiling is most likely to produce and the hardest to
    /// notice by eye. It is checked exhaustively per pixel, on a viewport small
    /// enough (48×48) that 500 seeds stay cheap.
    #[test]
    fn the_plan_is_sound_fuzz() {
        let viewport = Rect::new(Point::new(0, 0), Size::new(48, 48));
        let policies = [
            RegionLimits::whole(),
            tiles(16, 16),
            tiles(48, 8),
            RegionLimits::whole(),
        ];

        for seed in 0..500u64 {
            let mut rng = Rng(seed.wrapping_add(1));
            let count = 1 + rng.range(6) as usize;
            // Coordinates deliberately stray off-screen on both sides, so the
            // clamp is exercised rather than assumed.
            let damage: Vec<Rect> = (0..count)
                .map(|_| {
                    Rect::new(
                        Point::new(rng.range(64) - 8, rng.range(64) - 8),
                        Size::new(
                            1 + rng.range(24) as u32,
                            1 + rng.range(24) as u32,
                        ),
                    )
                })
                .collect();

            for limits in &policies {
                let planned = plan_regions(&damage, viewport, limits);
                let ctx = || format!("seed {seed}, {limits:?}, {damage:?}");

                // (a) coverage — every damaged on-screen pixel is painted.
                for d in &damage {
                    for p in d.intersection(&viewport).points() {
                        assert!(
                            planned.iter().any(|r| r.contains(p)),
                            "{p:?} unpainted: {} -> {planned:?}",
                            ctx()
                        );
                    }
                }

                // (b) capacity — nothing needs more storage than the
                // surface holds. A UNIT count, not a shape: a tall narrow
                // region is fine as long as its bytes fit.
                assert!(
                    planned.iter().all(|r| limits.holds(*r)),
                    "region over capacity: {} -> {planned:?}",
                    ctx()
                );

                // (c) no containment — overlap is allowed (and priced), but a
                // region wholly inside another is duplicated work with nothing
                // bought back. Checked only where chunking is off: two chunks
                // from different regions have no such guarantee.
                if limits.max_units.is_none() {
                    for (i, a) in planned.iter().enumerate() {
                        for b in &planned[i + 1..] {
                            let shared = a.intersection(b);
                            assert!(
                                shared != *a && shared != *b,
                                "{a:?} contains {b:?}: {}",
                                ctx()
                            );
                        }
                    }
                }

                // (d) on-screen — a region outside the viewport is a transfer
                // the display will reject or, worse, wrap.
                assert!(
                    planned.iter().all(|r| r.intersection(&viewport) == *r),
                    "region off-screen: {} -> {planned:?}",
                    ctx()
                );
            }
        }
    }

    /// A policy's `MAX_W`/`MAX_H` are what the compile-time capacity proof
    /// checks, so they have to bound what the planner actually emits — including
    /// when the viewport disagrees with them. Checked for both policies, at a
    /// viewport deliberately larger than the declared surface.
    #[test]
    fn a_policy_bounds_what_it_emits() {
        let viewport = Rect::new(Point::zero(), Size::new(320, 240));
        let damage =
            [rect(0, 0, 320, 240), rect(10, 10, 8, 8), rect(300, 230, 8, 8)];

        fn check<P: FramePolicy>(damage: &[Rect], viewport: Rect) {
            let limits = P::limits(viewport.size, 1);
            let budget = region_units(P::MAX_W, P::MAX_H, 1);
            let planned = plan_regions(damage, viewport, &limits);
            assert!(!planned.is_empty());
            for region in &planned {
                // The bound is the UNIT COUNT the compile-time proof was run
                // against — not the rectangle used to spell it. A region may be
                // taller than `MAX_H` provided it is narrow enough to fit.
                assert!(
                    limits.units_of(*region) <= budget,
                    "{region:?} needs {} units, over the {budget} the capacity \
                     proof was run against ({}x{})",
                    limits.units_of(*region),
                    P::MAX_W,
                    P::MAX_H
                );
            }
        }

        check::<Whole<240, 240>>(&damage, viewport);
        check::<Tiles<240, 24>>(&damage, viewport);
        check::<Tiles<32, 32>>(&damage, viewport);
    }

    #[test]
    fn every_planned_region_fits_the_surface() {
        // The other half of the contract: a region larger than the surface is a
        // buffer overrun, not a slow frame.
        let limits = tiles(32, 16);
        let damage =
            [rect(0, 0, 240, 240), rect(5, 5, 33, 5), rect(100, 100, 1, 200)];
        for planned in plan_regions(&damage, VIEWPORT, &limits) {
            assert!(
                limits.holds(planned),
                "{planned:?} needs {} units, over the surface's 512",
                limits.units_of(planned)
            );
        }
    }
}
