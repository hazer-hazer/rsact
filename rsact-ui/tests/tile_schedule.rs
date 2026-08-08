//! WS6.4a: the tile-schedule measurement harness, run against real pages.
//!
//! Two jobs, deliberately in one file because they share the capture:
//!
//! - **The regression.** [`tile_invariance`] on every page × schedule: a
//!   region-by-region replay must not lose an op it owes, and must not draw a
//!   geometry the full frame never produced (WS6.4's absolute-coordinate
//!   invariant). This is vacuous *today* — nothing culls yet, so every region
//!   redraws everything — and becomes load-bearing the moment WS6.4b starts
//!   pruning. `rsact_render::schedule`'s own tests are what prove the check has
//!   teeth, by feeding it deliberately broken replays.
//! - **The measurement.** A blessed report of what tiling costs now (`emitted`)
//!   against the floor a perfect geometric cull would reach (`required`), plus the
//!   traversal term the op log cannot see. WS6.4d is gated on these numbers, and
//!   WS6.4b's win will show up here as a golden diff.
//!
//! Run as an integration test on purpose: it proves the harness is usable from
//! *outside* the crate, which is what `metrics-probe` will need.

use core::fmt::Write as _;
use rsact_reactive::runtime::with_new_runtime;
use rsact_render::{
    golden::assert_text_golden,
    record::DrawOp,
    region::{RegionLimits, plan_regions},
    renderer::region_units,
    schedule::{
        ScheduleReport, TileSchedule, format_report, merge_verdict,
        tile_invariance,
    },
};
use rsact_ui::{
    prelude::*,
    test_support::tile_probe::{RecWtf, TileProbe},
    value::RangeU8,
    widget::knob::Knob,
};

/// G3's colour reference target: 240x240 RGB565 ST7789, the display whose 112.5
/// KiB framebuffer does not fit the Black Pill's 96 K RAM — i.e. the exact case
/// WS6.4 exists for.
fn viewport() -> Size {
    Size::new_equal(240)
}

/// The schedules compared. `whole` is the control group (every multiplier must
/// come out 1.00); the bands are WS6.4d's degenerate strip case at two budgets;
/// the grid is closer to what tight damage rects look like.
fn schedules(viewport: Size) -> Vec<(&'static str, TileSchedule)> {
    let frame = Rect::new(Point::zero(), viewport);
    vec![
        ("whole", TileSchedule::whole(frame)),
        ("rows-48", TileSchedule::rows(frame, 48)),
        ("rows-24", TileSchedule::rows(frame, 24)),
        ("grid-80", TileSchedule::grid(frame, Size::new_equal(80))),
    ]
}

/// Capture every schedule for one page: assert the replay is sound, and append
/// its measurements to `out`.
fn measure<V: View<RecWtf>>(name: &str, root: V, out: &mut String) {
    let viewport = viewport();
    let mut probe = TileProbe::new(viewport, root);

    for (label, schedule) in schedules(viewport) {
        let log = probe.capture(&schedule);

        let violations = tile_invariance(&log);
        assert!(
            violations.is_empty(),
            "{name} / {label}: {} violation(s), first: {}",
            violations.len(),
            violations[0]
        );

        let _ = writeln!(
            out,
            "== {name} / {label} ({} regions, coverage {:.2}) ==",
            schedule.len(),
            schedule.coverage()
        );
        out.push_str(&format_report(&ScheduleReport::of(&log)));
        let _ = write!(out, "{}", probe.visits(&schedule));
        out.push('\n');
    }
}

// -- the pages ---------------------------------------------------------------
//
// Chosen for distinct op profiles rather than realism: text-heavy (the `Pixel`
// flood through `DrawTargetProxy`), block-heavy, a page that overflows its
// viewport, and a mixed one.

fn labels_page(n: usize) -> impl View<RecWtf> {
    Flex::col(
        (0..n)
            .map(|i| Label::new(format!("label number {i}").inert()).into_el())
            .collect::<Vec<_>>(),
    )
    .fill()
    .gap(4u32)
}

