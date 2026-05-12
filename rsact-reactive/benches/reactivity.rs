use criterion::{
    BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
};
use rsact_reactive::{
    effect::create_effect, memo::create_memo, prelude::*,
    runtime::with_new_runtime, scope::new_scope, trigger::create_trigger,
};

/// Run `f` inside a fresh runtime that is torn down afterwards.
/// This ensures no state leaks between benchmark iterations.
#[inline(always)]
fn fresh<T>(f: impl FnOnce() -> T) -> T {
    with_new_runtime(|_| f())
}

//
// Signal read / write
//

fn signal_create(c: &mut Criterion) {
    c.bench_function("signal/create", |b| {
        b.iter(|| {
            fresh(|| {
                std::hint::black_box(create_signal(0i32));
            })
        })
    });
}

fn signal_write_no_subscriber(c: &mut Criterion) {
    c.bench_function("signal/write_no_subscriber", |b| {
        b.iter(|| {
            fresh(|| {
                let mut s = create_signal(0i32);
                s.set(std::hint::black_box(1));
            })
        })
    });
}

fn signal_read_untracked(c: &mut Criterion) {
    c.bench_function("signal/read_untracked", |b| {
        b.iter(|| {
            fresh(|| {
                let s = create_signal(42i32);
                std::hint::black_box(s.get_untracked());
            })
        })
    });
}

//
// Effect creation & initial run
//

fn effect_create_and_run(c: &mut Criterion) {
    c.bench_function("effect/create_and_run", |b| {
        b.iter(|| {
            fresh(|| {
                let s = create_signal(1i32);
                create_effect(move |_: Option<i32>| s.get());
            })
        })
    });
}

//
// Effect re-runs on signal change (the hot path)
//

fn effect_rerun_single_signal(c: &mut Criterion) {
    c.bench_function("effect/rerun_single_signal", |b| {
        b.iter(|| {
            fresh(|| {
                let mut s = create_signal(0i32);
                create_effect(move |_: Option<()>| {
                    s.get();
                });
                for i in 1..=10 {
                    s.set(std::hint::black_box(i));
                }
            })
        })
    });
}

fn effect_rerun_n_subscribers(c: &mut Criterion) {
    let mut g = c.benchmark_group("effect/rerun_n_subscribers");
    for n in [1usize, 10, 100, 1000] {
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                fresh(|| {
                    let mut s = create_signal(0i32);
                    for _ in 0..n {
                        create_effect(move |_: Option<()>| {
                            s.get();
                        });
                    }
                    s.set(std::hint::black_box(1));
                })
            })
        });
    }
    g.finish();
}

//
// Batch: defer effect flush until the batch ends
//

fn batch_n_writes(c: &mut Criterion) {
    let mut g = c.benchmark_group("batch/n_writes");
    for n in [10usize, 100, 1000] {
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                fresh(|| {
                    let mut s = create_signal(0i32);
                    let run_count = create_signal(0u32);
                    create_effect(move |_: Option<()>| {
                        s.get();
                        run_count.get_untracked();
                    });
                    batch(|| {
                        for i in 0..n {
                            s.set(std::hint::black_box(i as i32));
                        }
                    });
                })
            })
        });
    }
    g.finish();
}

//
// Memo: derived value computation
//

fn memo_create(c: &mut Criterion) {
    c.bench_function("memo/create", |b| {
        b.iter(|| {
            fresh(|| {
                let s = create_signal(1i32);
                std::hint::black_box(create_memo(move || s.get() * 2));
            })
        })
    });
}

fn memo_read_cached(c: &mut Criterion) {
    c.bench_function("memo/read_cached", |b| {
        b.iter(|| {
            fresh(|| {
                let s = create_signal(1i32);
                let m = create_memo(move || s.get() * 2);
                // First read triggers evaluation; subsequent reads return cached value.
                std::hint::black_box(m.get());
                std::hint::black_box(m.get());
                std::hint::black_box(m.get());
            })
        })
    });
}

