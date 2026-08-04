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
/// WS6.4b's subtree culling (see `VisitReport::escaping`).
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
/// - a **text** change damages the **whole viewport** — and, measured here,
///   `incremental-layout` does **not** change that (identical numbers with the
///   feature on). The reason is a channel mismatch, not a threshold: a `Label`'s
///   text is held by its layout (`ContentLayout::text`) and read during
///   measurement, so it reaches the relayout through the *tracked-read* channel
///   and never marks `ElArena`'s dirty set. WS5.2's incremental path requires a
///   non-empty dirty set, so it falls through to a full recompute, and WS6.1's
///   targeted repaint roots are never computed ⇒ `blanket` ⇒ `full_flush`.
///
/// Consequence for WS6.4d, and why this test exists: the "+0%" interactive claim
/// covers paint-only changes only. A text change is a **cold** frame today, so
/// under tiling it costs the cold multiplier, not zero — which makes this gap a
/// precondition for the interactive case on any text-driven UI. Filed as an
/// ISSUE rather than fixed here (6.4a measures).
///
/// Only the paint-only half is asserted; the text half is recorded in the golden,
/// because it is a number that *should* move and a diff here is the signal.
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

        // Text: recorded, not asserted (see the doc comment) — the number is
        // the same with `incremental-layout` on, which is the finding.
        let relayout =
            probe.damage_after(|_| caption.set(String::from("value 1")));
        let relayout_report = ScheduleReport::of(&probe.capture(&relayout));

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
        assert_text_golden(
            env!("CARGO_MANIFEST_DIR"),
            "tile_damage_240.txt",
            &out,
        );
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