fn buttons_page(n: usize) -> impl View<RecWtf> {
    Flex::col(
        (0..n)
            .map(|i| Button::new(format!("button {i}")).into_el())
            .collect::<Vec<_>>(),
    )
    .fill()
    .gap(4u32)
}

fn checkboxes_page(n: usize) -> impl View<RecWtf> {
    Flex::col(
        (0..n)
            .map(|_| Checkbox::new(true).into_el())
            .collect::<Vec<_>>(),
    )
    .fill()
    .gap(4u32)
}

/// Content taller than the viewport, inside a `Scrollable` — the page whose
/// layout nodes reach outside their parent, which is the soundness question for
/// WS6.4b's subtree culling (see `VisitReport::escaping`). Note the overflow is
/// genuinely unclipped, not merely un-pruned: nothing in the crate sets
/// `ElState::clip_path`, so only the framebuffer viewport bounds it.
fn scrollable_page(n: usize) -> impl View<RecWtf> {
    Scrollable::vertical(
        Flex::col(
            (0..n)
                .map(|i| Button::new(format!("row {i}")).into_el())
                .collect::<Vec<_>>(),
        )
        .width_fill()
        .gap(2u32),
    )
    .fill()
}

/// Rows of `checkbox + label`, the shape WS6.4d(1)'s tight-rect argument is
/// about: a full-width band around one checkbox also catches its neighbour's
/// text, a tight rect around the checkbox does not.
fn option_rows_page(n: usize) -> impl View<RecWtf> {
    Flex::col(
        (0..n)
            .map(|i| {
                Flex::row(vec![
                    Checkbox::new(true).into_el(),
                    Label::new(format!("option {i}").inert()).into_el(),
                ])
                .width_fill()
                .gap(6u32)
                .into_el()
            })
            .collect::<Vec<_>>(),
    )
    .fill()
    .gap(6u32)
}

/// [`option_rows_page`] with the checkboxes wired to caller-held signals, so a
/// test can damage a chosen *set* of rows in one frame — which is what makes the
/// WS6.4d(1) planner's merge decisions observable on real geometry.
fn toggle_rows_page(checks: Vec<Signal<bool>>) -> impl View<RecWtf> {
    Flex::col(
        checks
            .into_iter()
            .enumerate()
            .map(|(i, check)| {
                Flex::row(vec![
                    Checkbox::new(check).into_el(),
                    Label::new(format!("option {i}").inert()).into_el(),
                ])
                .width_fill()
                .gap(6u32)
                .into_el()
            })
            .collect::<Vec<_>>(),
    )
    .fill()
    .gap(6u32)
}

fn mixed_page() -> impl View<RecWtf> {
    Flex::col(vec![
        Label::new("Mixed page".inert()).into_el(),
        Checkbox::new(true).into_el(),
        Button::new("Act").into_el(),
        Flex::row(vec![
            Label::new("left".inert()).into_el(),
            Label::new("right".inert()).into_el(),
        ])
        .width_fill()
        .into_el(),
        Label::new("footer".inert()).into_el(),
    ])
    .fill()
    .gap(6u32)
}

// -- the tests ---------------------------------------------------------------

/// The blessed measurement. Bless with `UPDATE_GOLDENS=1`; a diff here is either
/// a change in what widgets draw or (the point) WS6.4b's culling landing.
#[test]
fn tile_schedule_measurements() {
    with_new_runtime(|_| {
        let mut out = String::new();
        measure("labels-12", labels_page(12), &mut out);
        measure("buttons-8", buttons_page(8), &mut out);
        measure("checkboxes-8", checkboxes_page(8), &mut out);
        measure("scrollable-20", scrollable_page(20), &mut out);
        measure("option-rows-6", option_rows_page(6), &mut out);
        measure("mixed", mixed_page(), &mut out);

        assert_text_golden(
            env!("CARGO_MANIFEST_DIR"),
            "tile_schedule_240.txt",
            &out,
        );
    });
}

