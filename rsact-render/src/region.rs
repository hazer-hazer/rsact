//! Turning a frame's damage rects into the regions it is painted in.
//!
//! Pure geometry — no renderer, no buffer, no widgets. [`plan_regions`] takes
//! what changed and what the surface can hold ([`RegionLimits`]) and answers
//! which rectangles to paint, sorted top-to-bottom, left-to-right.
//!
//! Three properties of the output:
//!
//! - **Everything intersecting a region repaints**, changed or not, so region
//!   shape decides how much work a frame is: a tight 16×16 rect around one
//!   checkbox costs 3 draw ops where a 240×24 band containing it costs 118.
//! - **Regions may overlap**, so a pixel can be painted twice. Waste, never
//!   corruption — painting is a pure function of position.
//! - **Nearby rects merge**, then anything too big for the surface is cut into
//!   full-width bands, so near-full-screen damage comes back out as strips.
//!
//! A surface bound is a **unit count, not a shape**: `Tiles<240, 24>` means "a
//! buffer big enough for a 240×24 tile", so a 16×38 region is emitted whole.

use crate::{
    geometry::{Point, Rect, Size},
    renderer::region_units,
};
use alloc::vec::Vec;

/// A [`FramePolicy`]'s constraints as the numbers [`plan_regions`] needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionLimits {
    /// Storage units the surface holds, or `None` when it covers the frame.
    pub max_units: Option<usize>,

    /// Pixels per storage unit, so a candidate region can be costed in units
    /// without knowing its color.
    pub pixels_per_unit: usize,

    /// Merge two regions when `union.area * 100 <= threshold * (a.area +
    /// b.area)` and the union fits [`RegionLimits::max_units`].
    pub merge_threshold_percent: u32,
}

impl RegionLimits {
    /// The measured merge threshold: area ratio ×2.0.
    ///
    /// TODO: likely the wrong *shape*. Costing a plan as `N·F + p·Σarea` makes a
    /// merge win when the dead space it adds is under `F/p` — an absolute pixel
    /// count (roughly 25–75 px for an ST7789 at 40 MHz), where a ratio scales
    /// the allowance with the size of the rects. Wants a measured `F/p` per
    /// target.
    pub const MERGE_THRESHOLD_PERCENT: u32 = 200;

    /// Unbounded: one region, no chunking.
    pub const fn whole() -> Self {
        Self {
            max_units: None,
            pixels_per_unit: 1,
            merge_threshold_percent: Self::MERGE_THRESHOLD_PERCENT,
        }
    }

    /// A surface of `max_units` units, `pixels_per_unit` pixels each.
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

/// How large a region a renderer will accept — **a type**, so a surface too
/// small is rejected at compile time. Declared as [`Renderer::Policy`];
/// implement it on a zero-sized type.
///
/// A `W × H` policy declares the **capacity** that rectangle implies, not its
/// shape: under `Tiles<240, 24>` a 16×38 region (608 units) and a 120×48 one
/// (5760) are both legal and neither is chunked.
///
/// [`Renderer::Policy`]: crate::renderer::Renderer::Policy
pub trait FramePolicy {
    /// The largest region this policy may ask for, `None` for no bound. A
    /// distinct case rather than a huge `Size`, which would overflow the
    /// capacity arithmetic.
    const MAX_REGION: Option<Size>;

    /// Pixels the surface packs into one storage unit — what turns
    /// [`MAX_REGION`](Self::MAX_REGION) into a unit count. `1` for any color a
    /// storage word holds whole. `attach` asserts it against the color's own
    /// packing, so the two cannot drift.
    const PIXELS_PER_UNIT: usize = 1;

    /// The runtime constraints, given the viewport.
    fn limits(viewport: Size) -> RegionLimits;
}

/// Storage units policy `P`'s largest region needs, or `None` if [`Unbounded`].
/// What a backend compares its surface against.
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
/// `out` is **cleared** first; reusing one across frames reaches a steady state
/// with no allocation. Output is sorted in scan order and may overlap.
///
/// Clamp to the viewport, merge to a fixpoint, chunk what the surface cannot
/// hold, sort. Merging before chunking is what makes near-full-screen damage
/// collapse to one rect and *then* become strips.
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

    // (1) Clamp: damage is absolute and not guaranteed on-screen.
    for rect in damage {
        let clamped = rect.intersection(&viewport);
        if !clamped.is_zero_sized() {
            out.push(clamped);
        }
    }
    if out.is_empty() {
        return;
    }

    // (2) Merge to a fixpoint. O(n^3) worst case on the damage count, which is
    // single digits in practice.
    merge_by_area(out, limits);

    // (3) Chunk to capacity.
    chunk_to_capacity(out, limits);

    // (4) Scan order.
    out.sort_unstable_by_key(|r| (r.top_left.y, r.top_left.x));
}

/// [`plan_regions_into`] with a fresh `Vec`.
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

