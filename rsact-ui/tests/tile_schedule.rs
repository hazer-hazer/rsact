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

/// G3's color reference target: 240x240 RGB565 ST7789, the display whose 112.5
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
            ("whole", RegionLimits::whole()),
            ("tile-24", RegionLimits::tiled(region_units(240, 24, 1), 1)),
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
        let limits = RegionLimits::tiled(surface_units, 1);

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

/// WS6.4d: the ownership contract, driven end to end — an **N-buffered** loop
/// over a surface an eighth the size of the frame.
///
/// This is the shape the whole redesign exists to make expressible, and every
/// line of it is the application's:
///
/// - the buffers are the app's, allocated where the app wants them (here a
///   heap `Vec`; on a device a `StaticCell` in whatever memory region the board
///   calls for);
/// - the renderer is the app's, passed into rsact per call and owned by nobody
///   else — `UI` has no renderer field to hold a surface hostage;
/// - the transport is the app's: `detach` hands back the painted tile *with*
///   the rect to blit it at, and what happens next (a channel, a DMA burst, a
///   blocking `spi.write`) is not rsact's business.
///
/// The assertion is that the tiled loop reconstructs the frame a full-size
/// surface would have painted, pixel for pixel. That is stronger than op-log
/// invariance, which cannot see an addressing mistake: a wrong origin or stride
/// leaves the op log intact and produces a *plausible* image.
#[test]
fn an_n_buffered_loop_paints_the_frame_a_full_surface_would() {
    use embedded_graphics::pixelcolor::Rgb888;
    use rsact_render::{
        blitter::framebuf::FramebufBlitter,
        framebuf::PackedColor,
        raster::eg::EgRasterizer,
        region::{Tiles, Unbounded},
        renderer::RasterRenderer,
    };

    const W: u32 = 64;
    const H: u32 = 64;
    const TILE_H: u32 = 8;
    let viewport = Size::new(W, H);

    /// A full-frame pixel map: what the panel would end up holding.
    ///
    /// A plain struct, not a `RenderTarget` — that trait is gone (WS6.4d). rsact
    /// renders; the caller flushes, and here the caller is the test.
    struct Panel {
        px: Vec<Option<Rgb888>>,
    }
    let blank = || Panel { px: vec![None; (W * H) as usize] };

    /// Blit a detached tile onto the panel: **raw units plus the rect to put
    /// them at**, which is exactly what a real transport receives — an ST7789
    /// takes `CASET`/`RASET` and then the bytes.
    ///
    /// Deliberately not re-wrapping the buffer in a `Framebuf` to reuse
    /// `output_region`. That would let the test lean on rsact's own addressing
    /// to read back what rsact's addressing wrote, which proves nothing; this
    /// asserts the contract from outside — the tile is strided at its **own**
    /// region width, so row `i` starts at `i * region.width`.
    fn blit(panel: &mut Panel, tile: &[u32], region: Rect) {
        let w = region.size.width as usize;
        for row in 0..region.size.height as usize {
            for col in 0..w {
                let x = region.top_left.x + col as i32;
                let y = region.top_left.y + row as i32;
                if x < 0 || y < 0 || x as u32 >= W || y as u32 >= H {
                    continue;
                }
                let color =
                    <Rgb888 as PackedColor>::as_color(&tile[row * w + col], 0);
                panel.px[y as usize * W as usize + x as usize] = Some(color);
            }
        }
    }

    // The layer split's stack, spelled once: an embedded-graphics rasterizer
    // over a framebuffer blitter. Deliberately not hidden behind a crate-level
    // alias — the three parameters ARE the architecture, and a call site that
    // wants a shorter name binds one, as here.
    type Fb = FramebufBlitter<Rgb888, &'static mut [u32]>;
    type Full = RasterRenderer<EgRasterizer, Fb, Unbounded>;
    type Tiled = RasterRenderer<EgRasterizer, Fb, Tiles<W, TILE_H>>;
    type FullWtf = Wtf<Full, (), Theme<Rgb888>, ()>;
    type TiledWtf = Wtf<Tiled, (), Theme<Rgb888>, ()>;

    fn page() -> impl View<FullWtf> {
        Flex::col(vec![
            Label::new("alpha".inert()).into_el(),
            Checkbox::new(true).into_el(),
            Label::new("omega".inert()).into_el(),
        ])
        .fill()
        .gap(4u32)
    }
    // Same tree, other context — `View` is generic over `W`, so this is the
    // same page under a renderer whose surface is an eighth the size.
    fn tiled_page() -> impl View<TiledWtf> {
        Flex::col(vec![
            Label::new("alpha".inert()).into_el(),
            Checkbox::new(true).into_el(),
            Label::new("omega".inert()).into_el(),
        ])
        .fill()
        .gap(4u32)
    }

    // ---- reference: one full-size surface, the whole-frame path ------------
    let reference = with_new_runtime(|_| {
        let mut renderer = Full::with_framebuf(
            EgRasterizer,
            viewport,
            vec![0u32; (W * H) as usize].leak(),
        );
        let mut ui: UI<FullWtf, _> =
            UI::new(Theme::default(), viewport).with_page((), page);
        let mut panel = blank();
        // Eight frames through the one render path. The first is a full
        // invalidate; the rest settle reactive state.
        for _ in 0..8 {
            let mut frame = ui.start_frame(&mut renderer);
            while frame.render(&mut renderer).is_some() {}
        }
        let (_, units, at) = renderer.detach();
        blit(&mut panel, &units, at);
        panel
    });

    // ---- the same frame through an N-buffered tiled loop -------------------
    let tiled = with_new_runtime(|_| {
        // Three tiles in a pool, so the loop really does rotate buffers rather
        // than reuse one — the case a single-buffer test would not exercise.
        const TILE_UNITS: usize = (W * TILE_H) as usize;
        // Three loans in a pool, so the loop really does rotate buffers rather
        // than reuse one. `leak` in a test is a `StaticCell` on a device — both
        // give the `'static` loan `WidgetCtx` requires.
        let mut free: Vec<&'static mut [u32]> =
            (0..3).map(|_| vec![0u32; TILE_UNITS].leak()).collect();

        let mut renderer =
            Tiled::with_framebuf(EgRasterizer, viewport, free.pop().unwrap());
        let mut ui: UI<TiledWtf, _> =
            UI::new(Theme::default(), viewport).with_page((), tiled_page);
        let mut panel = blank();
        let mut regions_painted = 0usize;

        // Same eight frames. The first is a full invalidate, which at a
        // 64x8 budget is the degenerate strip case — eight bands, every widget
        // cut by some boundary.
        for _ in 0..8 {
            let mut frame = ui.start_frame(&mut renderer);
            while frame.render(&mut renderer).is_some() {
                regions_painted += 1;
                // Publish first, acquire second — the ordering that keeps a
                // single-buffer pool from deadlocking (see `RasterRenderer::detach`).
                // `detach` consumes the renderer and hands back a parked one, so
                // the buffer cannot be painted into while the app holds it: that
                // is the type-state, not a convention.
                let (parked, tile, dirty) = renderer.detach();
                blit(&mut panel, &tile, dirty);
                free.push(tile);
                renderer = parked.attach(free.remove(0));
            }
        }

        assert!(
            regions_painted > 1,
            "the tiled loop painted {regions_painted} region(s); a {W}x{H} \
             viewport at a {W}x{TILE_H} budget must take more than one"
        );
        panel
    });

    // Not vacuous: an all-`None` comparison would pass trivially, and an
    // addressing bug is exactly the kind that can leave a map empty.
    let painted = |p: &Panel| p.px.iter().filter(|c| c.is_some()).count();
    assert!(
        painted(&reference) > (W * H) as usize / 3,
        "the reference frame painted only {} of {} pixels",
        painted(&reference),
        W * H
    );

    let mismatches: Vec<usize> = (0..(W * H) as usize)
        .filter(|&i| reference.px[i] != tiled.px[i])
        .collect();
    assert!(
        mismatches.is_empty(),
        "{} of {} pixels differ between a full framebuffer and an N-buffered \
         tiled loop; first at ({}, {}): full {:?} vs tiled {:?}",
        mismatches.len(),
        W * H,
        mismatches[0] % W as usize,
        mismatches[0] / W as usize,
        reference.px[mismatches[0]],
        tiled.px[mismatches[0]],
    );
}