/// The invariance check on the AA-heavy widgets, which are deliberately kept out
/// of the golden: `Knob`/`Slider` reach the anti-aliased circle/arc rasterisers,
/// whose op *positions* come from `f32` trigonometry, and a blessed count could
/// drift between architectures (the WS6.4 cost model excludes them for the same
/// reason — they are acknowledged stubs). Invariance is self-consistency between
/// two captures on the same machine, so it holds regardless.
#[test]
fn anti_aliased_widgets_replay_soundly() {
    with_new_runtime(|_| {
        let viewport = viewport();
        let mut probe = TileProbe::new(
            viewport,
            Flex::col(vec![
                Slider::horizontal(0.5f32, (0.0f32..=1.0f32).inert()).into_el(),
                Knob::new(create_signal(RangeU8::new_full_range(64))).into_el(),
            ])
            .fill()
            .gap(8u32),
        );

        for (label, schedule) in schedules(viewport) {
            let log = probe.capture(&schedule);
            let violations = tile_invariance(&log);
            assert!(
                violations.is_empty(),
                "{label}: {} violation(s), first: {}",
                violations.len(),
                violations[0]
            );
        }
    });
}

/// The interactive-frame case, on rsact's OWN damage rects.
///
/// The WS6.4 cost model claims interactive frames cost **+0%** under tiling
/// ("damage fits one tile, no repetition at all"). Every other test here measures
/// a *cold* frame, which can only confirm the expensive half. This one takes the
/// damage the probe-gated render actually recorded for a change and measures that
/// schedule, so the claim is checked against the real thing.
///
/// It measures **two** interactive changes, because they behave nothing alike and
/// the difference is the finding:
///
/// - a **paint-only** change (a checkbox toggles — same size, no relayout) damages
///   exactly that widget's 16x16 rect, and is the case the cost model describes;
/// - a **text** change used to damage the **whole viewport**, with
///   `incremental-layout` making no difference whatsoever. That was ISSUE-2, and
///   it was a channel mismatch rather than a threshold: a `Label`'s text lived in
///   its layout (`ContentLayout::text`) as a reactive handle *read during
///   measurement*, so it reached the relayout through the tracked-read channel
///   and never marked `ElArena`'s dirty set. WS5.2's incremental path requires a
///   non-empty dirty set, so it fell through to a full recompute, and WS6.1's
///   targeted repaint roots were never computed ⇒ `blanket` ⇒ `full_flush`.
///
///   Fixed by routing text through `LayoutBuilder::setter` like every other
///   layout property. **Measured here: coverage 1.00 → 0.01, required 478 → 93**
///   — which also restores the WS6.4 cost model's "+0%" interactive claim for
///   text-driven UIs, previously a WS6.4d precondition.
///
/// The two feature configurations legitimately differ now, so they keep separate
/// goldens. A default build still blankets: it maintains the dirty set but never
/// consumes it, so `compute_layout` always full-recomputes and always reports
/// `blanket`. That is the designed default, not a leftover bug.
///
/// Both halves are asserted under `incremental-layout`. Recording alone is what
/// let ISSUE-2 sit in a golden for months as a documented number rather than a
/// failing test.
#[test]
fn an_interactive_frame_repaints_a_fraction_of_the_screen() {
    with_new_runtime(|_| {
        let viewport = viewport();
        let mut checked = create_signal(false);
        let mut caption = create_signal(String::from("value 0"));
        let mut probe = TileProbe::new(
            viewport,
            Flex::col(vec![
                Label::new("static header".inert()).into_el(),
                Checkbox::new(checked).into_el(),
                Label::new(caption).into_el(),
                Label::new("static footer".inert()).into_el(),
            ])
            .fill()
            .gap(6u32),
        );

        let cold = ScheduleReport::of(
            &probe.capture(&TileSchedule::whole(probe.viewport())),
        );

        // Paint-only: the checkbox's size does not depend on its value.
        let paint = probe.damage_after(|_| checked.set(true));
        assert!(
            !paint.is_empty(),
            "the write must have damaged something, or there is nothing to measure"
        );
        let paint_report = ScheduleReport::of(&probe.capture(&paint));

        assert!(
            paint.coverage() < 0.5,
            "a checkbox toggle damaged {:.0}% of the viewport",
            paint.coverage() * 100.0
        );
        assert!(
            paint_report.total.required * 2 < cold.total.full,
            "a paint-only frame must repaint far less than a cold one \
             ({} vs {})",
            paint_report.total.required,
            cold.total.full
        );

        // Text. Same width ("value 0" -> "value 1"), so close to the best case:
        // only the label's own box should move, if anything.
        let relayout =
            probe.damage_after(|_| caption.set(String::from("value 1")));
        let relayout_report = ScheduleReport::of(&probe.capture(&relayout));

        // ISSUE-2's regression guard. Without `incremental-layout` the dirty set
        // is maintained but never consumed, so a text change is still a blanket
        // frame by design and there is nothing here to assert.
        #[cfg(feature = "incremental-layout")]
        assert!(
            relayout.coverage() < 0.5,
            "a text change damaged {:.0}% of the viewport — ISSUE-2 has \
             regressed: the label's text is reaching relayout through a \
             tracked read again instead of marking the arena dirty",
            relayout.coverage() * 100.0
        );

        let mut out = String::new();
        let _ = writeln!(
            out,
            "{:<16}{:>8}{:>9}{:>10}",
            "frame", "regions", "coverage", "required"
        );
        let _ = writeln!(
            out,
            "{:<16}{:>8}{:>9.2}{:>10}",
            "cold", 1, 1.0, cold.total.full
        );
        for (label, schedule, report) in [
            ("paint-only", &paint, &paint_report),
            ("text-change", &relayout, &relayout_report),
        ] {
            let _ = writeln!(
                out,
                "{:<16}{:>8}{:>9.2}{:>10}",
                label,
                schedule.len(),
                schedule.coverage(),
                report.total.required
            );
        }
        // Separate goldens per feature config: the two now legitimately differ
        // (see the doc comment), and folding them into one file would mean
        // either blessing the worse number or leaving one config ungoldened.
        #[cfg(not(feature = "incremental-layout"))]
        let golden = "tile_damage_240.txt";
        #[cfg(feature = "incremental-layout")]
        let golden = "tile_damage_240_incremental.txt";

        assert_text_golden(env!("CARGO_MANIFEST_DIR"), golden, &out);
    });
}