/// The area test in integers, plus the capacity veto. The denominator
/// double-counts any overlap deliberately: `a + b` is what painting them
/// separately costs, overlap included.
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
/// A union too big is chunked on the *union's* grid, so merging it can only
/// lose. **Containment is exempt**: when `b ⊆ a` the union *is* `a`, already in
/// the plan and already chunked, and refusing would leave `b` as a second region
/// painted twice. Common — a widget and its stable ancestor are damaged
/// together every time.
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

/// Cut every region to something the surface can hold, **preserving width**, so
/// the pieces are bands rather than a grid. Width is only cut when the surface
/// cannot hold even one row of the region.
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

        // Widest slice whose single row fits. Saturating: `max_units * pps` can
        // be enormous on a host renderer.
        let step_x = region
            .size
            .width
            .min(max_units.saturating_mul(pps).min(u32::MAX as usize) as u32)
            .max(1);
        // Rows of that width that fit. Clamped: a zero step would loop forever.
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

    /// No capacity limit, so what comes out is the merge policy alone.
    fn tight() -> RegionLimits {
        RegionLimits::whole()
    }

    /// A surface big enough for a `w × h` tile at one unit per pixel.
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
        // `union == outer`, so this merges under any threshold.
        let big = rect(0, 0, 200, 200);
        let speck = rect(10, 10, 2, 2);
        assert_eq!(plan_regions(&[big, speck], VIEWPORT, &tight()), vec![big]);
    }

    #[test]
    fn a_cross_shaped_overlap_is_left_alone() {
        // Overlapping rects do NOT always merge (fuzz seed 16): 30 px + 42 px
        // sharing 6 px, but a 14x15 = 210 px bounding box, so x2.92.
        let a = rect(35, 0, 2, 15);
        let b = rect(23, 4, 14, 3);
        let planned = plan_regions(&[a, b], VIEWPORT, &tight());
        assert_eq!(planned.len(), 2, "{planned:?}");
        assert!(planned[0].intersects(&planned[1]), "and they do overlap");
    }

    #[test]
    fn no_planned_region_contains_another() {
        // A region inside another is pure duplicated work, unlike a partial
        // overlap.
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
        // The area test likes it (3000 against 1800, x1.67) but a 32x32 surface
        // holds 1024 units and the 30x100 union needs 3000.
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
        // A 16x38 union is 608 units, so a surface spelled `240x24` emits it
        // whole despite it being 14 rows taller than that rectangle.
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
        // The union IS the container, already in the plan and already chunked
        // this way, so vetoing would paint the speck's 256 px twice.
        let container = rect(20, 20, 120, 90);
        let speck = rect(40, 50, 16, 16);
        let limits = tiles(240, 24);

        let planned = plan_regions(&[container, speck], VIEWPORT, &limits);
        assert!(
            !planned.iter().any(|r| *r == speck),
            "the contained rect survived as its own region: {planned:?}"
        );
        // 5760 units / 120 per row = 48 rows, so 90 rows is two bands.
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
        // The area test collapses this without needing a coverage threshold.
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
        // Whole-viewport damage + a 240x24 surface: a strip renderer.
        let limits = tiles(240, 24);
        let planned = plan_regions(&[VIEWPORT], VIEWPORT, &limits);
        assert_eq!(planned.len(), 10, "240 / 24 = 10 bands: {planned:?}");
        assert_eq!(planned[0], rect(0, 0, 240, 24));
        assert_eq!(planned[9], rect(0, 216, 240, 24));
    }

    #[test]
    fn chunks_tile_their_region_exactly() {
        // Uneven division: the last chunk is clipped, and the pieces still
        // partition the region exactly.
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
        // A capacity bound cuts rows, never columns.
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
        // A damaged pixel outside every planned region never gets repainted.
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

    // A tiny LCG: no `rand` dep, deterministic per seed.
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

    /// 500 random damage sets against three policies, checking coverage,
    /// capacity, non-containment and on-screen-ness.
    ///
    /// Coverage is the one that matters — an uncovered damaged pixel stays stale
    /// until something else damages it — so it is checked per pixel, on a
    /// viewport small enough that 500 seeds stay cheap.
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
            // Strays off-screen on both sides, exercising the clamp.
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

                // (b) capacity — in units, so a tall narrow region is fine.
                assert!(
                    planned.iter().all(|r| limits.holds(*r)),
                    "region over capacity: {} -> {planned:?}",
                    ctx()
                );

                // (c) no containment. Only where chunking is off: two chunks
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

                // (d) on-screen — the display would reject or wrap it.
                assert!(
                    planned.iter().all(|r| r.intersection(&viewport) == *r),
                    "region off-screen: {} -> {planned:?}",
                    ctx()
                );
            }
        }
    }

    /// A policy's declared size bounds what the planner emits, including when
    /// the viewport is larger than it.
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
                // In units, not the rectangle used to spell them: a region may
                // exceed `max.height` if it is narrow enough to fit.
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
        // A region larger than the surface is an overrun, not a slow frame.
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
