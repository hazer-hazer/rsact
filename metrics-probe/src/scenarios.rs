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
    el::ctx::Wtf,
    prelude::*,
    ui::{UI, WithPages},
};
use std::hint::black_box;

/// The headless widget context: no-op renderer, unit page-id / stylist / event.
/// Matches the `Wtf<NullRenderer, (), (), ()>` used by rsact-ui's own page
/// tests, so pages build and the redraw gate runs without a display.
type NullWtf = Wtf<NullRenderer, (), (), ()>;

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
        total: p.stored
            + p.signals
            + p.effects
            + p.memos
            + p.computed
            + p.observers,
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
        let heap_live_bytes = alloc::live() - base_live;
        let heap_peak_bytes = alloc::peak() - base_live;

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

        // Signals the labels read, kept so we can dirty one for the change frame.
        let labels: Vec<Signal<String>> = (0..n)
            .map(|i| create_signal(format!("label {i}")))
            .collect();
        let init_labels = labels.clone();

        let (build_allocs, build_bytes, mut ui) = charge(|| {
            let mut ui: UI<NullWtf, _> =
                UI::new((), NullRenderer).with_page((), move || {
                    Flex::col(
                        init_labels
                            .iter()
                            .map(|s| Label::new(*s).el())
                            .collect::<Vec<_>>(),
                    )
                    .el()
                });
            // Force the active page to build its arena + layout tree.
            let _ = ui.current_page();
            ui
        });

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
        let heap_live_bytes = alloc::live() - base_live;
        let heap_peak_bytes = alloc::peak() - base_live;

        // Idle frame: re-run the gate with nothing dirty (expect ~0 allocs).
        let idle = painted
            .then(|| {
                guarded_frame(&mut ui, |ui| {
                    ui.current_page().use_renderer(|_| {});
                })
            })
            .flatten();

        // Change frame: dirty one label, re-run the gate.
        let mut driver = labels[0];
        driver.set("changed".into());
        let change = painted
            .then(|| {
                guarded_frame(&mut ui, |ui| {
                    ui.current_page().use_renderer(|_| {});
                })
            })
            .flatten();

        Scenario {
            name: format!("ui_labels_{n}"),
            counts,
            heap_live_bytes,
            heap_peak_bytes,
            build_allocs,
            build_bytes,
            idle_frame_allocs: idle,
            change_frame_allocs: change,
            layout: None,
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