/// WS6.4d(1)'s central claim, measured: *region shape sets the repaint set*.
///
/// "A full-width 240×24 band makes a one-checkbox change repaint every widget
/// crossing those 24 rows; a tight 16×16 rect repaints the checkbox and its
/// backdrop." Both regions are derived from the frame itself — the tight rect is
/// the checkbox's own `RoundedRect` bound, i.e. exactly what `render_part` pushes
/// as damage — so this is the real geometry, not a hand-picked rectangle.
///
/// The assertion is the design premise as an inequality; the golden records by
/// how much, which is what makes the shape decision reviewable.
#[test]
fn a_tight_rect_repaints_less_than_a_band() {
    with_new_runtime(|_| {
        let viewport = viewport();
        let frame = Rect::new(Point::zero(), viewport);
        let mut probe = TileProbe::new(viewport, option_rows_page(6));

        // The first checkbox's box: a checkbox draws `FillSolid` + `RoundedRect` +
        // `Path`, and the rounded rect is its outer edge.
        let full = probe.capture(&TileSchedule::whole(frame)).full;
        let tight = full
            .iter()
            .find(|op| matches!(op, DrawOp::RoundedRect(_)))
            .and_then(|op| op.bounds())
            .expect("the page draws at least one checkbox");
        // The band WS6.4d(1) rejects: full width, 24 rows, containing that rect.
        let band = Rect::new(
            Point::new(0, tight.top_left.y),
            Size::new(viewport.width, 24),
        );

        let mut of = |region: Rect| {
            ScheduleReport::of(
                &probe.capture(&TileSchedule::from_regions(frame, [region])),
            )
        };
        let (tight_report, band_report) = (of(tight), of(band));

        assert!(
            tight_report.total.required < band_report.total.required,
            "a tight rect must repaint strictly less than the band containing it"
        );

        let mut out = String::new();
        let _ = writeln!(
            out,
            "{:<8}{:>12}{:>10}{:>10}",
            "region", "rect", "area", "required"
        );
        for (label, region, report) in
            [("tight", tight, &tight_report), ("band", band, &band_report)]
        {
            let _ = writeln!(
                out,
                "{:<8}{:>12}{:>10}{:>10}",
                label,
                format!(
                    "{},{} {}x{}",
                    region.top_left.x,
                    region.top_left.y,
                    region.size.width,
                    region.size.height
                ),
                region.size.area(),
                report.total.required
            );
        }
        assert_text_golden(
            env!("CARGO_MANIFEST_DIR"),
            "tile_shape_240.txt",
            &out,
        );
    });
}

