use cap::Cap;
use rsact_reactive::{
    runtime::{create_runtime, with_new_runtime},
    signal::{Signal, create_signal},
    storage::StoredValue,
};
use tinyvec::TinyVec;
use std::{alloc::System, cell::RefCell, collections::BTreeSet, rc::Rc};

#[global_allocator]
static GLOBAL: Cap<System> = Cap::new(System, usize::MAX);

const SIGNALS_COUNT: usize = 100_000;

fn with_heap_use(f: impl Fn()) -> usize {
    let mem_start = GLOBAL.allocated();

    f();

    GLOBAL.allocated() - mem_start
}

fn main() {
    println!("StoredValue size: {}B", size_of::<StoredValue>());

    println!("Unit Signal size: {}b", size_of::<Signal<()>>() * 8);

    // println!("bool size: {}B", size_of::<bool>());

    let empty_runtime_size = with_heap_use(|| {
        let _ = create_runtime();
    });
    println!("Empty runtime size: {}B", empty_runtime_size);

    println!("Size of RefCell: {}B", size_of::<RefCell<()>>());
    println!("Size of Rc: {}B", size_of::<Rc<()>>());
    println!("Size of Vec: {}B", size_of::<Vec<()>>());
    println!("Size of BTreeSet: {}B", size_of::<BTreeSet<()>>());
    println!("Size of TinyVec: {}B", size_of::<TinyVec<[(); 0]>>());

    {
        let single_signal_size = with_heap_use(|| {
            create_signal(());
        });

        println!("Single signal size: {}B", single_signal_size);
    }

    {
        let mem_use = with_heap_use(|| {
            (0..SIGNALS_COUNT).for_each(|_| {
                create_signal(());
            });
        });

        println!(
            "Memory used for runtime with {SIGNALS_COUNT} zero-sized signals is: {:.2}MB",
            mem_use as f32 / 1_000_000.0
        );
        println!(
            "Mean memory overhead per signal is: ~{}B",
            ((mem_use as f32 - empty_runtime_size as f32)
                / SIGNALS_COUNT as f32)
                .round()
        );
    }
}