fn memo_chain_depth(c: &mut Criterion) {
    let mut g = c.benchmark_group("memo/chain_depth");
    for depth in [1usize, 4, 8, 16] {
        g.bench_with_input(
            BenchmarkId::from_parameter(depth),
            &depth,
            |b, &depth| {
                b.iter(|| {
                    fresh(|| {
                        let s = create_signal(0i32);
                        // Build a linear chain of memos: s → m₁ → m₂ → … → mₙ
                        let mut prev: Box<dyn Fn() -> i32> =
                            Box::new(move || s.get());
                        for _ in 0..depth {
                            let m = create_memo(move || prev() + 1);
                            prev = Box::new(move || m.get());
                        }
                        let leaf = prev;
                        // Force evaluation.
                        std::hint::black_box(leaf());
                    })
                })
            },
        );
    }
    g.finish();
}

/// Diamond graph: two memos depend on the same signal, a third memo depends on
/// both. Tests glitch-free / topological ordering.
fn memo_diamond(c: &mut Criterion) {
    c.bench_function("memo/diamond", |b| {
        b.iter(|| {
            fresh(|| {
                let mut s = create_signal(0i32);
                let a = create_memo(move || s.get() + 1);
                let b = create_memo(move || s.get() * 2);
                let c_memo = create_memo(move || a.get() + b.get());
                std::hint::black_box(c_memo.get());
                s.set(std::hint::black_box(1));
                std::hint::black_box(c_memo.get());
            })
        })
    });
}

//
// Trigger: minimal notification without a value
//

fn trigger_notify(c: &mut Criterion) {
    c.bench_function("trigger/notify_one_subscriber", |b| {
        b.iter(|| {
            fresh(|| {
                let t = create_trigger();
                create_effect(move |_: Option<()>| {
                    t.track();
                });
                t.notify();
            })
        })
    });
}

//
// Scope / GC cost
//

fn scope_create_drop_n_signals(c: &mut Criterion) {
    let mut g = c.benchmark_group("scope/create_drop_n_signals");
    for n in [10usize, 100, 1000] {
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                fresh(|| {
                    let scope = new_scope();
                    for _ in 0..n {
                        std::hint::black_box(create_signal(0i32));
                    }
                    drop(scope); // disposes all n signals
                })
            })
        });
    }
    g.finish();
}

fn scope_drop_with_subscriptions(c: &mut Criterion) {
    c.bench_function("scope/drop_effect_with_subscriptions", |b| {
        b.iter(|| {
            fresh(|| {
                let mut s = create_signal(0i32);
                {
                    let scope = new_scope();
                    create_effect(move |_: Option<()>| {
                        s.get();
                    });
                    drop(scope);
                }
                // Writing after dispose must not panic and should be fast.
                s.set(std::hint::black_box(1));
            })
        })
    });
}

//
// Dynamic dependency tracking
//

/// Effect reads one of two signals depending on a condition signal.
/// Tests the overhead of cleanup + re-subscription on each run.
fn dynamic_dependency_switch(c: &mut Criterion) {
    c.bench_function("dynamic/dependency_switch", |b| {
        b.iter(|| {
            fresh(|| {
                let mut cond = create_signal(true);
                let mut a = create_signal(1i32);
                let mut b = create_signal(2i32);
                create_effect(move |_: Option<()>| {
                    if cond.get() {
                        a.get();
                    } else {
                        b.get();
                    }
                });
                // Flip condition → unsubscribes from a, subscribes to b.
                cond.set(false);
                b.set(std::hint::black_box(3));
            })
        })
    });
}

criterion_group! {
    name = reactivity;
    config = Criterion::default().sample_size(200);
    targets =
        signal_create,
        signal_write_no_subscriber,
        signal_read_untracked,
        effect_create_and_run,
        effect_rerun_single_signal,
        effect_rerun_n_subscribers,
        batch_n_writes,
        memo_create,
        memo_read_cached,
        memo_chain_depth,
        memo_diamond,
        trigger_notify,
        scope_create_drop_n_signals,
        scope_drop_with_subscriptions,
        dynamic_dependency_switch,
}
criterion_main!(reactivity);