/// WS6.4d(1)'s area-test threshold, measured on a real page instead of guessed.
///
/// Two damage rects are merged only when the union's area is not much larger than
/// the sum of the parts. This reports both terms the threshold has to balance —
/// paint (ops that fall inside each region) and transfer (area) — for a
/// near-neighbour pair and a far-apart pair on the same frame.
#[test]
fn merge_threshold_numbers() {
    with_new_runtime(|_| {
        let viewport = viewport();
        let mut probe = TileProbe::new(viewport, mixed_page());
        let log = probe
            .capture(&TileSchedule::whole(Rect::new(Point::zero(), viewport)));

        let mut out = String::new();
        let pairs = [
            (
                "adjacent",
                Rect::new(Point::new(0, 0), Size::new(64, 16)),
                Rect::new(Point::new(0, 20), Size::new(64, 16)),
            ),
            (
                "far",
                Rect::new(Point::new(0, 0), Size::new(64, 16)),
                Rect::new(Point::new(160, 200), Size::new(64, 16)),
            ),
            (
                "overlapping",
                Rect::new(Point::new(0, 0), Size::new(64, 32)),
                Rect::new(Point::new(0, 16), Size::new(64, 32)),
            ),
        ];
        for (label, a, b) in pairs {
            let verdict = merge_verdict(&log.full, a, b);
            let _ = writeln!(
                out,
                "{label:<12} ops {:>6} -> {:>6} ({:.2}x)  area {:>7} -> {:>7} ({:.2}x)",
                verdict.separate_ops,
                verdict.merged_ops,
                verdict.op_ratio(),
                verdict.separate_area,
                verdict.merged_area,
                verdict.area_ratio(),
            );
        }

        assert_text_golden(
            env!("CARGO_MANIFEST_DIR"),
            "tile_merge_240.txt",
            &out,
        );
    });
}

