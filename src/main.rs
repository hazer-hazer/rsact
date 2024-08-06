use rsact::prelude::*;
use std::println;

fn foo(signal: Signal<i32>) {
    signal.set(123);
}

fn main() {
    let signal = use_signal(1);

    use_effect(move |_| {
        println!("Updated to {}", signal.get());
    });

    let comp = use_computed(move || signal.get());

    foo(signal);

    println!("Computed {}", comp.get());

    // println!("{}", use_static(1) + use_static(2))
}