/// WS6.4d: **the loop the examples show, run and asserted against an absolute
/// reference.**
///
/// The examples cannot be compile-checked — every one has been broken since
/// before this workstream for unrelated reasons (ISSUE-5: no CI job builds them)
/// — so a defect in the pattern they demonstrate is invisible. One already
/// shipped that way: a loop flushing a damage *list* against a single buffer,
/// which worked only because that buffer happened to span the frame.
///
/// So the pattern lives here too, verbatim, and is checked against a frame read
/// **without** it. The first version of this test compared a whole-surface run
/// to a tiled one through the same `flush`, which proved nothing: replacing the
/// region stride with the frame width — the exact mistake at issue — left both
/// runs equally wrong and the test green. The reference therefore reads the
/// framebuffer directly, on the one region where stride cannot be ambiguous
/// (the whole viewport, where region width *is* frame width).
///
/// # What this catches, and what it cannot
///
/// Mutation-tested rather than assumed. A wrong **origin** — blitting every
/// region at `(0, 0)` — fails it at 3632 of 4096 pixels. A wrong **stride** —
/// using the frame width instead of the region's — does **not**, and cannot:
/// chunking is width-preserving, so a full-frame damage becomes full-width
/// bands where the two spellings are the same number, and any narrower region
/// comes from damage that is identical under both policies, so a shared helper
/// is wrong identically on both sides and the difference cancels.
///
/// That is a limit of differential testing here, not an oversight. The stride is
/// pinned where it is a contract rather than a convention:
/// `a_narrow_region_is_laid_out_at_its_own_width` in `rsact_render::renderer`.
#[test]
fn the_loop_the_examples_show_paints_the_frame_the_framebuffer_holds() {
    use embedded_graphics::{
        draw_target::DrawTarget, pixelcolor::Rgb888, prelude::OriginDimensions,
    };
    use rsact_render::{
        blitter::framebuf::FramebufBlitter,
        framebuf::PackedColor,
        raster::eg::EgRasterizer,
        region::{Tiles, Unbounded},
        renderer::RasterRenderer,
    };

    const W: u32 = 64;
    const H: u32 = 64;
    const TILE_H: u32 = 8;
    let viewport = Size::new(W, H);

    /// Stands in for the simulator's `SimulatorDisplay` — a real
    /// `embedded-graphics` target, so `fill_contiguous`'s row-major contract is
    /// the genuine one rather than a mock of it.
    struct Panel {
        px: Vec<Option<Rgb888>>,
    }
    impl OriginDimensions for Panel {
        fn size(&self) -> embedded_graphics::geometry::Size {
            embedded_graphics::geometry::Size::new(W, H)
        }
    }
    impl DrawTarget for Panel {
        type Color = Rgb888;
        type Error = core::convert::Infallible;
        fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
        {
            for embedded_graphics::Pixel(p, c) in pixels {
                if p.x >= 0 && p.y >= 0 && (p.x as u32) < W && (p.y as u32) < H
                {
                    self.px[p.y as usize * W as usize + p.x as usize] = Some(c);
                }
            }
            Ok(())
        }
    }

    // ── verbatim from the examples ──────────────────────────────────────────
    fn flush<D: DrawTarget<Color = Rgb888>>(
        display: &mut D,
        units: &[u32],
        at: Rect,
    ) {
        let stride = at.size.width as usize;
        let _ = display.fill_contiguous(
            &embedded_graphics::primitives::Rectangle::new(
                at.top_left.into(),
                at.size.into(),
            ),
            (0..at.size.height as usize).flat_map(|row| {
                (0..stride).map(move |col| {
                    <Rgb888 as PackedColor>::as_color(
                        &units[row * stride + col],
                        0,
                    )
                })
            }),
        );
    }

    type Fb = FramebufBlitter<Rgb888, &'static mut [u32]>;
    type Whole = RasterRenderer<EgRasterizer, Fb, Unbounded>;
    type Tiled = RasterRenderer<EgRasterizer, Fb, Tiles<W, TILE_H>>;

    macro_rules! page {
        () => {
            || {
                Flex::col(vec![
                    Label::new("alpha".inert()).into_el(),
                    Checkbox::new(true).into_el(),
                    Label::new("omega".inert()).into_el(),
                ])
                .fill()
                .gap(4u32)
                .into_el()
            }
        };
    }

    // ── the reference: one full-viewport region, read from the buffer ───────
    //
    // A page's first frame is a full invalidate, so an unbounded policy plans it
    // as exactly one region covering the viewport — asserted below, because the
    // whole point is that this region's width IS the frame width and therefore
    // no stride convention can be got wrong here.
    let reference: Vec<Option<Rgb888>> = with_new_runtime(|_| {
        let mut renderer = Whole::with_framebuf(
            EgRasterizer,
            viewport,
            vec![0u32; (W * H) as usize].leak(),
        );
        let mut ui: UI<Wtf<Whole, (), Theme<Rgb888>, ()>, _> =
            UI::new(Theme::default(), viewport).with_page((), page!());

        let mut frame = ui.start_frame(&mut renderer);
        let at = frame.render(&mut renderer).expect("a first-frame region");
        assert_eq!(
            at,
            Rect::new(Point::zero(), viewport),
            "a page's first frame must plan as the whole viewport under an \
             unbounded policy, or this reference is not stride-unambiguous"
        );
        assert!(
            frame.render(&mut renderer).is_none(),
            "…and as exactly ONE region"
        );
        drop(frame);

        let (_, buf, _) = renderer.detach();
        buf.iter()
            .map(|u| Some(<Rgb888 as PackedColor>::as_color(u, 0)))
            .collect()
    });

    // ── under test: the example loop, tiled, many regions ──────────────────
    let tiled = with_new_runtime(|_| {
        let mut panel = Panel { px: vec![None; (W * H) as usize] };
        let mut renderer = Tiled::with_framebuf(
            EgRasterizer,
            viewport,
            vec![0u32; (W * TILE_H) as usize].leak(),
        );
        let mut ui: UI<Wtf<Tiled, (), Theme<Rgb888>, ()>, _> =
            UI::new(Theme::default(), viewport).with_page((), page!());

        let mut regions = 0;
        {
            let mut frame = ui.start_frame(&mut renderer);
            while frame.render(&mut renderer).is_some() {
                regions += 1;
                let (parked, buf, at) = renderer.detach();
                flush(&mut panel, &buf, at);
                renderer = parked.attach(buf);
            }
        }
        assert!(
            regions > 1,
            "the tiled run took {regions} region(s); with a {W}x{TILE_H} \
             budget a full first frame must take several, or the loop is not \
             being exercised"
        );
        panel
    });

    // Not vacuous: the reference must hold a real picture. An all-background
    // comparison would pass while proving nothing.
    let ink = reference
        .iter()
        .filter(|c| {
            **c != Some(
                <Rgb888 as rsact_render::color::Color>::default_background(),
            )
        })
        .count();
    assert!(
        ink > 64,
        "the reference frame has only {ink} non-background pixels"
    );

    let mismatches: Vec<usize> = (0..(W * H) as usize)
        .filter(|&i| reference[i] != tiled.px[i])
        .collect();
    assert!(
        mismatches.is_empty(),
        "{} of {} pixels differ between the framebuffer and what the example \
         loop put on the display; first at ({}, {}): framebuffer {:?} vs \
         display {:?}",
        mismatches.len(),
        W * H,
        mismatches[0] % W as usize,
        mismatches[0] / W as usize,
        reference[mismatches[0]],
        tiled.px[mismatches[0]],
    );
}
