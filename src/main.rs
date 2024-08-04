use std::println;

use rsact::{effect::create_effect, signal::create_signal};

fn main() {
    let signal = create_signal(1);

    create_effect(move |_| {
        println!("Updated to {}", signal.get());
    });

    // signal.set(123);
}
