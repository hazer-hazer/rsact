//! Turning a frame's damage rects into the regions it is painted in.
//!
//! Pure geometry — no renderer, no buffer, no widgets. [`plan_regions`] takes
//! what changed and what the surface can hold ([`RegionLimits`]) and answers
//! which rectangles to paint, sorted top-to-bottom, left-to-right.
//!
//! Three things to know about the output:
//!
//! - **Everything intersecting a region repaints**, changed or not. A region
//!   arrives holding whatever the last one left in it, so region shape decides
//!   how much work a frame is: a tight 16×16 rect around one checkbox costs 3
//!   draw ops where a 240×24 full-width band containing it costs 118, having
//!   caught every neighbour on those rows.
//! - **Regions may overlap**, so a pixel can be painted twice. That is waste,
//!   never corruption — painting is a pure function of position — and it is
//!   waste the planner has already priced against the cost of splitting.
//! - **Nearby rects are merged** when their union's area is within
//!   [`merge_threshold_percent`](RegionLimits::merge_threshold_percent) of the
//!   sum of their own, and a region too big for the surface is then cut into
//!   full-width bands. Damage covering most of the screen therefore collapses
//!   to one rect and comes back out as strips.
//!
//! # A tile is a budget, not a shape
//!
//! The surface constraint is a **unit count**. A buffer does not care whether
//! its 5760 units are laid out 240×24, 120×48 or 16×38, so `Tiles<240, 24>`
//! reads *"a buffer big enough for a 240×24 tile"* — not *"regions are at most
//! 240×24"* — and a 16×38 damage region is emitted whole.

use crate::{
    geometry::{Point, Rect, Size},
    renderer::region_units,
};
use alloc::vec::Vec;

/// What the output path can accept — the frame policy's constraints, flattened
/// into the numbers the planner needs.
///
/// Separate from the policy *type*: that proves at compile time that the
/// surface can hold `max_region`, while planning is ordinary runtime geometry
/// with no reason to be generic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionLimits {
    /// Storage units the surface holds, or `None` when it covers the whole
    /// frame (a GPU, a host renderer, a full-size framebuffer).
    ///
    /// A capacity and not a shape — see the module docs. It is also the shape
    /// the hardware ceilings have: `CASET`/`RASET` take arbitrary rects, and
    /// nRF52 SPIM's `MAXCNT` limits a transfer's byte count rather than its
    /// rectangle.
    pub max_units: Option<usize>,

    /// How the surface packs pixels into storage units, carried here so the
    /// planner can convert a candidate region into units without knowing
    /// anything about color — or about surfaces, which rsact-ui never sees.
    pub pixels_per_unit: usize,

    /// Merge two regions when `union.area * 100 <= threshold * (a.area +
    /// b.area)` and the union fits [`RegionLimits::max_units`].
    pub merge_threshold_percent: u32,
}

impl RegionLimits {
    /// The measured merge threshold: area ratio ×2.0.
    ///
    /// TODO: probably the wrong *shape*, not just the wrong value. Costing a
    /// plan as `N·F + p·Σarea` (F = per-region fixed cost, p = per-pixel) makes
    /// a merge win exactly when the dead space it adds is under `F/p` — an
    /// absolute pixel count, where a ratio scales the allowance with the size of
    /// the rects being merged. For an ST7789 at 40 MHz, `F/p` is roughly 25–75
    /// px, nowhere near "double the area". The replacement is a measured `F/p`
    /// per target, which is a `RegionPolicy` parameter.
    pub const MERGE_THRESHOLD_PERCENT: u32 = 200;

    /// The whole-surface case: one unbounded region, no chunking. What a GPU or
    /// a full-size framebuffer wants.
    pub const fn whole() -> Self {
        Self {
            max_units: None,
            pixels_per_unit: 1,
            merge_threshold_percent: Self::MERGE_THRESHOLD_PERCENT,
        }
    }

