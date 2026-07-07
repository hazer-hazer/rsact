//! The measured binaries from the audit, in-process: a pure-reactive
//! observe-gated tree and real widget pages (5 and 10 labels). Each runs in its
//! own reactive runtime so node counts are isolated, and is measured with the
//! [`crate::alloc`] tracking allocator.

use crate::{alloc, snapshot::Scenario};
use rsact_reactive::{
    prelude::*,
    runtime::{current_runtime_profile, observe, with_new_runtime},
};
use rsact_ui::{
    prelude::*,
    test_support::{NullWtf, labels_page},
    ui::{UI, WithPages},
};
use std::hint::black_box;

/// Run `f` and return `(allocs, bytes)` charged while it ran.
fn charge<R>(f: impl FnOnce() -> R) -> (usize, usize, R) {
    let before = alloc::read();
    let r = f();
    let after = alloc::read();
    (after.allocs - before.allocs, after.bytes - before.bytes, r)
}

/// Allocations charged while `f` runs — used for per-frame measurements.
fn frame_allocs(f: impl FnOnce()) -> usize {
    let before = alloc::read();
    f();
    alloc::read().allocs - before.allocs
}

/// Reset the layout counters before a pass we want to attribute (no-op unless
/// built with `layout-counters`).
fn reset_layout() {
    #[cfg(feature = "layout-counters")]
    rsact_ui::layout::counters::reset();
}

/// Read layout counters after a pass (`None` unless built with
/// `layout-counters`).
fn read_layout() -> Option<crate::snapshot::LayoutCounters> {
    #[cfg(feature = "layout-counters")]
    {
        let (visits, measures) = rsact_ui::layout::counters::snapshot();
        return Some(crate::snapshot::LayoutCounters { visits, measures });
    }
    #[cfg(not(feature = "layout-counters"))]
    None
}

fn profile_counts() -> crate::snapshot::NodeCounts {
    let p = current_runtime_profile();
    crate::snapshot::NodeCounts {
        stored: p.stored,
        signals: p.signals,
        effects: p.effects,
        memos: p.memos,
        computed: p.computed,
        observers: p.observers,
        subscribers: p.subscribers,
        subscribers_bindings: p.subscribers_bindings,
        sources: p.sources,
        sources_bindings: p.sources_bindings,
        total: p.total(),
    }
}

/// Pure-reactive scenario: one outer observe + `n` child observes over `n`
/// signals — the shape of a page's redraw gate, with zero UI/render code. This
/// is the "reactive-only bin" whose thumbv6m `.text` baseline is ~16.8 KiB.
fn reactive_only(n: usize) -> Scenario {
    with_new_runtime(|_| {
        alloc::reset_peak();
        let base_live = alloc::live();

        // Build.
        let (build_allocs, build_bytes, (sigs, render)) = charge(|| {
            let sigs: Vec<Signal<i32>> =
                (0..n).map(|_| create_signal(0i32)).collect();
            let render_sigs = sigs.clone();
            let render = move || {
                observe("outer", || {
                    for (i, s) in render_sigs.iter().enumerate() {
                        let s = *s;
                        observe(("child", i), move || {
                            black_box(s.get());
                        });
                    }
                });
            };
            render();
            (sigs, render)
        });

        let counts = profile_counts();
        // Saturating: a scenario that nets a free (base measured mid-churn)
        // must not underflow-panic and abort the whole recording.
        let heap_live_bytes = alloc::live().saturating_sub(base_live);
        let heap_peak_bytes = alloc::peak().saturating_sub(base_live);

        // Idle frame: re-run the gate with nothing dirty.
        let idle = frame_allocs(|| render());

        // Change frame: dirty one leaf, re-run the gate.
        let mut driver = sigs[0];
        driver.set(1);
        let change = frame_allocs(|| render());

        Scenario {
            name: format!("reactive_only_{n}"),
            counts,
            heap_live_bytes,
            heap_peak_bytes,
            build_allocs,
            build_bytes,
            idle_frame_allocs: Some(idle),
            change_frame_allocs: Some(change),
            layout: None,
        }
    })
}

