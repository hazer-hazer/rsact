//! Allocation profile for `rsact-reactive` — how many heap allocations and how
//! many bytes each reactive operation costs.
//!
//! On an embedded/no_std target the *number* of allocations (and the heap
//! churn/fragmentation it causes) often matters more than raw nanoseconds, so
//! this is a first-class benchmark rather than an afterthought.
//!
//! This lives in its own binary because it installs a **counting global
//! allocator**: run under criterion it would also count criterion's own churn.
//! Unlike `cap` (which reports *currently-allocated* bytes and so misses
//! transient allocations that are freed within the op — exactly the churn we
//! target), this counts every `alloc`/`realloc` call and the bytes requested,
//! so a freed-immediately allocation still shows up.
//!
//! Run with:  `cargo bench -p rsact-reactive --features std --bench allocations`
//! (or `cargo run`). Numbers are per-operation, measured after a short warm-up
//! so first-time allocations (initial subscribe, lazily-grown buffers) don't
//! skew the average. Compare tables before/after a change by eye.

use rsact_reactive::alloc_probe::{self, Tracking};
use rsact_reactive::{
    effect::create_effect,
    memo::create_memo,
    prelude::*,
    runtime::{batch, with_new_runtime},
    scope::new_scope,
    signal::create_signal,
    trigger::create_trigger,
    write::{SignalSetter, WriteSignal},
};
use std::hint::black_box;

// ---------------------------------------------------------------------------
// Counting global allocator — shared with metrics-probe (WS0.7j) so the bench
// and the snapshot tool count identically.
// ---------------------------------------------------------------------------

#[global_allocator]
static GLOBAL: Tracking = Tracking;

// ---------------------------------------------------------------------------
// Measurement harness
// ---------------------------------------------------------------------------

const WARMUP: usize = 16;
const ITERS: usize = 4096;

fn row(name: &str, allocs: f64, bytes: f64) {
    println!("  {name:<38} {allocs:>8.3}     {bytes:>9.2}");
}

/// Measure allocations of `op` (steady-state) against a graph built once by
/// `setup`. Setup + warm-up allocations are excluded.
fn measure<S>(
    name: &str,
    setup: impl FnOnce() -> S,
    mut op: impl FnMut(&mut S),
) {
    with_new_runtime(|_| {
        let mut s = setup();
        for _ in 0..WARMUP {
            op(&mut s);
        }
        let a0 = alloc_probe::read().allocs;
        let b0 = alloc_probe::read().bytes;
        for _ in 0..ITERS {
            op(&mut s);
        }
        let da = alloc_probe::read().allocs - a0;
        let db = alloc_probe::read().bytes - b0;
        row(name, da as f64 / ITERS as f64, db as f64 / ITERS as f64);
    });
}

/// Measure allocations of `op` for ops that create runtime nodes.
fn measure_create(name: &str, mut op: impl FnMut()) {
    with_new_runtime(|_| {
        for _ in 0..WARMUP {
            op();
        }
        let a0 = alloc_probe::read().allocs;
        let b0 = alloc_probe::read().bytes;
        for _ in 0..ITERS {
            op();
        }
        let da = alloc_probe::read().allocs - a0;
        let db = alloc_probe::read().bytes - b0;
        row(name, da as f64 / ITERS as f64, db as f64 / ITERS as f64);
    });
}

fn header(group: &str) {
    println!("\n{group}");
    println!("  {:<38} {:>8}     {:>9}", "operation", "allocs/op", "bytes/op");
    println!("  {}", "-".repeat(60));
}

