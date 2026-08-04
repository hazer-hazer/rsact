//! WS6.4a: measurement + invariance arithmetic over a **tile schedule** — one
//! frame replayed region by region instead of once over the whole viewport.
//!
//! This is the instrument WS6.4d is gated on. It answers three questions from a
//! pair of [`DrawOp`] logs, with no renderer, no pixels and no timing:
//!
//! 1. **What does tiling cost today?** `emitted` — the ops a real N-region
//!    replay actually issues, summed over regions.
//! 2. **What could it cost?** `required` — for each op the full frame drew, the
//!    number of regions its [`DrawOp::bounds`] intersects. This is the floor a
//!    perfect geometric cull (WS6.4b) would reach, computed from real layout
//!    geometry rather than estimated, and it is the number that replaces WS6.4's
//!    estimated "×1.6–2.0 per-object multiplier".
//! 3. **Is the replay sound?** [`tile_invariance`] — every op a region is
//!    obliged to draw must appear in that region's log, and no region may draw a
//!    geometry the full frame never produced.
//!
//! # Why op counts, and where they mislead
//!
//! The WS6.4 cost model says per-pixel work is *invariant* under tiling (each
//! output pixel is written once across the whole schedule) and only per-object
//! work repeats. An op log does not respect that split: text is drawn through
//! `DrawTargetProxy`, which issues **one `Pixel` op per glyph pixel**, so a
//! text-heavy page's log is dominated by what the cost model calls per-pixel
//! work. That is exactly why [`ScheduleReport`] reports `Pixel` separately and
//! carries a **structural** subtotal (every kind except `Pixel`): the structural
//! multiplier is the per-object number the cost model predicts, and the all-ops
//! multiplier is what the CPU actually pays. Where they diverge is the finding,
//! not a defect in the instrument — a clip that is a write-*filter* rather than a
//! loop bound shows up precisely as per-pixel work that fails to shrink.
//!
//! # What "bookkeeping" means here
//!
//! Ops with no [`DrawOp::bounds`] (only [`DrawOp::Clip`]) paint nothing, so they
//! carry no obligation and are excluded from every count and comparison. A region
//! replay legitimately pushes its own region clip that the full frame never had;
//! excluding clips is what keeps that from reading as a violation.

use crate::{
    geometry::{Point, Rect, Size},
    record::DrawOp,
};
use alloc::{string::String, vec::Vec};
use core::fmt;

/// How a frame is cut into regions.
///
/// Region *shape* is the economic lever (WS6.4c(1): a tile has no history, so
/// everything intersecting a region repaints), which is why this is a first-class
/// value with several constructors rather than a tile height parameter: the whole
/// point of 6.4a is comparing shapes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileSchedule {
    viewport: Rect,
    tiles: Vec<Rect>,
}

impl TileSchedule {
    /// One region covering everything — the degenerate schedule, and the control
    /// group: every multiplier must come out exactly 1.0 against it.
    pub fn whole(viewport: Rect) -> Self {
        Self { viewport, tiles: vec![viewport] }
    }

    /// Full-width horizontal bands, top to bottom. The last band is clipped to
    /// the viewport, so a height that does not divide evenly is still an exact
    /// partition (this is the "classic strips" degenerate case of WS6.4d(1)).
    pub fn rows(viewport: Rect, tile_height: u32) -> Self {
        Self::grid(viewport, Size::new(viewport.size.width, tile_height))
    }

    /// A row-major grid of `tile`-sized regions, each clipped to the viewport.
    pub fn grid(viewport: Rect, tile: Size) -> Self {
        let mut tiles = Vec::new();
        if !tile.is_zero_area() && !viewport.is_zero_sized() {
            let mut y = viewport.top_left.y;
            let bottom = viewport.top_left.y + viewport.size.height as i32;
            let right = viewport.top_left.x + viewport.size.width as i32;
            while y < bottom {
                let mut x = viewport.top_left.x;
                while x < right {
                    tiles.push(
                        Rect::new(Point::new(x, y), tile)
                            .intersection(&viewport),
                    );
                    x += tile.width as i32;
                }
                y += tile.height as i32;
            }
        }
        Self { viewport, tiles }
    }

