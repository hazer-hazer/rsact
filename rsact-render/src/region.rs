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
//! # The capacity veto: a merge chunking would undo is never made
//!
//! The area test alone is not enough once the surface is smaller than the
//! frame, and the first real-page measurement is what showed it. Two 16×16
//! checkboxes 22 px apart merge happily on area (×1.19) into a 16×38 region —
//! which a 240×24 tile cannot hold, so step (5) immediately cuts it back into
//! two. The count is unchanged, the paint is not: the chunk boundary falls on
//! the *merged* region's grid at y = 24, slicing the lower checkbox in half so
//! it is drawn in both chunks. Measured on `tile_plan_240.txt`: 8 required ops
//! merged-then-chunked against 5 left alone.
//!
//! So a union larger than [`RegionLimits::max_region`] is rejected outright,
//! before the area test is even consulted. The rule generalises past this one
//! case — **any merge that capacity will immediately undo can only lose**,
//! because chunking re-derives the split from the union's own origin rather
//! than from where the damage actually was.
//!
//! Capacity also outranks [`RegionLimits::max_regions`]: when no pair can be
//! merged within capacity the budget is simply not met, because a region the
//! buffer cannot hold is a buffer overrun and an extra region is only a slower
//! frame.

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
    /// The largest region the surface can hold, or `None` when it covers the
    /// whole frame (a GPU, a host renderer, a full-size framebuffer).
    ///
    /// `Option` rather than a saturated `Size` because "unbounded" is a real,
    /// common case and `Size::MAX.area()` would overflow the moment anything
    /// asked how big it is. It also lines up one-to-one with
    /// [`Renderer::SURFACE_UNITS`]'s `usize::MAX` default.
    ///
    /// [`Renderer::SURFACE_UNITS`]: crate::renderer::Renderer::SURFACE_UNITS
    pub max_region: Option<Size>,

    /// How many *damage* regions to keep before falling back to merging the
    /// cheapest pairs regardless of the area test.
    ///
    /// This bounds the term WS6.4a found to be the expensive one: every region
    /// is a full tree traversal (×1.19–2.44 per region, worse than the drawing
    /// it guards), so an unbounded damage list costs unbounded walks.
    ///
    /// It does **not** bound the chunks a too-large region is cut into — those
    /// are forced by surface capacity, not chosen, and capping them would mean
    /// emitting a region the output cannot hold. For the same reason it is a
    /// *preference*: where capacity leaves no mergeable pair, the plan comes out
    /// over budget rather than over capacity.
    pub max_regions: usize,

    /// Merge two regions when `union.area * 100 <= threshold * (a.area +
    /// b.area)`, and the union fits [`RegionLimits::max_region`]. `200` is
    /// WS6.4a's measured ×2.0.
    pub merge_threshold_percent: u32,

    /// When the planned regions already *paint* this much of the viewport's
    /// area, give up on being tight and plan the whole viewport as one region
    /// (which [`RegionLimits::max_region`] then chunks into bands).
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
    pub const MERGE_THRESHOLD_PERCENT: u32 = 200;

    /// The whole-surface case: one unbounded region, no chunking. What a GPU or
    /// a full-size framebuffer wants.
    pub const fn whole() -> Self {
        Self {
            max_region: None,
            max_regions: 1,
            merge_threshold_percent: Self::MERGE_THRESHOLD_PERCENT,
            full_frame_percent: 90,
        }
    }

    /// Tiles of at most `max_region`, at most `max_regions` of them before
    /// chunking.
    pub const fn tiled(max_region: Size, max_regions: usize) -> Self {
        Self {
            max_region: Some(max_region),
            max_regions,
            merge_threshold_percent: Self::MERGE_THRESHOLD_PERCENT,
            full_frame_percent: 90,
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
/// [`SURFACE_UNITS`]: crate::renderer::Renderer::SURFACE_UNITS
pub trait FramePolicy {
    /// The widest region this policy will emit.
    const MAX_W: u32;
    /// The tallest region this policy will emit.
    const MAX_H: u32;

    /// The runtime constraints, given the viewport being painted.
    ///
    /// A method rather than more consts because two of the four knobs are
    /// genuinely viewport-relative, and because this is where a policy gets to
    /// be opinionated without growing more type parameters.
    fn limits(viewport: Size) -> RegionLimits;
}

/// One region per frame: the surface covers everything.
///
/// What a GPU wants (one walk, one scissor), what a host renderer wants, and
/// what a full-size framebuffer wants. `W`/`H` are the display's own size, so
/// the capacity proof still runs — it is exactly the check that the
/// full-framebuffer path has a full framebuffer.
///
/// Damage still shrinks the *flush*: the single region is the damage bounding
/// box, not unconditionally the viewport, so an idle-ish frame transfers a
/// small rect even here.
///
/// `W`/`H` bound the emitted region rather than merely describing it, which
/// matters when they and the viewport disagree: a `Whole<240, 240>` policy
/// driving a 320×240 viewport degrades into chunks instead of handing the
/// surface a region 80 px wider than it can hold. The compile-time proof only
/// covers what the *policy* asks for, so the policy has to be honest.
pub struct Whole<const W: u32, const H: u32>;

impl<const W: u32, const H: u32> FramePolicy for Whole<W, H> {
    const MAX_W: u32 = W;
    const MAX_H: u32 = H;

    fn limits(_viewport: Size) -> RegionLimits {
        RegionLimits {
            max_region: Some(Size::new(W, H)),
            ..RegionLimits::whole()
        }
    }
}

/// Regions of at most `W × H`, at most `N` of them before capacity forces more.
///
/// The embedded case. `Tiles<240, 24>` on RGB565 is an 11.25 KiB tile against
/// the 112.5 KiB a 240×240 framebuffer costs — the WS6.4 acceptance target — and
/// `N` bounds the *traversal* term WS6.4a found to be the expensive one (every
/// region is a full tree walk).
///
/// This is also where an app encodes its peripheral's transfer ceiling: nRF52
/// SPIM's `MAXCNT` is a hard limit independent of RAM, and only the app knows
/// it, so it belongs in the policy rather than anywhere in rsact.
pub struct Tiles<const W: u32, const H: u32, const N: usize = 4>;

impl<const W: u32, const H: u32, const N: usize> FramePolicy
    for Tiles<W, H, N>
{
    const MAX_W: u32 = W;
    const MAX_H: u32 = H;

    fn limits(_viewport: Size) -> RegionLimits {
        RegionLimits::tiled(Size::new(W, H), N)
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
/// 3. force merges, cheapest union-growth first, until `max_regions` is met;
/// 4. if coverage crosses `full_frame_percent`, collapse to the viewport;
/// 5. chunk anything larger than `max_region` (this is where bands come from);
/// 6. sort.
///
/// Steps 2–4 choose; step 5 obeys. Keeping them in that order is what makes the
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
    // count — single digits in practice, and `max_regions` bounds what survives
    // anyway. A smarter structure here would cost more to maintain than it saves.
    merge_by_area(out, limits);

    // (3) Over budget: merge the pair whose union grows least, repeatedly. This
    // ignores the area test by design — the test asks "is merging cheaper?",
    // this asks "which merge hurts least?", and at this point one is mandatory.
    //
    // The budget can be genuinely unsatisfiable once capacity is in play (six
    // 16x16 rects 22 px apart cannot be merged into anything a 240x24 surface
    // holds). Capacity wins: `cheapest_pair` then finds no candidate and the
    // loop stops over budget rather than emitting a region that overruns the
    // buffer.
    while out.len() > limits.max_regions.max(1) {
        let Some((i, j)) = cheapest_pair(out, limits.max_region) else { break };
        let merged = out[i].union(&out[j]);
        // Remove the higher index first so the lower one stays valid.
        out.remove(j);
        out[i] = merged;
        // A forced merge can bring the result within reach of the area test for
        // other regions, so re-run it rather than only shrinking the count.
        merge_by_area(out, limits);
    }

    // (4) Full-frame guard. This sums *paint* area, which double-counts any
    // surviving overlap — deliberately, since that overlap is painted twice.
    let painted: u64 = out.iter().map(|r| r.size.area() as u64).sum();
    let viewport_area = viewport.size.area() as u64;
    if painted * 100 >= limits.full_frame_percent as u64 * viewport_area {
        out.clear();
        out.push(viewport);
    }

    // (5) Chunk to capacity.
    if let Some(max_region) = limits.max_region {
        chunk_to_capacity(out, max_region);
    }

    // (6) Scan order.
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
    if let Some(max) = limits.max_region {
        if !fits(union, max) {
            return false;
        }
    }
    let separate = a.size.area() as u64 + b.size.area() as u64;
    let merged = union.size.area() as u64;
    merged * 100 <= limits.merge_threshold_percent as u64 * separate
}

/// The pair whose union adds the least dead space *and still fits the surface*,
/// or `None` when no such pair exists (fewer than two regions, or every union
/// is over capacity).
fn cheapest_pair(
    regions: &[Rect],
    max_region: Option<Size>,
) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize, u64)> = None;
    for i in 0..regions.len() {
        for j in (i + 1)..regions.len() {
            let union = regions[i].union(&regions[j]);
            if max_region.is_some_and(|max| !fits(union, max)) {
                continue;
            }
            let separate =
                regions[i].size.area() as u64 + regions[j].size.area() as u64;
            let growth = (union.size.area() as u64).saturating_sub(separate);
            if best.is_none_or(|(_, _, b)| growth < b) {
                best = Some((i, j, growth));
            }
        }
    }
    best.map(|(i, j, _)| (i, j))
}

/// Cut every region down to at most `max` on each axis, row-major from the
/// region's own top-left so the pieces tile it exactly.
///
/// A zero on either axis would loop forever; treat it as "no limit on that
/// axis", which is the degradation that costs paint rather than hanging.
fn chunk_to_capacity(regions: &mut Vec<Rect>, max: Size) {
    if regions.iter().all(|r| fits(*r, max)) {
        return;
    }
    let mut chunked = Vec::with_capacity(regions.len());
    for region in regions.iter() {
        if fits(*region, max) {
            chunked.push(*region);
            continue;
        }
        let step_x = if max.width == 0 { region.size.width } else { max.width };
        let step_y =
            if max.height == 0 { region.size.height } else { max.height };
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

fn fits(region: Rect, max: Size) -> bool {
    (max.width == 0 || region.size.width <= max.width)
        && (max.height == 0 || region.size.height <= max.height)
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
        RegionLimits { max_regions: 8, ..RegionLimits::whole() }
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
        // The real-page case from `tile_plan_240.txt`: two 16x16 checkboxes 22 px
        // apart. On area alone they merge (x1.19); the 16x38 union does not fit a
        // 240x24 tile, so chunking would cut it at y=24 and slice the lower
        // checkbox in two — 8 required ops instead of 5.
        let a = rect(0, 0, 16, 16);
        let b = rect(0, 22, 16, 16);
        assert_eq!(
            plan_regions(&[a, b], VIEWPORT, &tight()),
            vec![rect(0, 0, 16, 38)],
            "with room to hold it, the area test merges this pair"
        );
        assert_eq!(
            plan_regions(
                &[a, b],
                VIEWPORT,
                &RegionLimits::tiled(Size::new(240, 24), 8)
            ),
            vec![a, b],
            "a 240x24 surface cannot hold the union, so the merge is vetoed"
        );
    }

    #[test]
    fn capacity_outranks_the_region_budget() {
        // Six rects that no pair can merge within a 240x24 surface. The budget of
        // 2 is then unsatisfiable, and being over budget (a slower frame) is the
        // right way to fail — over capacity is a buffer overrun.
        let damage: Vec<Rect> =
            (0..6).map(|i| rect(0, i * 22, 16, 16)).collect();
        let limits = RegionLimits::tiled(Size::new(240, 24), 2);
        let planned = plan_regions(&damage, VIEWPORT, &limits);
        assert_eq!(planned, damage, "{planned:?}");
    }

    #[test]
    fn the_budget_forces_merges_the_area_test_rejected() {
        // Five scattered specks the area test would never join.
        let damage = [
            rect(0, 0, 8, 8),
            rect(232, 0, 8, 8),
            rect(0, 232, 8, 8),
            rect(232, 232, 8, 8),
            rect(116, 116, 8, 8),
        ];
        let limits = RegionLimits { max_regions: 2, ..tight() };
        let planned = plan_regions(&damage, VIEWPORT, &limits);
        assert!(planned.len() <= 2, "budget of 2 not respected: {planned:?}");
        for d in &damage {
            assert!(
                planned.iter().any(|r| r.intersection(d) == *d),
                "forced merges must still cover {d:?}, got {planned:?}"
            );
        }
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
        let limits = RegionLimits::tiled(Size::new(240, 24), 8);
        let planned = plan_regions(&[VIEWPORT], VIEWPORT, &limits);
        assert_eq!(planned.len(), 10, "240 / 24 = 10 bands: {planned:?}");
        assert_eq!(planned[0], rect(0, 0, 240, 24));
        assert_eq!(planned[9], rect(0, 216, 240, 24));
    }

    #[test]
    fn chunking_is_not_capped_by_the_region_budget() {
        // `max_regions` bounds chosen regions; chunks are forced by capacity, and
        // capping them would emit a region the surface cannot hold.
        let limits = RegionLimits::tiled(Size::new(240, 24), 2);
        let planned = plan_regions(&[VIEWPORT], VIEWPORT, &limits);
        assert_eq!(planned.len(), 10);
        assert!(planned.iter().all(|r| r.size.height <= 24));
    }

    #[test]
    fn chunks_tile_their_region_exactly() {
        // A region whose size does not divide evenly: the last chunk is clipped,
        // and the pieces still partition the region with no gap and no overlap.
        let damage = rect(10, 10, 100, 50);
        let limits = RegionLimits::tiled(Size::new(32, 32), 8);
        let planned = plan_regions(&[damage], VIEWPORT, &limits);
        let covered: u32 = planned.iter().map(|r| r.size.area()).sum();
        assert_eq!(
            covered,
            damage.size.area(),
            "chunks must not overlap or gap"
        );
        assert!(planned.iter().all(|r| r.intersection(&damage) == *r));
        assert!(
            planned
                .iter()
                .all(|r| r.size.width <= 32 && r.size.height <= 32)
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
        for limits in [
            RegionLimits { max_regions: 8, ..RegionLimits::whole() },
            RegionLimits::tiled(Size::new(16, 16), 4),
            RegionLimits::tiled(Size::new(64, 4), 2),
        ] {
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
            RegionLimits { max_regions: 8, ..RegionLimits::whole() },
            RegionLimits::tiled(Size::new(16, 16), 4),
            RegionLimits::tiled(Size::new(48, 8), 3),
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

                // (b) capacity — nothing exceeds the surface.
                if let Some(max) = limits.max_region {
                    assert!(
                        planned.iter().all(|r| r.size.width <= max.width
                            && r.size.height <= max.height),
                        "region over capacity: {} -> {planned:?}",
                        ctx()
                    );
                }

                // (c) no containment — overlap is allowed (and priced), but a
                // region wholly inside another is duplicated work with nothing
                // bought back. Checked only where chunking is off: two chunks
                // from different regions have no such guarantee.
                if limits.max_region.is_none() {
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

                // (e) budget, where chunking is not in play to inflate it.
                if limits.max_region.is_none() {
                    assert!(
                        planned.len() <= limits.max_regions.max(1),
                        "over budget: {} -> {planned:?}",
                        ctx()
                    );
                }
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
            let planned =
                plan_regions(damage, viewport, &P::limits(viewport.size));
            assert!(!planned.is_empty());
            for region in &planned {
                assert!(
                    region.size.width <= P::MAX_W
                        && region.size.height <= P::MAX_H,
                    "{region:?} exceeds the declared {}x{} the capacity proof \
                     was run against",
                    P::MAX_W,
                    P::MAX_H
                );
            }
        }

        check::<Whole<240, 240>>(&damage, viewport);
        check::<Tiles<240, 24>>(&damage, viewport);
        check::<Tiles<32, 32, 2>>(&damage, viewport);
    }

    #[test]
    fn every_planned_region_fits_the_surface() {
        // The other half of the contract: a region larger than the surface is a
        // buffer overrun, not a slow frame.
        let max = Size::new(32, 16);
        let limits = RegionLimits::tiled(max, 8);
        let damage =
            [rect(0, 0, 240, 240), rect(5, 5, 33, 5), rect(100, 100, 1, 200)];
        for planned in plan_regions(&damage, VIEWPORT, &limits) {
            assert!(
                planned.size.width <= max.width
                    && planned.size.height <= max.height,
                "{planned:?} exceeds the surface {max:?}"
            );
        }
    }
}
