//! Reactivity benchmarks for `rsact-reactive`.
//!
//! # Methodology
//!
//! Every benchmark isolates the **operation under test** from runtime
//! construction and graph setup. Criterion's [`Bencher::iter_custom`] lets us
//! place the timing window (`Instant::now()`) around *only* the op loop, while
//! [`with_new_runtime`] builds and tears down the runtime *outside* that window
//! (it returns the measured [`Duration`] out of its closure).
//!
//! Two helpers cover every shape:
//! - [`steady`] — the op does not grow the runtime (reads, writes, notify,
//!   propagation). Build the graph once, time the N-op loop.
//! - [`growing`] — the op allocates new runtime nodes (create/dispose). Chunk
//!   into fresh runtimes so live memory stays bounded, excluding each runtime's
//!   lifecycle from the timing.
//!
//! This is deliberately different from timing `with_new_runtime(|| { setup; op })`
//! as a whole, which is dominated by runtime construction/teardown and hides the
//! per-operation cost.
//!
//! Allocation cost (bytes + alloc-count per op) is measured separately in
//! `benches/allocations.rs`, since a counting global allocator would otherwise
//! also count criterion's own churn.

use criterion::{
    BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
};
use rsact_reactive::{
    effect::create_effect,
    memo::{Memo, create_memo},
    prelude::*,
    runtime::{batch, untrack, with_new_runtime},
    scope::new_scope,
    signal::create_signal,
    trigger::create_trigger,
    write::{SignalSetter, WriteSignal},
};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

// ---------------------------------------------------------------------------
// Timing helpers
// ---------------------------------------------------------------------------

/// Time only `op`, repeated `iters` times, against a graph built once by
/// `setup`. Runtime construction/teardown and `setup` are excluded from the
/// measurement. Use for steady-state ops that do not grow the runtime.
#[inline]
fn steady<S>(
    iters: u64,
    setup: impl FnOnce() -> S,
    mut op: impl FnMut(&mut S),
) -> Duration {
    with_new_runtime(|_| {
        let mut state = setup();
        let start = Instant::now();
        for _ in 0..iters {
            op(&mut state);
        }
        start.elapsed()
    })
}

/// Time `op` repeated `iters` times, but recreate the runtime every `chunk`
/// ops so live memory stays bounded. Use for ops that allocate runtime nodes
/// (create/dispose) where a single runtime would grow without bound.
#[inline]
fn growing(iters: u64, chunk: u64, mut op: impl FnMut()) -> Duration {
    let mut total = Duration::ZERO;
    let mut done = 0u64;
    while done < iters {
        let n = (iters - done).min(chunk);
        total += with_new_runtime(|_| {
            let start = Instant::now();
            for _ in 0..n {
                op();
            }
            start.elapsed()
        });
        done += n;
    }
    total
}

// ===========================================================================
// Group 1 — Primitives
// ===========================================================================

/// A non-trivial `Copy` struct to exercise partial `.with(|s| s.field)` access
/// versus full `get()`/`get_cloned()` (the AGENTS.md guidance in practice).
#[derive(Clone, Copy, PartialEq)]
struct Wide {
    a: u64,
    b: u64,
    c: u64,
    d: u64,
    e: u64,
    f: u64,
}
impl Wide {
    fn new(v: u64) -> Self {
        Self { a: v, b: v, c: v, d: v, e: v, f: v }
    }
}