/// WS6.4d(1): the planner, driven by rsact's **own** damage rather than an
/// invented partition — the first end-to-end measurement of what a real frame
/// costs once the regions are the ones we would actually paint.
///
/// Three damage shapes, produced by real widget writes, because they are the
/// three cases the area test has to tell apart:
///
/// - **adjacent** — two neighbouring rows toggle, 22 px apart. Their union is
///   barely larger than the parts (×1.19), so they merge — *if* the surface can
///   hold the result.
/// - **far** — the first and last rows toggle. The union is half the screen, so
///   they must stay separate; merging here is the mistake that turns a 2% frame
///   into a full one.
/// - **all** — every row toggles. Six rects that cascade into one tall region
///   when there is room, and stay six when there is not.
///
/// Measured under **two** policies, which is the point of the table: `whole` is
/// an unbounded surface (a GPU, a host renderer, a full framebuffer) where only
/// the area test speaks, and `tile-24` is a 240×24 = 11.25 KiB tile — the
/// embedded case, where capacity vetoes merges the area test would make. The
/// veto is not cosmetic: without it `adjacent` merges to 16×38, chunking cuts it
/// at y=24, and the lower checkbox is sliced across both chunks for 8 required
/// ops instead of 5.
///
/// The `band-req` column is what a strip renderer would repaint for the same
/// damage (every 24-row band the damage touches) — WS6.4d(1)'s tight-rect
/// decision restated as a number on a real frame.
///
/// `nodes`/`visits` carry the traversal term, and they are here because they
/// **corrected** `RegionLimits::max_regions`' stated justification. WS6.4a
/// measured traversal at ×1.19–2.44 per region and called it the worse term,
/// which reads as "a region costs a full tree walk" — true then, and true of
/// the *partition* schedules it measured, but not of damage regions after
/// WS6.4c's prune. Six tight regions here visit 18 nodes on a 19-node page (3
/// each: root, row, widget), i.e. all six together cost less than one full
/// walk. The budget therefore bounds per-region *fixed* cost, not N× traversal.
#[test]
fn the_planner_turns_real_damage_into_regions() {
    with_new_runtime(|_| {
        let viewport = viewport();
        let frame = Rect::new(Point::zero(), viewport);
        let mut checks: Vec<Signal<bool>> =
            (0..6).map(|_| create_signal(false)).collect();
        let mut probe =
            TileProbe::new(viewport, toggle_rows_page(checks.clone()));

        let policies = [
            // Shipping defaults: the region budget is a bound on a finite
            // list, not a plan-shaping knob, so it must not fire here.
            ("whole", RegionLimits::whole()),
            (
                "tile-24",
                RegionLimits::tiled(
                    region_units(240, 24, 1),
                    1,
                    RegionLimits::DEFAULT_MAX_REGIONS,
                ),
            ),
        ];

        let mut out = String::new();
        let _ = writeln!(
            out,
            "{:<10}{:<9}{:>7}{:>9}{:>10}{:>6}{:>7}{:>7}{:>10}{:>10}",
            "frame",
            "policy",
            "rects",
            "planned",
            "coverage",
            "req",
            "nodes",
            "visits",
            "bands",
            "band-req"
        );

        for (label, rows) in [
            ("adjacent", vec![0usize, 1]),
            ("far", vec![0usize, 5]),
            ("all", (0..6).collect::<Vec<_>>()),
        ] {
            let damage = probe.damage_after(|_| {
                for &row in &rows {
                    checks[row].update(|checked| *checked = !*checked);
                }
            });
            assert!(
                !damage.is_empty(),
                "{label}: the writes damaged nothing, so there is nothing to plan"
            );

            // What a strip renderer would paint for the same damage: every
            // 24-row band any damage rect touches.
            let bands = TileSchedule::from_regions(
                frame,
                TileSchedule::rows(frame, 24)
                    .tiles()
                    .iter()
                    .copied()
                    .filter(|band| {
                        damage.tiles().iter().any(|d| d.intersects(band))
                    })
                    .collect::<Vec<_>>(),
            );
            let band_report = ScheduleReport::of(&probe.capture(&bands));

            for (policy, limits) in &policies {
                let planned = TileSchedule::from_regions(
                    frame,
                    plan_regions(damage.tiles(), frame, limits),
                );
                let log = probe.capture(&planned);

                // The plan is a real schedule, so it owes the same soundness as
                // any other: no op it is obliged to draw may go missing, and no
                // region may invent geometry the full frame never produced.
                let violations = tile_invariance(&log);
                assert!(
                    violations.is_empty(),
                    "{label}/{policy}: {} violation(s), first: {}",
                    violations.len(),
                    violations[0]
                );

                let report = ScheduleReport::of(&log);
                assert!(
                    report.total.required <= band_report.total.required,
                    "{label}/{policy}: the plan repaints MORE than the bands \
                     covering the same damage ({} vs {}) — the tight-rect \
                     premise is inverted",
                    report.total.required,
                    band_report.total.required
                );

                // WS6.4a's traversal term, which no op log can see — the cost
                // `max_regions` exists to bound, measured here per region count
                // rather than assumed.
                let visits = probe.visits(&planned);

                let _ = writeln!(
                    out,
                    "{:<10}{:<9}{:>7}{:>9}{:>10.2}{:>6}{:>7}{:>7}{:>10}{:>10}",
                    label,
                    policy,
                    damage.len(),
                    planned.len(),
                    planned.coverage(),
                    report.total.required,
                    visits.nodes,
                    visits.measured,
                    bands.len(),
                    band_report.total.required
                );
            }
        }

        assert_text_golden(
            env!("CARGO_MANIFEST_DIR"),
            "tile_plan_240.txt",
            &out,
        );
    });
}