    /// An explicit region list — the shape WS6.4d actually produces (tight damage
    /// rects after an area-test merge), which is neither a partition nor
    /// necessarily disjoint.
    pub fn from_regions(
        viewport: Rect,
        regions: impl IntoIterator<Item = Rect>,
    ) -> Self {
        Self { viewport, tiles: regions.into_iter().collect() }
    }

    pub fn viewport(&self) -> Rect {
        self.viewport
    }

    pub fn tiles(&self) -> &[Rect] {
        &self.tiles
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Σ region areas ÷ viewport area: `1.0` for an exact partition, `< 1.0` for
    /// a damage-driven schedule that covers only part of the screen, `> 1.0` when
    /// regions overlap (which double-paints, and is what WS6.4d's merge test
    /// exists to avoid).
    pub fn coverage(&self) -> f32 {
        let viewport = self.viewport.size.area() as f32;
        if viewport == 0.0 {
            return 0.0;
        }
        self.tiles.iter().map(|t| t.size.area() as f32).sum::<f32>() / viewport
    }
}

/// One region's replay: the region, and the ops that pass issued.
#[derive(Clone, Debug)]
pub struct TilePass {
    pub tile: Rect,
    pub ops: Vec<DrawOp>,
}

/// A frame captured twice: once whole, once region by region.
#[derive(Clone, Debug)]
pub struct ScheduleLog {
    /// The same frame drawn in one pass over the whole viewport — the reference.
    pub full: Vec<DrawOp>,
    /// One entry per region of the schedule, in schedule order.
    pub passes: Vec<TilePass>,
}

impl ScheduleLog {
    /// The schedule these passes were captured for.
    pub fn schedule(&self, viewport: Rect) -> TileSchedule {
        TileSchedule::from_regions(
            viewport,
            self.passes.iter().map(|pass| pass.tile),
        )
    }
}

/// A way a region replay failed to reproduce the full frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Violation {
    /// A region drew an op fewer times than its [`DrawOp::bounds`] obliges it to
    /// — pixels the full frame produced are simply absent from the tiled result.
    /// This is the crack-on-screen failure, and the reason the check exists.
    Missing { tile: Rect, op: DrawOp, wanted: usize, got: usize },
    /// A region drew a geometry that appears **nowhere** in the full frame.
    ///
    /// The failure this catches is position-dependent work computed in
    /// *region-relative* coordinates (WS6.4's absolute-coordinate invariant): the
    /// op count would be right and every rect subtly displaced, which no count
    /// comparison notices. Note what it does *not* mean: an op drawn in a region
    /// its bounds misses is legitimate today (there is no culling yet) and is
    /// reported as *waste* by [`ScheduleReport`], not as a violation.
    Foreign { tile: Rect, op: DrawOp, got: usize },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Violation::Missing { tile, op, wanted, got } => write!(
                f,
                "tile {tile}: `{op}` drawn {got}x, obliged {wanted}x (its bounds intersect this tile)"
            ),
            Violation::Foreign { tile, op, got } => write!(
                f,
                "tile {tile}: `{op}` drawn {got}x but the full frame never drew it \
                 (region-relative coordinates?)"
            ),
        }
    }
}

/// Check that a region-by-region replay reproduces the full frame: nothing
/// obliged is missing, and nothing foreign appears. Returns every violation, so a
/// failing test can print the whole picture instead of the first symptom.
///
/// The predicate is [`DrawOp::bounds`], deliberately — see its docs: this
/// function is that method's one-directional obligation turned into an
/// assertion, so a future cull must use the same bound or this check becomes
/// either vacuous or wrong.
pub fn tile_invariance(log: &ScheduleLog) -> Vec<Violation> {
    let mut violations = Vec::new();
    // Counted once, reused per region: the full frame's ops, deduplicated with
    // multiplicities.
    let full = counted(&log.full);

    for pass in &log.passes {
        let got = counted(&pass.ops);

        // Obliged: full-frame ops whose bounds intersect this region.
        for &(op, full_count) in &full {
            let Some(bounds) = op.bounds() else { continue };
            if !bounds.intersects(&pass.tile) {
                continue;
            }
            let drawn = lookup(&got, op);
            if drawn < full_count {
                violations.push(Violation::Missing {
                    tile: pass.tile,
                    op,
                    wanted: full_count,
                    got: drawn,
                });
            }
        }

        // Foreign: drawn here, absent from the full frame entirely.
        for &(op, pass_count) in &got {
            if op.bounds().is_none() {
                continue;
            }
            if lookup(&full, op) == 0 {
                violations.push(Violation::Foreign {
                    tile: pass.tile,
                    op,
                    got: pass_count,
                });
            }
        }
    }

    violations
}