fn primitives(c: &mut Criterion) {
    let mut g = c.benchmark_group("primitives");

    g.bench_function("signal_read_untracked", |b| {
        b.iter_custom(|iters| {
            steady(
                iters,
                || create_signal(42i32),
                |s| {
                    black_box(s.get_untracked());
                },
            )
        })
    });

    g.bench_function("signal_read_get", |b| {
        b.iter_custom(|iters| {
            steady(
                iters,
                || create_signal(42i32),
                |s| {
                    black_box(s.get());
                },
            )
        })
    });

    // Partial access to one field of a wide struct signal, no clone of the
    // whole value.
    g.bench_function("signal_with_field", |b| {
        b.iter_custom(|iters| {
            steady(
                iters,
                || create_signal(Wide::new(1)),
                |s| {
                    black_box(s.with(|w| w.c));
                },
            )
        })
    });

    // Clone the whole wide struct out (get_cloned) — contrast to with_field.
    g.bench_function("signal_get_cloned_wide", |b| {
        b.iter_custom(|iters| {
            steady(
                iters,
                || create_signal(Wide::new(1)),
                |s| {
                    black_box(s.get_cloned());
                },
            )
        })
    });

    // Pure value mutation, no subscriber notification.
    g.bench_function("signal_write_untracked", |b| {
        b.iter_custom(|iters| {
            let mut i = 0i32;
            steady(
                iters,
                || create_signal(0i32),
                |s| {
                    i = i.wrapping_add(1);
                    s.set_untracked(black_box(i));
                },
            )
        })
    });

    // set + notify with zero subscribers: pure notify/flush overhead.
    g.bench_function("signal_write_notify_no_subs", |b| {
        b.iter_custom(|iters| {
            let mut i = 0i32;
            steady(
                iters,
                || create_signal(0i32),
                |s| {
                    i = i.wrapping_add(1);
                    s.set(black_box(i));
                },
            )
        })
    });

    // Setting a signal to an EQUAL value while an effect is subscribed: the
    // effect still re-runs (no source-side change detection). This is the
    // Phase-2 `set_if_changed` target — expect it to drop to ~0 once added.
    g.bench_function("signal_write_noop_equal_1_effect", |b| {
        b.iter_custom(|iters| {
            steady(
                iters,
                || {
                    let s = create_signal(0i32);
                    create_effect(move |_: Option<()>| {
                        black_box(s.get());
                    });
                    s
                },
                |s| {
                    s.set(black_box(0i32)); // same value every time
                },
            )
        })
    });

    // Change-detecting write of an EQUAL value with an effect subscribed: with
    // source-side change detection the effect must NOT re-run (contrast to
    // `signal_write_noop_equal_1_effect`, which always notifies).
    g.bench_function("signal_set_if_changed_noop_1_effect", |b| {
        b.iter_custom(|iters| {
            steady(
                iters,
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
            )
        })
    });

    // Change-detecting write of a CHANGED value each time: pays the equality
    // check and then propagates (should track `effect_rerun_1_signal`).
    g.bench_function("signal_set_if_changed_real_1_effect", |b| {
        b.iter_custom(|iters| {
            let mut i = 0i32;
            steady(
                iters,
                || {
                    let s = create_signal(0i32);
                    create_effect(move |_: Option<()>| {
                        black_box(s.get());
                    });
                    s
                },
                |s| {
                    i = i.wrapping_add(1);
                    black_box(s.set_if_changed(black_box(i)));
                },
            )
        })
    });

    g.bench_function("memo_read_cached", |b| {
        b.iter_custom(|iters| {
            steady(
                iters,
                || {
                    let s = create_signal(1i32);
                    let m = create_memo(move || s.get() * 2);
                    black_box(m.get()); // prime the cache
                    m
                },
                |m| {
                    black_box(m.get());
                },
            )
        })
    });

    // Read a memo after its source changed each iteration: forces recompute.
    g.bench_function("memo_recompute_on_change", |b| {
        b.iter_custom(|iters| {
            let mut i = 0i32;
            with_new_runtime(|_| {
                let mut s = create_signal(0i32);
                let m = create_memo(move || s.get() * 2);
                black_box(m.get());
                let start = Instant::now();
                for _ in 0..iters {
                    i = i.wrapping_add(1);
                    s.set(black_box(i));
                    black_box(m.get());
                }
                start.elapsed()
            })
        })
    });

    g.bench_function("trigger_notify_no_subs", |b| {
        b.iter_custom(|iters| {
            steady(
                iters,
                || create_trigger(),
                |t| {
                    t.notify();
                },
            )
        })
    });

    // Overhead of untrack() wrapping a read vs a bare read.
    g.bench_function("untrack_read", |b| {
        b.iter_custom(|iters| {
            steady(
                iters,
                || create_signal(42i32),
                |s| {
                    let s = *s;
                    black_box(untrack(move || s.get()));
                },
            )
        })
    });

    g.finish();
}

// ===========================================================================
// Group 2 — Graph shapes (write → propagation)
// ===========================================================================

