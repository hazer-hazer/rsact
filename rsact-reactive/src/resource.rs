use crate::{
    effect::create_effect,
    read::ReadSignal,
    signal::{Signal, create_signal},
};

// /**
//  * Resource is a signal structure with data stored out of reactive system.
//  * For example, it can be some data on the disk.
//  */
// pub struct Resource<T> {
//     signal: Signal<T>,
// }

pub fn create_resource<T: 'static>(
    mut get: impl FnMut() -> T,
    mut set: impl FnMut(&T) + 'static,
) -> Signal<T> {
    let signal = create_signal(get());

    create_effect(move |_| {
        signal.with(|value| set(value));
        // set(signal.get_cloned());
    });

    signal
}