/// Per-primitive-kind counts, one row of the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindCount {
    pub kind: &'static str,
    /// Occurrences in the full frame.
    pub full: usize,
    /// Occurrences summed over every region pass.
    pub emitted: usize,
    /// Occurrences a perfect geometric cull would still have to issue.
    pub required: usize,
}

impl KindCount {
    /// `emitted / full` — what a region replay costs today.
    pub fn emitted_multiplier(&self) -> f32 {
        ratio(self.emitted, self.full)
    }

    /// `required / full` — the floor WS6.4b's culling could reach. This is the
    /// per-object multiplier the WS6.4 cost model estimates at ×1.6–2.0.
    pub fn required_multiplier(&self) -> f32 {
        ratio(self.required, self.full)
    }
}

/// What one schedule costs, measured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduleReport {
    pub tiles: usize,
    /// All drawing ops (bookkeeping excluded).
    pub total: KindCount,
    /// Every kind except `Pixel` — the per-object term of the cost model. See
    /// the module docs for why the split matters.
    pub structural: KindCount,
    /// Per kind, in [`DrawOp`] declaration order, kinds absent from the full
    /// frame omitted.
    pub per_kind: Vec<KindCount>,
    /// Ops a region issued whose bounds do not touch that region: pure waste,
    /// and the budget WS6.4b is spending against.
    pub wasted: usize,
    /// Bookkeeping ops seen and excluded (clips), reported so the exclusion is
    /// visible rather than silent.
    pub bookkeeping: usize,
}

impl ScheduleReport {
    pub fn of(log: &ScheduleLog) -> Self {
        let mut per_kind = Vec::new();
        for &kind in KINDS {
            let full = log.full.iter().filter(|op| kind_of(op) == kind).count();
            if full == 0 {
                continue;
            }
            let emitted = log
                .passes
                .iter()
                .flat_map(|pass| pass.ops.iter())
                .filter(|op| kind_of(op) == kind)
                .count();
            let required = log
                .full
                .iter()
                .filter(|op| kind_of(op) == kind)
                .map(|op| tiles_touched(op, log))
                .sum();
            per_kind.push(KindCount { kind, full, emitted, required });
        }

        let sum = |rows: &[KindCount]| KindCount {
            kind: "",
            full: rows.iter().map(|r| r.full).sum(),
            emitted: rows.iter().map(|r| r.emitted).sum(),
            required: rows.iter().map(|r| r.required).sum(),
        };

        let drawing: Vec<KindCount> = per_kind
            .iter()
            .copied()
            .filter(|r| r.kind != CLIP)
            .collect();
        let structural: Vec<KindCount> = drawing
            .iter()
            .copied()
            .filter(|r| r.kind != PIXEL)
            .collect();

        let wasted = log
            .passes
            .iter()
            .flat_map(|pass| pass.ops.iter().map(move |op| (pass.tile, op)))
            .filter(|(tile, op)| {
                op.bounds().map(|b| !b.intersects(tile)).unwrap_or(false)
            })
            .count();

        Self {
            tiles: log.passes.len(),
            total: KindCount { kind: "all", ..sum(&drawing) },
            structural: KindCount { kind: "structural", ..sum(&structural) },
            per_kind: drawing,
            wasted,
            bookkeeping: log
                .passes
                .iter()
                .flat_map(|pass| pass.ops.iter())
                .chain(log.full.iter())
                .filter(|op| op.bounds().is_none())
                .count(),
        }
    }
}

impl fmt::Display for ScheduleReport {
    /// A fixed-width table, stable enough to keep as a blessed golden so any
    /// change in op emission shows up as a reviewable diff (and so WS6.4b's win
    /// is visible as one).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} tiles", self.tiles)?;
        writeln!(
            f,
            "{:<14}{:>9}{:>9}{:>9}{:>9}{:>9}",
            "kind", "full", "emitted", "required", "xemit", "xreq"
        )?;
        let row = |f: &mut fmt::Formatter<'_>, r: &KindCount| {
            writeln!(
                f,
                "{:<14}{:>9}{:>9}{:>9}{:>9.2}{:>9.2}",
                r.kind,
                r.full,
                r.emitted,
                r.required,
                r.emitted_multiplier(),
                r.required_multiplier()
            )
        };
        row(f, &self.total)?;
        row(f, &self.structural)?;
        for kind in &self.per_kind {
            row(f, kind)?;
        }
        writeln!(f, "wasted {} bookkeeping {}", self.wasted, self.bookkeeping)
    }
}