fn graph_shapes(c: &mut Criterion) {
    // One effect subscribed to one signal; drive the signal.
    c.bench_function("graph/effect_rerun_1_signal", |b| {
        b.iter_custom(|iters| {
            let mut i = 0i32;
            steady(
                iters,
                || {
                    let s = create_signal(0i32);
                    create_effect(move |_: Option<()>| {
                        black_box(s.get());
                    });
                    s
                },
                |s| {
                    i = i.wrapping_add(1);
                    s.set(black_box(i));
                },
            )
        })
    });

    // Wide fan-out: N effects on one signal (theme/viewport → every widget).
    {
        let mut g = c.benchmark_group("graph/effect_rerun_n_subscribers");
        for n in [1usize, 10, 100, 1000] {
            g.throughput(Throughput::Elements(n as u64));
            g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
                b.iter_custom(|iters| {
                    let mut i = 0i32;
                    steady(
                        iters,
                        || {
                            let s = create_signal(0i32);
                            for _ in 0..n {
                                create_effect(move |_: Option<()>| {
                                    black_box(s.get());
                                });
                            }
                            s
                        },
                        |s| {
                            i = i.wrapping_add(1);
                            s.set(black_box(i));
                        },
                    )
                })
            });
        }
        g.finish();
    }

    // Diamond: b,c depend on a; d depends on both. Glitch-free single recompute.
    c.bench_function("graph/memo_diamond", |b| {
        b.iter_custom(|iters| {
            let mut i = 0i32;
            with_new_runtime(|_| {
                let mut s = create_signal(0i32);
                let a = create_memo(move || s.get() + 1);
                let bb = create_memo(move || s.get() * 2);
                let d = create_memo(move || a.get() + bb.get());
                black_box(d.get());
                let start = Instant::now();
                for _ in 0..iters {
                    i = i.wrapping_add(1);
                    s.set(black_box(i));
                    black_box(d.get());
                }
                start.elapsed()
            })
        })
    });

    // Deep linear memo chain: s → m1 → … → mN, drive s, read the leaf.
    {
        let mut g = c.benchmark_group("graph/memo_chain_depth");
        for depth in [1usize, 4, 8, 16] {
            g.bench_with_input(
                BenchmarkId::from_parameter(depth),
                &depth,
                |b, &depth| {
                    b.iter_custom(|iters| {
                        let mut i = 0i32;
                        with_new_runtime(|_| {
                            let mut s = create_signal(0i32);
                            let mut prev: Memo<i32> = s.map(|v| *v);
                            for _ in 0..depth {
                                let p = prev;
                                prev = create_memo(move || p.get() + 1);
                            }
                            let leaf = prev;
                            black_box(leaf.get());
                            let start = Instant::now();
                            for _ in 0..iters {
                                i = i.wrapping_add(1);
                                s.set(black_box(i));
                                black_box(leaf.get());
                            }
                            start.elapsed()
                        })
                    })
                },
            );
        }
        g.finish();
    }

    // Wide fan-in: one memo reads N signals; change one signal, read the memo.
    {
        let mut g = c.benchmark_group("graph/memo_fan_in");
        for n in [1usize, 10, 100] {
            g.throughput(Throughput::Elements(n as u64));
            g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
                b.iter_custom(|iters| {
                    let mut i = 0i32;
                    with_new_runtime(|_| {
                        let sigs: Vec<_> =
                            (0..n).map(|_| create_signal(0i32)).collect();
                        let read = sigs.clone();
                        let m = create_memo(move || {
                            read.iter().map(|s| s.get()).sum::<i32>()
                        });
                        black_box(m.get());
                        let mut driver = sigs[0];
                        let start = Instant::now();
                        for _ in 0..iters {
                            i = i.wrapping_add(1);
                            driver.set(black_box(i));
                            black_box(m.get());
                        }
                        start.elapsed()
                    })
                })
            });
        }
        g.finish();
    }

    // Memo fan-out: N memos each derived from one shared signal; change the
    // signal and read all memos (styles/theme recompute pattern).
    {
        let mut g = c.benchmark_group("graph/memo_fan_out");
        for n in [1usize, 10, 100] {
            g.throughput(Throughput::Elements(n as u64));
            g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
                b.iter_custom(|iters| {
                    let mut i = 0i32;
                    with_new_runtime(|_| {
                        let mut s = create_signal(0i32);
                        let memos: Vec<_> = (0..n)
                            .map(|k| {
                                let k = k as i32;
                                create_memo(move || s.get() + k)
                            })
                            .collect();
                        for m in &memos {
                            black_box(m.get());
                        }
                        let start = Instant::now();
                        for _ in 0..iters {
                            i = i.wrapping_add(1);
                            s.set(black_box(i));
                            for m in &memos {
                                black_box(m.get());
                            }
                        }
                        start.elapsed()
                    })
                })
            });
        }
        g.finish();
    }
}

// ===========================================================================
// Group 3 — Dynamic dependencies
// ===========================================================================

