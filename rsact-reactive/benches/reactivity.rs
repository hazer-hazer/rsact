use criterion::{Criterion, criterion_group, criterion_main};
use rsact_reactive::{
    effect::create_effect, prelude::*, runtime::with_new_runtime,
};

fn single_effect_single_signal() {
    with_new_runtime(|_| {
        let mut signal = create_signal(1);
        create_effect(move |_| {
            signal.get();
        });
        signal.set(2);
    })
}

fn thousand_effects_single_signal() {
    with_new_runtime(|_| {
        let mut signal = create_signal(1);
        for _ in 0..1000 {
            create_effect(move |_| {
                signal.get();
            });
        }
        signal.set(2);
    })
}

fn single_effect_thousand_signals() {
    with_new_runtime(|_| {
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

criterion_group! {
    name = reactivity;
    config = Criterion::default().sample_size(500);
    targets = bench
}
criterion_main!(reactivity);