/// Whether merging two damage rects into their union is cheaper than painting
/// them separately — WS6.4d's area-test threshold, measured instead of guessed.
///
/// Both costs are real: `separate` pays the per-region overhead twice
/// (`CASET`/`RASET`/`RAMWR`, plus one background fill per region) and paints the
/// overlap twice; `merged` pays it once but repaints everything in the union's
/// dead space. The op counts here are the *paint* term and the areas are the
/// *fill/transfer* term; the threshold is where their sum crosses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MergeVerdict {
    pub separate_ops: usize,
    pub merged_ops: usize,
    pub separate_area: u32,
    pub merged_area: u32,
}

impl MergeVerdict {
    /// `merged_ops / separate_ops` — below 1.0 merging strictly wins on paint
    /// (the overlap was being drawn twice).
    pub fn op_ratio(&self) -> f32 {
        ratio(self.merged_ops, self.separate_ops)
    }

    /// `merged_area / separate_area` — the classic LVGL area test. Above 1.0 the
    /// union has dead space the separate rects did not.
    pub fn area_ratio(&self) -> f32 {
        ratio(self.merged_area as usize, self.separate_area as usize)
    }
}

pub fn merge_verdict(full: &[DrawOp], a: Rect, b: Rect) -> MergeVerdict {
    let touching = |region: &Rect| {
        full.iter()
            .filter(|op| {
                op.bounds().map(|s| s.intersects(region)).unwrap_or(false)
            })
            .count()
    };
    let union = a.union(&b);
    MergeVerdict {
        separate_ops: touching(&a) + touching(&b),
        merged_ops: touching(&union),
        separate_area: a.size.area() + b.size.area(),
        merged_area: union.size.area(),
    }
}

// -- internals --------------------------------------------------------------

const CLIP: &str = "Clip";
const PIXEL: &str = "Pixel";

/// [`DrawOp`] declaration order, so a report's rows are stable across runs
/// (discovery order would depend on what the page happened to draw first).
const KINDS: &[&str] = &[
    CLIP,
    "FillSolid",
    PIXEL,
    "Line",
    "Rect",
    "RoundedRect",
    "Circle",
    "Arc",
    "Ellipse",
    "Sector",
    "Polygon",
    "Path",
    "Image",
];

fn kind_of(op: &DrawOp) -> &'static str {
    match op {
        DrawOp::Clip(_) => CLIP,
        DrawOp::FillSolid(_) => "FillSolid",
        DrawOp::Pixel(_) => PIXEL,
        DrawOp::Line { .. } => "Line",
        DrawOp::Rect(_) => "Rect",
        DrawOp::RoundedRect(_) => "RoundedRect",
        DrawOp::Circle { .. } => "Circle",
        DrawOp::Arc { .. } => "Arc",
        DrawOp::Ellipse(_) => "Ellipse",
        DrawOp::Sector { .. } => "Sector",
        DrawOp::Polygon { .. } => "Polygon",
        DrawOp::Path { .. } => "Path",
        DrawOp::Image { .. } => "Image",
    }
}

/// How many of the schedule's regions this op is obliged to appear in.
fn tiles_touched(op: &DrawOp, log: &ScheduleLog) -> usize {
    let Some(bounds) = op.bounds() else { return 0 };
    log.passes
        .iter()
        .filter(|p| bounds.intersects(&p.tile))
        .count()
}