/// Every region the planner emits must fit the surface it was planned for.
///
/// A plan that exceeds the tile buffer is not a slow frame, it is a buffer
/// overrun — and unlike the coverage property it cannot be caught by looking at
/// the screen. `rsact_render::region` fuzzes this over synthetic damage; this
/// asserts it over the damage real widgets produce, against a surface small
/// enough (48×16) that chunking is unavoidable.
#[test]
fn a_real_plan_never_exceeds_the_surface() {
    with_new_runtime(|_| {
        let viewport = viewport();
        let frame = Rect::new(Point::zero(), viewport);
        let mut checks: Vec<Signal<bool>> =
            (0..6).map(|_| create_signal(false)).collect();
        let mut probe =
            TileProbe::new(viewport, toggle_rows_page(checks.clone()));

        // A 48x16 tile's worth of storage — 768 units at one unit per pixel.
        // What the planner is bound by is that COUNT, so a region may be any
        // shape needing no more than it.
        let surface_units = region_units(48, 16, 1);
        let limits = RegionLimits::tiled(
            surface_units,
            1,
            RegionLimits::DEFAULT_MAX_REGIONS,
        );

        for rows in [vec![0usize], vec![0usize, 5], (0..6).collect::<Vec<_>>()]
        {
            let damage = probe.damage_after(|_| {
                for &row in &rows {
                    checks[row].update(|checked| *checked = !*checked);
                }
            });
            let planned = plan_regions(damage.tiles(), frame, &limits);
            assert!(!planned.is_empty(), "rows {rows:?} damaged nothing");
            for region in &planned {
                assert!(
                    limits.holds(*region),
                    "region {region:?} needs {} units, over the surface's \
                     {surface_units} (damage {:?})",
                    limits.units_of(*region),
                    damage.tiles()
                );
            }
        }
    });
}

/// WS6.4c(E): the traversal prune must hit the modelled floor **exactly**, on
/// every page and every schedule.
///
/// The goldens carry the numbers, but a golden can be re-blessed by accident;
/// this asserts the *invariant* instead. `cullable` is computed straight from
/// the `LayoutModel` ("which nodes could a region reach"), `measured` is what
/// the render walk actually processed — so equality says the implementation and
/// the model agree, and any prune that becomes conservative (or, worse,
/// over-eager) breaks it.
#[test]
fn the_traversal_prune_hits_the_modelled_floor() {
    with_new_runtime(|_| {
        let viewport = Size::new_equal(240);
        let mut probe = TileProbe::new(viewport, mixed_page());

        for schedule in [
            TileSchedule::whole(Rect::new(Point::zero(), viewport)),
            TileSchedule::rows(Rect::new(Point::zero(), viewport), 48),
            TileSchedule::rows(Rect::new(Point::zero(), viewport), 24),
            TileSchedule::grid(
                Rect::new(Point::zero(), viewport),
                Size::new_equal(80),
            ),
        ] {
            let report = probe.visits(&schedule);
            assert_eq!(
                report.measured,
                report.cullable,
                "the walk processed {} nodes over {} regions where the layout \
                 model says {} are reachable — the prune and the model disagree",
                report.measured,
                schedule.len(),
                report.cullable,
            );
            assert!(
                report.measured <= report.visited,
                "pruning made the walk BIGGER: {} > {}",
                report.measured,
                report.visited,
            );
        }
    });
}
