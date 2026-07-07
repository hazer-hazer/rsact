//! WS0.5 layout timing benches: `build_and_layout_full` (cold start: build a
//! page + first full layout + paint), `layout_only` (a whole-tree relayout on a
//! built tree, setup outside the timed window), and `layout_leaf_change`
//! (relayout after one label changes). Today all do whole-tree work; WS5's
//! incremental layout should make `layout_leaf_change` diverge sharply from
//! `layout_only`.
//!
//! Run: `cargo bench -p rsact-ui --features "std,embedded-graphics" --bench layout`
//!
//! Timing is noisy compared with the deterministic layout counters in the
//! metrics-probe snapshot (WS0.3/0.5); this is the wall-clock companion.

use criterion::{Criterion, criterion_group, criterion_main};
use rsact_reactive::{prelude::*, runtime::with_new_runtime};
use rsact_ui::{
    el::ctx::Wtf,
    prelude::*,
    ui::{UI, WithPages},
};
use std::{hint::black_box, time::Instant};

type NullWtf = Wtf<NullRenderer, (), (), ()>;

const LABELS: usize = 10;

/// Build a headless page of `n` labels and return the UI plus the label signals.
fn build_ui(n: usize) -> (UI<NullWtf, WithPages>, Vec<Signal<String>>) {
    let labels: Vec<Signal<String>> = (0..n)
        .map(|i| create_signal(format!("label {i}")))
        .collect();
    let init = labels.clone();
    let mut ui: UI<NullWtf, _> =
        UI::new((), NullRenderer).with_page((), move || {
            Flex::col(
                init.iter().map(|s| Label::new(*s).el()).collect::<Vec<_>>(),
            )
            .el()
        });
    let _ = ui.current_page();
    (ui, labels)
}

// Cold-start cost: build the widget tree + first full layout + first paint.
// Named honestly — the three can't be cheaply separated (a "full layout" only
// happens on first paint or a full invalidation, which needs a rebuild), so
// this measures them together. Use `layout_only` for relayout in isolation.
fn build_and_layout_full(c: &mut Criterion) {
    c.bench_function("build_and_layout_full", |b| {
        b.iter(|| {
            with_new_runtime(|_| {
                let (mut ui, _labels) = build_ui(LABELS);
                // First paint lays out the whole tree.
                ui.current_page().use_renderer(|_| {});
                black_box(&mut ui);
            })
        })
    });
}

// A relayout in isolation: build + first paint are untimed setup, then each
// timed iteration invalidates every label (so the layout memo recomputes the
// whole tree) and re-paints. No tree construction or runtime creation in the
// window. Today this is a whole-tree relayout like `layout_leaf_change`; after
// WS5's incremental layout, leaf_change should diverge (fast) while this — a
// genuine full invalidation — stays O(tree).
fn layout_only(c: &mut Criterion) {
    c.bench_function("layout_only", |b| {
        b.iter_custom(|iters| {
            with_new_runtime(|_| {
                let (mut ui, labels) = build_ui(LABELS);
                ui.current_page().use_renderer(|_| {});
                let start = Instant::now();
                for i in 0..iters {
                    let v = if i % 2 == 0 { "a" } else { "b" };
                    for mut label in labels.iter().copied() {
                        label.set(v.into());
                    }
                    ui.current_page().use_renderer(|_| {});
                }
                start.elapsed()
            })
        })
    });
}

fn layout_leaf_change(c: &mut Criterion) {
    c.bench_function("layout_leaf_change", |b| {
        // iter_custom so the build + first paint are untimed setup and only the
        // change+relayout loop is measured, all inside one runtime scope.
        b.iter_custom(|iters| {
            with_new_runtime(|_| {
                let (mut ui, labels) = build_ui(LABELS);
                ui.current_page().use_renderer(|_| {});
                let mut driver = labels[0];
                let start = Instant::now();
                for i in 0..iters {
                    // Alternate the value so the memo never cuts propagation.
                    driver.set(if i % 2 == 0 {
                        "a".into()
                    } else {
                        "b".into()
                    });
                    ui.current_page().use_renderer(|_| {});
                }
                start.elapsed()
            })
        })
    });
}

criterion_group!(
    benches,
    build_and_layout_full,
    layout_only,
    layout_leaf_change
);
criterion_main!(benches);
