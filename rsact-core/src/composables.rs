use crate::{
    effect::use_effect,
    signal::{
        marker::{self, Rw},
        Signal, StaticSignal, WriteSignal as _,
    },
};

pub fn use_signal<T: 'static>(value: T) -> Signal<T> {
    Signal::new(value)
}

pub fn use_static<T: 'static>(value: T) -> StaticSignal<T> {
    StaticSignal::new(value)
}

pub type Computed<T> = Signal<T, marker::ReadOnly>;

/// Recomputes every time reactive values inside change
pub fn use_computed<T, F>(f: F) -> Computed<T>
where
    T: 'static,
    F: Fn() -> T + 'static,
{
    let signal = use_signal(f());

    use_effect(move |_| {
        signal.set(f());
    });

    signal.read_only()
}

/// Recomputes every time reactive values inside change and don't equal to
/// previous
pub fn use_memo<T, F>(f: F) -> Signal<T, marker::Rw>
where
    T: 'static + PartialEq + Clone,
    F: Fn() -> T + 'static,
{
    let signal = use_signal(f());

    use_effect(move |prev| {
        let value = f();
        if prev.map(|prev| prev == value).unwrap_or(false) {
            signal.set(value.clone());
        }
        value
    });

    signal
}

// TODO:
// - `use_reactive`

// Useless without scoped reactive context, without it it is the same as an
// effect which uses some signals TODO: Scoped watch to avoid leaking of
// subscribers from watch closure? pub fn watch<T: Clone, W, F>(watch: W, f: F)
// where
//     T: 'static,
//     W: Fn() -> Signal<T> + 'static,
//     F: Fn(T) + 'static,
// {
//     let signal = watch();

//     create_effect(move |_| {
//         let value = signal.get_cloned();
//         f(value);
//     });
// }