    /// A surface of `max_units` storage units packing `pixels_per_unit` pixels
    /// each.
    pub const fn tiled(max_units: usize, pixels_per_unit: usize) -> Self {
        Self {
            max_units: Some(max_units),
            pixels_per_unit,
            merge_threshold_percent: Self::MERGE_THRESHOLD_PERCENT,
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

/// How a frame is cut into regions — **a type, not a value**, so that a
/// surface too small for what the renderer will ask of it is rejected at
/// compile time. See [`Renderer::Policy`], where a renderer declares the one it
/// obeys.
///
/// Implement it on a zero-sized type; the two below cover the cases that exist.
///
/// A policy names a `W × H` rectangle because that is what a person can picture
/// and what the error should say, but what it declares is the **capacity** that
/// rectangle implies. The planner may emit any region needing no more units:
/// under `Tiles<240, 24>` both a 16×38 region (608 units) and a 120×48 one
/// (5760) are legal, and neither is chunked.
///
/// [`Renderer::Policy`]: crate::renderer::Renderer::Policy
pub trait FramePolicy {
    /// The largest region this policy may ask a renderer to paint, or `None`
    /// for "no bound at all".
    ///
    /// `None` is [`Unbounded`] — a GPU, a host renderer, any surface that always
    /// covers the frame. A distinct case rather than a very large `Size`: the
    /// capacity arithmetic would overflow long before `u32::MAX × u32::MAX` meant
    /// anything, and the check must *skip*, not merely pass.
    const MAX_REGION: Option<Size>;

    /// How many pixels the surface this policy describes packs into one storage
    /// unit.
    ///
    /// `1` is right for every 8-bit-or-wider color and for any surface that
    /// does not pack. It lives on the *policy* because it is only consulted
    /// alongside [`MAX_REGION`](Self::MAX_REGION), being what turns a declared
    /// `W × H` into a unit count.
    ///
    /// The row padding it implies is why capacity is not simply `w * h`: a
    /// 122-pixel 1-bpp row occupies 16 bytes, not 15.25 (see [`region_units`]).
    ///
    /// No packed policy ships yet — a packed surface small enough to pack is
    /// usually small enough not to need tiling. One would be a ZST setting this
    /// to `8`; `attach` asserts it against the color's own packing, so the two
    /// cannot drift.
    const PIXELS_PER_UNIT: usize = 1;

    /// The runtime constraints, given the viewport.
    ///
    /// A method rather than more consts because the knobs are viewport-relative,
    /// and because this is where a policy gets to be opinionated without growing
    /// more type parameters.
    fn limits(viewport: Size) -> RegionLimits;
}

/// Storage units policy `P`'s largest region needs, or `None` if `P` is
/// [`Unbounded`].
///
/// The one conversion from a policy to a capacity requirement — a backend
/// compares its surface against this, in a `const` block when the surface is a
/// fixed-size array and at `attach` time when it is a runtime-length slice.
pub const fn policy_units<P: FramePolicy>() -> Option<usize> {
    match P::MAX_REGION {
        None => None,
        Some(max) => {
            Some(region_units(max.width, max.height, P::PIXELS_PER_UNIT))
        },
    }
}

/// No capacity bound: the surface always covers the frame.
///
/// The policy for every renderer that never needed tiling — a GPU, a host
/// renderer with a resizable buffer, [`NullRenderer`], a full-framebuffer
/// backend — so the planner may stop chunking entirely.
///
/// [`NullRenderer`]: crate::renderer::NullRenderer
pub struct Unbounded;

impl FramePolicy for Unbounded {
    const MAX_REGION: Option<Size> = None;

    fn limits(_viewport: Size) -> RegionLimits {
        RegionLimits::whole()
    }
}

/// A surface the size of a `W × H` region — the embedded case. On RGB565,
/// `Tiles<240, 24>` is an 11.25 KiB buffer against the 112.5 KiB a 240×240
/// framebuffer costs.
///
/// Read it as *"a buffer big enough for a 240×24 tile"*, not *"regions are at
/// most 240×24"*: a 16×38 damage region needs 608 of those 5760 units and is
/// emitted whole.
///
/// `W`/`H` **bound** the emitted region rather than describing it, which matters
/// when they and the viewport disagree: a 240×240 policy driving a 320×240
/// viewport degrades into bands rather than handing the surface a frame 25%
/// larger than it can hold. The compile-time proof only covers what the policy
/// asks for, so the policy has to be honest.
///
/// It is also where an app encodes its peripheral's transfer ceiling — nRF52
/// SPIM's `MAXCNT` is a byte count independent of RAM, which is exactly what
/// this declares, and only the app knows it.
pub struct Tiles<const W: u32, const H: u32>;

impl<const W: u32, const H: u32> FramePolicy for Tiles<W, H> {
    const MAX_REGION: Option<Size> = Some(Size::new(W, H));

    fn limits(_viewport: Size) -> RegionLimits {
        RegionLimits::tiled(
            region_units(W, H, Self::PIXELS_PER_UNIT),
            Self::PIXELS_PER_UNIT,
        )
    }
}

/// Assert that a surface of `surface_units` units holds policy `P`'s largest
/// region, failing with a message naming both.
///
/// `const`, so a backend whose surface is a fixed-size array can call it in a
/// `const` block and make "this buffer is too small for its policy" a compile
/// error. One handed a runtime-length slice has no such const and calls it at
/// `attach` instead — same arithmetic, same message.
///
/// [`Unbounded`] always fits, and short-circuits before any arithmetic.
///
/// ```
/// # use rsact_render::region::{assert_policy_fits, Tiles, Unbounded};
/// // 240x24 RGB565 needs 5760 u16 — exactly what the buffer holds.
/// const _: () = assert_policy_fits::<Tiles<240, 24>>(5760);
/// // A full framebuffer is just the degenerate policy.
/// const _: () = assert_policy_fits::<Tiles<240, 240>>(57600);
/// // An unbounded policy imposes nothing, so even nothing satisfies it.
/// const _: () = assert_policy_fits::<Unbounded>(0);
/// ```
///
/// One row too tall does not compile:
///
/// ```compile_fail
/// # use rsact_render::region::{assert_policy_fits, Tiles};
/// // 240x25 needs 6000 units; the surface holds 5760.
/// const _: () = assert_policy_fits::<Tiles<240, 25>>(5760);
/// ```
///
/// Nor does a surface one row short:
///
/// ```compile_fail
/// # use rsact_render::region::{assert_policy_fits, Tiles};
/// const _: () = assert_policy_fits::<Tiles<240, 24>>(5759);
/// ```
pub const fn assert_policy_fits<P: FramePolicy>(surface_units: usize) {
    if let Some(needed) = policy_units::<P>() {
        assert!(
            needed <= surface_units,
            "the renderer's surface is too small for this frame policy: its \
             largest region does not fit. Shrink the policy's region or \
             enlarge the buffer — the instantiation in this error names both."
        );
    }
}

/// Plan `damage` into the regions to paint, appending them to `out`.
///
/// `out` is a caller-owned buffer rather than a return value so a driver can
/// keep one across frames and reach a steady state with no allocation at all.
/// It is **cleared** first.
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
/// 3. chunk anything the surface cannot hold (this is where bands come from);
/// 4. sort.
///
/// Step 2 chooses; step 3 obeys. Keeping them in that order is what makes the
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
    // clipped parent, and repaint roots union old and new positions.
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

    // (3) Chunk to capacity.
    chunk_to_capacity(out, limits);

    // (4) Scan order.
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
/// A union the surface cannot hold is chunked immediately, on the *union's*
/// grid, adding dead space and possibly a cut through a widget neither rect
/// split. **Containment is exempt**: when `b ⊆ a` the union *is* `a`, a region
/// already in the plan and already chunked this way, so merging adds no area
/// and no boundary — while refusing leaves `b` as a second region whose pixels
/// are painted twice.
///
/// Not a corner case: it is the shape repaint roots produce every time a widget
/// and its stable ancestor are both damaged. Without the exemption,
/// `20,20 120×90` containing `40,50 16×16` plans as five regions under
/// `Tiles<240,24>` — four chunks plus the orphaned speck — where four is right.
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

/// Cut every region down to something the surface can hold, **preserving width
/// wherever possible** so the pieces come out as bands rather than a grid:
/// `rows = max_units / row_units(width)`.
///
/// The width fallback covers the degenerate case where the surface cannot hold
/// even one row of the region — a very wide frame with a very small buffer.
/// The width is cut first, to the widest row that fits, and the band arithmetic
/// runs inside that.
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
        // Opposite corners: 57600 px of union against 512 of rect, x112.5.
        let a = rect(0, 0, 16, 16);
        let b = rect(224, 224, 16, 16);
        assert_eq!(plan_regions(&[a, b], VIEWPORT, &tight()), vec![a, b]);
    }

    #[test]
    fn adjacent_rects_merge() {
        // 20 px apart: 832 against 512, x1.62 — under the x2.0 threshold.
        let a = rect(0, 0, 16, 16);
        let b = rect(36, 0, 16, 16);
        assert_eq!(
            plan_regions(&[a, b], VIEWPORT, &tight()),
            vec![rect(0, 0, 52, 16)]
        );
    }

    #[test]
    fn a_contained_rect_always_merges() {
        // The unconditional half of the overlap rule, and the shape
        // that actually occurs: repaint roots damage a widget and the ancestor rect
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
    fn near_full_coverage_reaches_the_viewport_on_its_own() {
        // Four quadrants, each a pixel shy of meeting: 98.3% of the screen.
        //
        // No "damage is basically everything" threshold is needed: the area
        // test arrives here on its own, pricing each merge rather than
        // tripping on a percentage.
        let damage = [
            rect(0, 0, 119, 119),
            rect(121, 0, 119, 119),
            rect(0, 121, 119, 119),
            rect(121, 121, 119, 119),
        ];
        assert_eq!(
            plan_regions(&damage, VIEWPORT, &tight()),
            vec![VIEWPORT],
            "the union is x1.017 the parts, so the area test merges it"
        );
    }

    #[test]
    fn a_full_screen_plan_chunks_into_bands() {
        // The degenerate case a strip renderer starts from, reached
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
    // same shape the fill fuzz uses.
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
            let limits = P::limits(viewport.size);
            let max = P::MAX_REGION.expect("a bounded policy");
            let budget = policy_units::<P>().expect("a bounded policy");
            let planned = plan_regions(damage, viewport, &limits);
            assert!(!planned.is_empty());
            for region in &planned {
                // The bound is the UNIT COUNT the capacity proof was run
                // against — not the rectangle used to spell it. A region may be
                // taller than `max.height` provided it is narrow enough to fit.
                assert!(
                    limits.units_of(*region) <= budget,
                    "{region:?} needs {} units, over the {budget} the capacity \
                     proof was run against ({}x{})",
                    limits.units_of(*region),
                    max.width,
                    max.height
                );
            }
        }

        check::<Tiles<240, 240>>(&damage, viewport);
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