fn main() {
    println!(
        "rsact-reactive allocation profile ({ITERS} iters/op, {WARMUP} warm-up)"
    );

    header("primitives");
    measure(
        "signal_read_untracked",
        || create_signal(42i32),
        |s| {
            black_box(s.get_untracked());
        },
    );
    measure(
        "signal_read_get",
        || create_signal(42i32),
        |s| {
            black_box(s.get());
        },
    );
    {
        let mut i = 0i32;
        measure(
            "signal_write_notify_no_subs",
            || create_signal(0i32),
            move |s| {
                i = i.wrapping_add(1);
                s.set(black_box(i));
            },
        );
    }
    measure(
        "signal_write_noop_equal_1_effect",
        || {
            let s = create_signal(0i32);
            create_effect(move |_: Option<()>| {
                black_box(s.get());
            });
            s
        },
        |s| {
            s.set(black_box(0i32));
        },
    );
    measure(
        "set_if_changed_noop_1_effect",
        || {
            let s = create_signal(0i32);
            create_effect(move |_: Option<()>| {
                black_box(s.get());
            });
            s
        },
        |s| {
            black_box(s.set_if_changed(black_box(0i32)));
        },
    );
    measure(
        "memo_read_cached",
        || {
            let s = create_signal(1i32);
            let m = create_memo(move || s.get() * 2);
            black_box(m.get());
            m
        },
        |m| {
            black_box(m.get());
        },
    );
    {
        let mut i = 0i32;
        measure(
            "memo_recompute_on_change",
            || {
                let s = create_signal(0i32);
                let m = create_memo(move || s.get() * 2);
                black_box(m.get());
                (s, m)
            },
            move |(s, m)| {
                i = i.wrapping_add(1);
                s.set(black_box(i));
                black_box(m.get());
            },
        );
    }
    measure(
        "trigger_notify_no_subs",
        || create_trigger(),
        |t| {
            t.notify();
        },
    );

    header("graph shapes (write → propagation)");
    {
        let mut i = 0i32;
        measure(
            "effect_rerun_1_signal",
            || {
                let s = create_signal(0i32);
                create_effect(move |_: Option<()>| {
                    black_box(s.get());
                });
                s
            },
            move |s| {
                i = i.wrapping_add(1);
                s.set(black_box(i));
            },
        );
    }
    for n in [10usize, 100] {
        let mut i = 0i32;
        measure(
            &format!("effect_rerun_{n}_subscribers"),
            || {
                let s = create_signal(0i32);
                for _ in 0..n {
                    create_effect(move |_: Option<()>| {
                        black_box(s.get());
                    });
                }
                s
            },
            move |s| {
                i = i.wrapping_add(1);
                s.set(black_box(i));
            },
        );
    }
    {
        let mut i = 0i32;
        measure(
            "memo_diamond_update",
            || {
                let s = create_signal(0i32);
                let a = create_memo(move || s.get() + 1);
                let b = create_memo(move || s.get() * 2);
                let d = create_memo(move || a.get() + b.get());
                black_box(d.get());
                (s, d)
            },
            move |(s, d)| {
                i = i.wrapping_add(1);
                s.set(black_box(i));
                black_box(d.get());
            },
        );
    }

    header("scheduling");
    {
        let mut base = 0i32;
        measure(
            "batch_100_writes_1_effect",
            || {
                let s = create_signal(0i32);
                create_effect(move |_: Option<()>| {
                    black_box(s.get());
                });
                s
            },
            move |s| {
                let mut s = *s;
                base = base.wrapping_add(1);
                batch(|| {
                    for k in 0..100i32 {
                        s.set(black_box(base.wrapping_add(k)));
                    }
                });
            },
        );
    }

    header("lifecycle (create)");
    measure_create("create_signal", || {
        black_box(create_signal(0i32));
    });
    measure_create("create_memo", || {
        let s = create_signal(1i32);
        black_box(create_memo(move || s.get() * 2));
    });
    measure_create("create_effect", || {
        let s = create_signal(1i32);
        black_box(create_effect(move |_: Option<()>| {
            black_box(s.get());
        }));
    });
    measure_create("create_trigger", || {
        black_box(create_trigger());
    });
    measure_create("scope_10_signals_create+drop", || {
        let scope = new_scope();
        for _ in 0..10 {
            black_box(create_signal(0i32));
        }
        drop(scope);
    });

    header("rsact-ui patterns");
    {
        // Observe-gated redraw: outer + 16 child observes; dirty one leaf/frame.
        with_new_runtime(|_| {
            let sigs: Vec<_> = (0..16).map(|_| create_signal(0i32)).collect();
            let render_sigs = sigs.clone();
            let outer = create_probe();
            let children: Vec<Probe> =
                (0..16).map(|_| create_probe()).collect();
            let render = move || {
                outer.poll(false, || {
                    for (i, s) in render_sigs.iter().enumerate() {
                        let s = *s;
                        children[i].poll(false, move || {
                            black_box(s.get());
                        });
                    }
                });
            };
            let mut driver = sigs[0];
            render();
            for w in 0..WARMUP as i32 {
                driver.set(w + 1);
                render();
            }
            let a0 = alloc_probe::read().allocs;
            let b0 = alloc_probe::read().bytes;
            let mut k = 0i32;
            for _ in 0..ITERS {
                k = k.wrapping_add(1);
                driver.set(black_box(k));
                render();
            }
            let da = alloc_probe::read().allocs - a0;
            let db = alloc_probe::read().bytes - b0;
            row(
                "observe_redraw_1_of_16",
                da as f64 / ITERS as f64,
                db as f64 / ITERS as f64,
            );
        });
    }
    {
        // Idle frame: re-invoke an unchanged observe tree (nothing dirty).
        with_new_runtime(|_| {
            let s = create_signal(0i32);
            let outer2 = create_probe();
            let children2: Vec<Probe> =
                (0..16).map(|_| create_probe()).collect();
            let render = move || {
                outer2.poll(false, || {
                    for i in 0..16 {
                        children2[i].poll(false, || {
                            black_box(s.get());
                        });
                    }
                });
            };
            render();
            for _ in 0..WARMUP {
                render();
            }
            let a0 = alloc_probe::read().allocs;
            let b0 = alloc_probe::read().bytes;
            for _ in 0..ITERS {
                render();
            }
            let da = alloc_probe::read().allocs - a0;
            let db = alloc_probe::read().bytes - b0;
            row(
                "observe_noop_frame_16",
                da as f64 / ITERS as f64,
                db as f64 / ITERS as f64,
            );
        });
    }
    measure_create("reactive_on_write_upgrade", || {
        let src = create_signal(0i32);
        let mut m = MaybeSignal::new_inert(0i32);
        m.set_from(src.maybe_reactive());
        black_box(&m);
    });

    println!();
}
