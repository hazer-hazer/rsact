use crate::{
    memo::Memo,
    signal::{Signal, StaticSignal},
};

#[track_caller]
pub fn create_signal<T: 'static>(value: T) -> Signal<T> {
    Signal::new(value)
}

#[track_caller]
pub fn create_static<T: 'static>(value: T) -> StaticSignal<T> {
    StaticSignal::new(value)
}

#[track_caller]
pub fn create_memo<T: PartialEq + 'static>(
    f: impl Fn(Option<&T>) -> T + 'static,
) -> Memo<T> {
    Memo::new(f)
}

// pub fn use_mapped<T: 'static, U: 'static, G, S>(g: G, s: S) -> Signal<T>
// where
//     G: Fn(T) -> U + 'static,
//     S: Fn(&mut T) + 'static,
// {
//     let signal = use_signal(g());

//     use_effect(move |_| s(g()));

//     signal
// }

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