/// A total order on ops, used only to collapse a log into counted runs.
///
/// It is injective over the fields a [`DrawOp`] records — two ops with equal keys
/// are equal ops — which is what makes the sort-then-merge multiset comparison
/// exact. Kept private and key-based rather than deriving `Ord` on `DrawOp`: a
/// lexicographic order on rectangles is meaningless as public API, and this
/// avoids putting `Ord` on the geometry types where it would invite misuse.
fn sort_key(op: &DrawOp) -> (u8, i32, i32, i32, i32, usize) {
    let rect = |kind: u8, r: Rect| {
        (
            kind,
            r.top_left.x,
            r.top_left.y,
            r.size.width as i32,
            r.size.height as i32,
            0,
        )
    };
    match *op {
        DrawOp::Clip(r) => rect(0, r),
        DrawOp::FillSolid(r) => rect(1, r),
        DrawOp::Pixel(p) => (2, p.x, p.y, 0, 0, 0),
        DrawOp::Line { from, to } => (3, from.x, from.y, to.x, to.y, 0),
        DrawOp::Rect(r) => rect(4, r),
        DrawOp::RoundedRect(r) => rect(5, r),
        DrawOp::Circle { top_left, diameter } => {
            (6, top_left.x, top_left.y, diameter as i32, 0, 0)
        },
        DrawOp::Arc { top_left, diameter } => {
            (7, top_left.x, top_left.y, diameter as i32, 0, 0)
        },
        DrawOp::Ellipse(r) => rect(8, r),
        DrawOp::Sector { top_left, diameter } => {
            (9, top_left.x, top_left.y, diameter as i32, 0, 0)
        },
        DrawOp::Polygon { points, bounds } => {
            let (k, x, y, w, h, _) = rect(10, bounds);
            (k, x, y, w, h, points)
        },
        DrawOp::Path { bounds } => rect(11, bounds),
        DrawOp::Image { bounds } => rect(12, bounds),
    }
}

/// Collapse a log into `(op, count)` runs sorted by [`sort_key`]. `O(n log n)`,
/// which matters: a text-heavy page logs one `Pixel` op per glyph pixel, so the
/// obvious `Vec` linear scan would be quadratic in tens of thousands of ops.
fn counted(ops: &[DrawOp]) -> Vec<(DrawOp, usize)> {
    let mut sorted: Vec<DrawOp> = ops.to_vec();
    sorted.sort_unstable_by_key(sort_key);
    let mut out: Vec<(DrawOp, usize)> = Vec::new();
    for op in sorted {
        match out.last_mut() {
            Some((last, count)) if *last == op => *count += 1,
            _ => out.push((op, 1)),
        }
    }
    out
}

/// Multiplicity of `op` in a [`counted`] table, `0` if absent.
fn lookup(table: &[(DrawOp, usize)], op: DrawOp) -> usize {
    table
        .binary_search_by_key(&sort_key(&op), |(candidate, _)| {
            sort_key(candidate)
        })
        .map(|at| table[at].1)
        .unwrap_or(0)
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 { 0.0 } else { numerator as f32 / denominator as f32 }
}

