//! Reactive-only size probe: exercises the pure reactive engine (signals, memo,
//! effect, probe-gate, writes) so its `.text` reflects the engine footprint —
//! the "reactive-only bin" whose thumbv6m opt-z fat-LTO baseline is ~16.8 KiB.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::{vec::Vec, vec};
use core::hint::black_box;
use cortex_m_rt::entry;
use rsact_reactive::{
    effect::create_effect, memo::create_memo, prelude::*,
    signal::create_signal,
};

#[entry]
fn main() -> ! {
    size_probe::init_heap();

    let sigs: Vec<Signal<i32>> =
        (0..16).map(|_| create_signal(0i32)).collect();

    // A memo over all signals + an effect reading it (change-propagation code).
    let memo_sigs = sigs.clone();
    let m = create_memo(move || {
        memo_sigs.iter().map(|s| s.get()).sum::<i32>()
    });
    create_effect(move |_: Option<()>| {
        black_box(m.get());
    });

    // Probe-gated tree (the page redraw-gate shape; WS2 replaced keyed
    // `observe()` with owned `Probe` handles — mirrors benches/allocations.rs).
    let render_sigs = sigs.clone();
    let outer = create_probe();
    let children: Vec<Probe> = (0..16).map(|_| create_probe()).collect();
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
    render();

    let mut driver = sigs[0];
    driver.set(1);
    render();

    black_box(vec![driver]);
    black_box(&sigs);
    loop {}
}