fn dynamic_deps(c: &mut Criterion) {
    // Effect reads a or b depending on a condition; flip the condition each
    // iteration → cleanup + re-subscription churn.
    c.bench_function("dynamic/dependency_switch", |b| {
        b.iter_custom(|iters| {
            with_new_runtime(|_| {
                let mut cond = create_signal(true);
                let a = create_signal(1i32);
                let bb = create_signal(2i32);
                create_effect(move |_: Option<()>| {
                    if cond.get() {
                        black_box(a.get());
                    } else {
                        black_box(bb.get());
                    }
                });
                let mut flip = false;
                let start = Instant::now();
                for _ in 0..iters {
                    flip = !flip;
                    cond.set(black_box(flip));
                }
                start.elapsed()
            })
        })
    });

    // A memo whose dependency set shrinks after a latch, then keeps being
    // driven (should stop recomputing).
    c.bench_function("dynamic/vanishing_dependency", |b| {
        b.iter_custom(|iters| {
            let mut i = 0i32;
            with_new_runtime(|_| {
                let mut s = create_signal(1i32);
                let mut done = create_signal(false);
                let m = create_memo(move || {
                    if done.get() {
                        0
                    } else {
                        let v = s.get();
                        if v > 1_000_000 {
                            done.set(true);
                        }
                        v
                    }
                });
                black_box(m.get());
                let start = Instant::now();
                for _ in 0..iters {
                    i = i.wrapping_add(1);
                    s.set(black_box(i));
                    black_box(m.get());
                }
                start.elapsed()
            })
        })
    });
}

// ===========================================================================
// Group 4 — Scheduling (batch / defer)
// ===========================================================================

fn scheduling(c: &mut Criterion) {
    // Many writes to one signal with a subscribed effect, deferred into a single
    // flush. Measures per-batch overhead amortized over the writes.
    let mut g = c.benchmark_group("scheduling/batch_n_writes");
    for n in [10usize, 100, 1000] {
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_custom(|iters| {
                let mut base = 0i32;
                steady(
                    iters,
                    || {
                        let s = create_signal(0i32);
                        create_effect(move |_: Option<()>| {
                            black_box(s.get());
                        });
                        s
                    },
                    |s| {
                        let mut s = *s;
                        base = base.wrapping_add(1);
                        batch(|| {
                            for k in 0..n as i32 {
                                s.set(black_box(base.wrapping_add(k)));
                            }
                        });
                    },
                )
            })
        });
    }
    g.finish();

    // Same write count without batching: the effect flushes on every write.
    let mut g = c.benchmark_group("scheduling/unbatched_n_writes");
    for n in [10usize, 100] {
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_custom(|iters| {
                let mut base = 0i32;
                steady(
                    iters,
                    || {
                        let s = create_signal(0i32);
                        create_effect(move |_: Option<()>| {
                            black_box(s.get());
                        });
                        s
                    },
                    |s| {
                        base = base.wrapping_add(1);
                        for k in 0..n as i32 {
                            s.set(black_box(base.wrapping_add(k)));
                        }
                    },
                )
            })
        });
    }
    g.finish();
}

// ===========================================================================
// Group 5 — Lifecycle (create / dispose)
// ===========================================================================

fn lifecycle(c: &mut Criterion) {
    c.bench_function("lifecycle/create_signal", |b| {
        b.iter_custom(|iters| {
            growing(iters, 8192, || {
                black_box(create_signal(0i32));
            })
        })
    });

    c.bench_function("lifecycle/create_memo", |b| {
        b.iter_custom(|iters| {
            growing(iters, 8192, || {
                let s = create_signal(1i32);
                black_box(create_memo(move || s.get() * 2));
            })
        })
    });

    c.bench_function("lifecycle/create_effect", |b| {
        b.iter_custom(|iters| {
            growing(iters, 8192, || {
                let s = create_signal(1i32);
                black_box(create_effect(move |_: Option<()>| {
                    black_box(s.get());
                }));
            })
        })
    });

    c.bench_function("lifecycle/create_trigger", |b| {
        b.iter_custom(|iters| {
            growing(iters, 8192, || {
                black_box(create_trigger());
            })
        })
    });

    // Scope create + drop of N signals (page teardown pattern). Build is
    // untimed; only the scope drop (which disposes all N) is measured. Slots
    // free on drop and are reused, so memory stays bounded.
    let mut g = c.benchmark_group("lifecycle/dispose_scope_n_signals");
    for k in [10usize, 100, 1000] {
        g.throughput(Throughput::Elements(k as u64));
        g.bench_with_input(BenchmarkId::from_parameter(k), &k, |b, &k| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                let mut done = 0u64;
                let chunk = (1_000_000u64 / k as u64).max(1);
                while done < iters {
                    let n = (iters - done).min(chunk);
                    with_new_runtime(|_| {
                        for _ in 0..n {
                            let scope = new_scope();
                            for _ in 0..k {
                                black_box(create_signal(0i32));
                            }
                            let start = Instant::now();
                            drop(scope);
                            total += start.elapsed();
                        }
                    });
                    done += n;
                }
                total
            })
        });
    }
    g.finish();

    // Dispose an effect subscribed to a signal: exercises subscriber-edge
    // cleanup in the source on dispose.
    c.bench_function("lifecycle/dispose_effect_with_subscription", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            let mut done = 0u64;
            while done < iters {
                let n = (iters - done).min(100_000);
                with_new_runtime(|_| {
                    let s = create_signal(0i32);
                    for _ in 0..n {
                        let scope = new_scope();
                        create_effect(move |_: Option<()>| {
                            black_box(s.get());
                        });
                        let start = Instant::now();
                        drop(scope);
                        total += start.elapsed();
                    }
                });
                done += n;
            }
            total
        })
    });
}