/// A widget page of `n` labels in a column, `n` of them bound to signals so a
/// change frame is measurable. Built headlessly through the public `UI` API
/// with a `NullRenderer`.
fn ui_labels(n: usize) -> Scenario {
    with_new_runtime(|_| {
        alloc::reset_peak();
        let base_live = alloc::live();

        // Build the canonical N-label page (shared with the layout bench). The
        // returned `labels` are kept so we can dirty one for the change frame.
        let (build_allocs, build_bytes, (mut ui, labels)) =
            charge(|| labels_page(n));

        // Warm-up: the first paint is always full work (page starts dirty and
        // the render gate's observe-nodes are created here), so it is not a
        // "frame". Rendering the null theme can hit a pre-existing ColorStyle
        // panic for some widgets, so guard it and degrade to "not measured"
        // rather than aborting the whole probe.
        let painted = guarded_frame(&mut ui, |ui| {
            ui.current_page().use_renderer(|_| {});
        })
        .is_some();

        // Measure node population / heap in the steady state, after first paint.
        let counts = profile_counts();
        // Saturating: a scenario that nets a free (base measured mid-churn)
        // must not underflow-panic and abort the whole recording.
        let heap_live_bytes = alloc::live().saturating_sub(base_live);
        let heap_peak_bytes = alloc::peak().saturating_sub(base_live);

        // Idle frame: re-run the gate with nothing dirty (expect ~0 allocs).
        let idle = painted
            .then(|| {
                guarded_frame(&mut ui, |ui| {
                    ui.current_page().use_renderer(|_| {});
                })
            })
            .flatten();

        // Change frame: dirty one label, re-run the gate. Attribute the layout
        // work of this single-leaf change to the layout counters (whole-tree
        // today; WS5 makes it incremental).
        let mut driver = labels[0];
        driver.set("changed".into());
        reset_layout();
        let change = painted
            .then(|| {
                guarded_frame(&mut ui, |ui| {
                    ui.current_page().use_renderer(|_| {});
                })
            })
            .flatten();
        // Only trust the layout counters if the change frame actually completed;
        // a panicked frame leaves them at 0 (reset, never incremented), which
        // would record a phantom `Some {visits: 0, measures: 0}` and show as a
        // fake −100% "improvement" in a later diff.
        let layout = change.and_then(|_| read_layout());

        Scenario {
            name: format!("ui_labels_{n}"),
            counts,
            heap_live_bytes,
            heap_peak_bytes,
            build_allocs,
            build_bytes,
            idle_frame_allocs: idle,
            change_frame_allocs: change,
            layout,
        }
    })
}

/// Measure a frame, returning `None` if the render gate panics (see the
/// null-theme note above). Uses `catch_unwind`; the closure only touches the
/// UI, and a panic here just drops the optional metric.
fn guarded_frame(
    ui: &mut UI<NullWtf, WithPages>,
    f: impl Fn(&mut UI<NullWtf, WithPages>),
) -> Option<usize> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let before = alloc::read();
    let ok = catch_unwind(AssertUnwindSafe(|| f(ui))).is_ok();
    ok.then(|| alloc::read().allocs - before.allocs)
}

/// Run every scenario, in a stable order.
pub fn run_all() -> Vec<Scenario> {
    vec![reactive_only(16), ui_labels(5), ui_labels(10)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // The tests share process-global state — the tracking allocator and the
    // layout counters — so a parallel libtest run (the default, and what
    // `cargo test --workspace` does) has them clobber each other's measurement
    // windows. This lock makes them non-overlapping regardless of
    // `--test-threads`; poison is ignored (a prior test's failure shouldn't
    // cascade). `--test-threads=1` is no longer required for correctness.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    // WS0.4 node-count regression. Locks the reactive node population and the
    // zero-idle-frame-allocs invariant for the canonical scenarios. Node counts
    // are deterministic (same construction each run) and idle frames must stay
    // allocation-free. If a change moves these, it is either a real regression
    // or an intended shift that should be re-locked here in the same commit.
    //
    // These lock the values measured on 2026-07-07 (see WS0.3b commit); the
    // 10-label total is 52 = ~42 reactive nodes + 10 render-gate observers +
    // page observer — higher than the roadmap's pre-metrics "42" estimate only
    // because the profile now counts observers (WS0.3a).
    #[test]
    fn node_count_regression() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let reactive = reactive_only(16);
        assert_eq!(
            reactive.counts.total, 33,
            "reactive_only_16 node total moved"
        );
        assert_eq!(reactive.counts.signals, 16);
        assert_eq!(reactive.counts.observers, 17);
        assert_eq!(
            reactive.idle_frame_allocs,
            Some(0),
            "reactive idle frame must be allocation-free"
        );

        let ui5 = ui_labels(5);
        assert_eq!(ui5.counts.total, 32, "ui_labels_5 node total moved");
        assert_eq!(
            ui5.idle_frame_allocs,
            Some(0),
            "ui_labels_5 idle frame must be allocation-free (render gate must short-circuit)"
        );

        let ui10 = ui_labels(10);
        assert_eq!(ui10.counts.total, 52, "ui_labels_10 node total moved");
        assert_eq!(
            ui10.counts.observers, 11,
            "one render observer per label + page"
        );
        assert_eq!(
            ui10.idle_frame_allocs,
            Some(0),
            "ui_labels_10 idle frame must be allocation-free"
        );
    }

    // WS0.5 layout-counter baseline. Locks the whole-tree relayout cost of a
    // single leaf change: for the 10-label page every one of the 11 layout
    // nodes is re-visited and text is re-measured 40× — the pre-WS5 pathology
    // (incremental layout should cut both to O(changed path)). Runs only with
    // `--features layout-counters`.
    //
    // (The roadmap cites ~180 visits / 60-120 measures for a *30-node* page;
    // the canonical scenarios here are 5/10 labels, so their absolute numbers
    // are smaller — the locked signal is the same: visits ≈ node count, i.e.
    // the whole tree, per single change.)
    #[cfg(feature = "layout-counters")]
    #[test]
    fn layout_counter_baseline() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let ui10 = ui_labels(10);
        let layout =
            ui10.layout.expect("layout counters present under feature");
        assert_eq!(
            layout.visits, 11,
            "one leaf change relayouts the whole 11-node tree today"
        );
        assert_eq!(
            layout.measures, 40,
            "one leaf change re-measures text 40× today"
        );
    }
}
