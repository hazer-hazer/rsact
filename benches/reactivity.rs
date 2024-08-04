use criterion::{criterion_group, criterion_main, Criterion};
use rsact::{
    effect::create_effect,
    runtime::with_scoped_runtime,
    signal::create_signal,
};

fn single_effect_single_signal() {
    with_scoped_runtime(|_| {
        let signal = create_signal(1);
        create_effect(move |_| {
            signal.get();
        });
        signal.set(2);
    })
}

fn thousand_effects_single_signal() {
    with_scoped_runtime(|_| {
        let signal = create_signal(1);
        for _ in 0..1000 {
            create_effect(move |_| {
                signal.get();
            });
        }
        signal.set(2);
    })
}

fn single_effect_thousand_signals() {
    with_scoped_runtime(|_| {
        let signals = (0..1000).map(|_| create_signal(1)).collect::<Vec<_>>();
        create_effect(move |_| {
            signals.iter().for_each(|signal| {
                signal.get();
            });
        });
    })
}

fn bench(c: &mut Criterion) {
    c.bench_function("single_effect_single_signal", |b| {
        b.iter(single_effect_single_signal)
    });

    c.bench_function("thousand_effects_single_signal", |b| {
        b.iter(thousand_effects_single_signal)
    });

    c.bench_function("single_effect_thousand_signals", |b| {
        b.iter(single_effect_thousand_signals)
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