// ===========================================================================
// Group 6 — rsact-ui patterns
// ===========================================================================

fn ui_patterns(c: &mut Criterion) {
    // Observe-gated redraw: an outer observe (page) wraps N child observes
    // (widgets). Only one leaf signal changes per frame, so only the outer +
    // that one child should re-run — the fine-grained redraw promise.
    {
        let mut g = c.benchmark_group("ui/observe_redraw_1_of_n");
        for n in [4usize, 16, 64] {
            g.throughput(Throughput::Elements(n as u64));
            g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
                b.iter_custom(|iters| {
                    with_new_runtime(|_| {
                        let sigs: Vec<_> =
                            (0..n).map(|_| create_signal(0i32)).collect();
                        let render_sigs = sigs.clone();
                        let outer = create_probe();
                        let children: Vec<Probe> =
                            (0..n).map(|_| create_probe()).collect();
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
                        render(); // initial build
                        let mut driver = sigs[0];
                        let mut k = 0i32;
                        let start = Instant::now();
                        for _ in 0..iters {
                            k = k.wrapping_add(1);
                            driver.set(black_box(k)); // dirty child 0 only
                            render();
                        }
                        start.elapsed()
                    })
                })
            });
        }
        g.finish();
    }

    // Per-frame re-invocation of an unchanged observe tree: nothing dirty, so
    // the whole tree should skip. Measures the "idle frame" cost.
    c.bench_function("ui/observe_noop_frame", |b| {
        b.iter_custom(|iters| {
            with_new_runtime(|_| {
                let s = create_signal(0i32);
                let outer = create_probe();
                let children: Vec<Probe> =
                    (0..16).map(|_| create_probe()).collect();
                let render = move || {
                    outer.poll(false, || {
                        for i in 0..16 {
                            children[i].poll(false, || {
                                black_box(s.get());
                            });
                        }
                    });
                };
                render();
                let start = Instant::now();
                for _ in 0..iters {
                    render(); // no signal changed → should skip
                }
                start.elapsed()
            })
        })
    });

    // Reactive-on-write upgrade: a layout property starts inert and is bound to
    // a signal once (MaybeSignal Inert → Signal via set_from). Growing op (each
    // upgrade wires a persistent effect), so chunk to bound memory.
    c.bench_function("ui/reactive_on_write_upgrade", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            let mut done = 0u64;
            while done < iters {
                let n = (iters - done).min(100_000);
                with_new_runtime(|_| {
                    let src = create_signal(0i32);
                    let start = Instant::now();
                    for _ in 0..n {
                        let mut m = MaybeSignal::new_inert(0i32);
                        m.set_from(src.maybe_reactive());
                        black_box(&m);
                    }
                    total += start.elapsed();
                });
                done += n;
            }
            total
        })
    });

    // Partial field access on a wide struct signal, inside an observer that
    // re-runs when the struct changes (layout/style struct pattern).
    c.bench_function("ui/struct_with_field_reactive", |b| {
        b.iter_custom(|iters| {
            let mut i = 0u64;
            with_new_runtime(|_| {
                let mut s = create_signal(Wide::new(0));
                create_effect(move |_: Option<()>| {
                    black_box(s.with(|w| w.c));
                });
                let start = Instant::now();
                for _ in 0..iters {
                    i = i.wrapping_add(1);
                    s.set(black_box(Wide::new(i)));
                }
                start.elapsed()
            })
        })
    });
}

criterion_group! {
    name = reactivity;
    config = Criterion::default().sample_size(100);
    targets =
        primitives,
        graph_shapes,
        dynamic_deps,
        scheduling,
        lifecycle,
        ui_patterns,
}
criterion_main!(reactivity);