/// Render a report to the stable text used as a blessed golden.
pub fn format_report(report: &ScheduleReport) -> String {
    use core::fmt::Write as _;
    let mut out = String::new();
    let _ = write!(out, "{report}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect::new(Point::new(x, y), Size::new(w, h))
    }

    /// The 64x64 viewport used below, cut into four 64x16 bands.
    fn bands() -> TileSchedule {
        TileSchedule::rows(r(0, 0, 64, 64), 16)
    }

    /// Three ops, one per band, none straddling: a perfect replay draws each op
    /// in exactly one band.
    fn one_per_band() -> Vec<DrawOp> {
        vec![
            DrawOp::Rect(r(0, 0, 8, 8)),  // band 0
            DrawOp::Rect(r(0, 20, 8, 8)), // band 1
            DrawOp::Rect(r(0, 40, 8, 8)), // band 2
        ]
    }

    /// A replay that culls perfectly: each band gets exactly the ops it owes.
    fn ideal_log(full: &[DrawOp], schedule: &TileSchedule) -> ScheduleLog {
        ScheduleLog {
            full: full.to_vec(),
            passes: schedule
                .tiles()
                .iter()
                .map(|&tile| TilePass {
                    tile,
                    ops: full
                        .iter()
                        .copied()
                        .filter(|op| {
                            op.bounds()
                                .map(|b| b.intersects(&tile))
                                .unwrap_or(false)
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// A replay that culls nothing: every band redraws the whole frame. This is
    /// what rsact does TODAY (there is no geometric cull yet), so it must be
    /// sound — merely wasteful.
    fn unculled_log(full: &[DrawOp], schedule: &TileSchedule) -> ScheduleLog {
        ScheduleLog {
            full: full.to_vec(),
            passes: schedule
                .tiles()
                .iter()
                .map(|&tile| TilePass { tile, ops: full.to_vec() })
                .collect(),
        }
    }

    #[test]
    fn rows_partition_the_viewport_exactly() {
        let schedule = bands();
        assert_eq!(schedule.len(), 4);
        assert_eq!(schedule.tiles()[0], r(0, 0, 64, 16));
        assert_eq!(schedule.tiles()[3], r(0, 48, 64, 16));
        assert_eq!(schedule.coverage(), 1.0);
    }

    /// A height that does not divide the viewport must still partition it — the
    /// last band is short, not overhanging, or every area figure is inflated.
    #[test]
    fn a_ragged_last_row_is_clipped_to_the_viewport() {
        let schedule = TileSchedule::rows(r(0, 0, 64, 50), 16);
        assert_eq!(schedule.len(), 4);
        assert_eq!(schedule.tiles()[3], r(0, 48, 64, 2));
        assert_eq!(schedule.coverage(), 1.0);
    }

    #[test]
    fn the_whole_schedule_is_the_control_group() {
        let full = one_per_band();
        let schedule = TileSchedule::whole(r(0, 0, 64, 64));
        let report = ScheduleReport::of(&ideal_log(&full, &schedule));
        // One region: nothing repeats, nothing is wasted, both multipliers 1.0.
        assert_eq!(report.total.emitted_multiplier(), 1.0);
        assert_eq!(report.total.required_multiplier(), 1.0);
        assert_eq!(report.wasted, 0);
    }

    #[test]
    fn a_perfect_cull_costs_exactly_one_pass_worth() {
        let full = one_per_band();
        let report = ScheduleReport::of(&ideal_log(&full, &bands()));
        assert_eq!(report.total.full, 3);
        assert_eq!(report.total.emitted, 3);
        assert_eq!(report.total.required, 3);
        assert_eq!(report.wasted, 0);
    }

    /// The measurement that matters: with no culling, an N-region replay pays N×
    /// for *everything*, while `required` stays at the geometric floor. The gap
    /// is what WS6.4b is worth.
    #[test]
    fn no_culling_costs_the_full_frame_per_region() {
        let full = one_per_band();
        let schedule = bands();
        let report = ScheduleReport::of(&unculled_log(&full, &schedule));
        assert_eq!(report.total.emitted, 3 * 4, "every band redrew everything");
        assert_eq!(report.total.emitted_multiplier(), 4.0);
        assert_eq!(report.total.required, 3, "each op owes exactly one band");
        assert_eq!(report.total.required_multiplier(), 1.0);
        assert_eq!(report.wasted, 3 * 4 - 3);
    }

    /// An op straddling a boundary owes *both* regions — this is the entire
    /// per-object multiplier the cost model is about, in its smallest form.
    #[test]
    fn a_straddling_op_owes_every_region_it_touches() {
        // y 12..=20 crosses the 16-px band boundary.
        let full = vec![DrawOp::Rect(r(0, 12, 8, 9))];
        let report = ScheduleReport::of(&ideal_log(&full, &bands()));
        assert_eq!(report.total.required, 2);
        assert_eq!(report.total.required_multiplier(), 2.0);
    }

    #[test]
    fn text_is_reported_apart_from_structure() {
        // One rect plus four glyph pixels, all inside band 0.
        let mut full = vec![DrawOp::Rect(r(0, 0, 8, 8))];
        for x in 0..4 {
            full.push(DrawOp::Pixel(Point::new(x, 2)));
        }
        let report = ScheduleReport::of(&unculled_log(&full, &bands()));
        // All-ops is dominated by per-pixel work; the structural subtotal is the
        // per-object number the WS6.4 cost model predicts.
        assert_eq!(report.total.full, 5);
        assert_eq!(report.structural.full, 1);
        assert_eq!(report.structural.emitted, 4);
        assert_eq!(report.structural.required, 1);
    }

    #[test]
    fn clips_are_bookkeeping_not_drawing() {
        let full = vec![DrawOp::Rect(r(0, 0, 8, 8))];
        let schedule = bands();
        // Each band pushes its own region clip, which the full frame never had.
        let log = ScheduleLog {
            full: full.clone(),
            passes: schedule
                .tiles()
                .iter()
                .map(|&tile| TilePass {
                    tile,
                    ops: vec![DrawOp::Clip(tile), full[0]],
                })
                .collect(),
        };
        // The region clips must not read as foreign ops...
        assert_eq!(tile_invariance(&log), &[]);
        // ...nor inflate any count.
        let report = ScheduleReport::of(&log);
        assert_eq!(report.total.emitted, 4);
        assert_eq!(report.bookkeeping, 4);
    }

    // -- the checker's own teeth ---------------------------------------------
    //
    // Every assertion above passes for rsact TODAY, so on its own it proves
    // nothing about what the check would catch. These feed it deliberately broken
    // replays.

    #[test]
    fn a_dropped_op_is_caught() {
        let full = one_per_band();
        let mut log = ideal_log(&full, &bands());
        // Band 1 forgets the op it owes: a hole in the tiled frame.
        let dropped = log.passes[1].ops.remove(0);
        let violations = tile_invariance(&log);
        assert_eq!(
            violations,
            &[Violation::Missing {
                tile: r(0, 16, 64, 16),
                op: dropped,
                wanted: 1,
                got: 0,
            }]
        );
    }

    /// An op straddling two regions that is drawn in only one of them: the count
    /// is right in aggregate, which is exactly why the check is per region.
    #[test]
    fn a_half_drawn_straddler_is_caught() {
        let full = vec![DrawOp::Rect(r(0, 12, 8, 9))];
        let mut log = ideal_log(&full, &bands());
        log.passes[1].ops.clear();
        assert!(matches!(
            tile_invariance(&log).as_slice(),
            [Violation::Missing { wanted: 1, got: 0, .. }]
        ));
    }

    /// Region-relative coordinates: the same number of ops, each displaced to the
    /// region's own origin. No count comparison sees this; `Foreign` does.
    #[test]
    fn region_relative_coordinates_are_caught() {
        let full = vec![DrawOp::Rect(r(0, 20, 8, 8))];
        let schedule = bands();
        let log = ScheduleLog {
            full: full.clone(),
            passes: vec![TilePass {
                tile: schedule.tiles()[1],
                // Drawn at y=4 *within* band 1 instead of absolute y=20.
                ops: vec![DrawOp::Rect(r(0, 4, 8, 8))],
            }],
        };
        let violations = tile_invariance(&log);
        assert_eq!(
            violations.len(),
            2,
            "the absolute op is missing AND a foreign one appeared"
        );
        assert!(
            violations
                .iter()
                .any(|v| matches!(v, Violation::Missing { .. }))
        );
        assert!(
            violations
                .iter()
                .any(|v| matches!(v, Violation::Foreign { .. }))
        );
    }

    /// Drawing an op in a region its bounds miss is *waste*, not a violation —
    /// it is what rsact does today, and mislabelling it would make the check
    /// permanently red instead of useful.
    #[test]
    fn over_drawing_is_waste_and_not_a_violation() {
        let full = one_per_band();
        let log = unculled_log(&full, &bands());
        assert_eq!(tile_invariance(&log), &[]);
        assert!(ScheduleReport::of(&log).wasted > 0);
    }

    #[test]
    fn merging_two_close_rects_saves_a_region_but_not_paint() {
        // Two 8x8 rects, 4 px apart: the union is barely bigger than the parts.
        let full =
            vec![DrawOp::Rect(r(0, 0, 8, 8)), DrawOp::Rect(r(12, 0, 8, 8))];
        let verdict = merge_verdict(&full, r(0, 0, 8, 8), r(12, 0, 8, 8));
        assert_eq!(verdict.separate_ops, 2);
        assert_eq!(verdict.merged_ops, 2, "the union covers both");
        assert_eq!(verdict.separate_area, 128);
        assert_eq!(verdict.merged_area, 160);
        assert_eq!(verdict.area_ratio(), 1.25);
    }

    /// Far-apart rects: the union's dead space is most of it, so the area test
    /// must reject the merge. This is the case WS6.4d's threshold exists for.
    #[test]
    fn merging_far_apart_rects_is_rejected_by_area() {
        let full =
            vec![DrawOp::Rect(r(0, 0, 8, 8)), DrawOp::Rect(r(200, 200, 8, 8))];
        let verdict = merge_verdict(&full, r(0, 0, 8, 8), r(200, 200, 8, 8));
        assert_eq!(verdict.separate_area, 128);
        assert_eq!(verdict.merged_area, 208 * 208);
        assert!(verdict.area_ratio() > 300.0);
    }
}
