use crate::{
    memo::{create_memo, IntoMemo, Memo},
    prelude::MemoChain,
    read::{impl_read_signal_traits, ReadSignal, SignalMap},
    signal::{create_signal, IntoSignal, Signal},
    write::{SignalSetter, WriteSignal},
    ReactiveValue,
};
use alloc::{rc::Rc, vec::Vec};
use core::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

#[repr(transparent)]
#[derive(Clone, Copy)]
// TODO: Maybe rename to Idle/Phlegmatic/Inert/Supine for shortness?
/// Plain data, basically a wrapper around T which you can treat as a real reactive value but it isn't reactive: not tracked and not trackable. Important: Unlike other reactive values, [`Inert`] is not copy-type!
pub struct Inert<T: 'static> {
    value: T,
}

impl_read_signal_traits!(Inert<T>);

impl<T> Inert<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T> From<T> for Inert<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: 'static> ReactiveValue for Inert<T> {
    type Value = T;

    /// [`NonReactive`] is always alive! Useless to call
    fn is_alive(&self) -> bool {
        true
    }

    /// Drop [`NonReactive`]
    fn dispose(self) {
        core::mem::drop(self);
    }
}

impl<T: PartialEq + Clone> IntoMemo<T> for Inert<T> {
    fn memo(self) -> Memo<T> {
        create_memo(move |_| self.value.clone())
    }
}

impl<T> IntoSignal<T> for Inert<T> {
    fn signal(self) -> Signal<T> {
        create_signal(self.value)
    }
}

impl<T> ReadSignal<T> for Inert<T> {
    fn track(&self) {
        // Static signal never changes thus scope does not need to subscribe to
        // its changes
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        f(&self.value)
    }
}

impl<T: PartialEq + 'static> SignalMap<T> for Inert<T> {
    type Output<U: PartialEq + 'static> = Inert<U>;

    fn map<U: PartialEq + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Self::Output<U> {
        map(&self.value).inert()
    }
}

impl<T: PartialEq + 'static> SignalSetter<T, Self> for Inert<T> {
    fn setter(
        &mut self,
        source: Self,
        set_map: impl Fn(&mut T, &<Self as ReactiveValue>::Value) + 'static,
    ) {
        set_map(&mut self.value, &source.value)
    }
}

impl<T> Deref for Inert<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Inert<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

pub trait IntoInert<T> {
    fn inert(self) -> Inert<T>;
}

impl<T> IntoInert<T> for T {
    fn inert(self) -> Inert<T> {
        Inert::from(self)
    }
}

/// Maybe reactive read-only value, i.e. anything from static values to writable signals.
/// For RW version of [`MaybeReactive`] see [`MaybeSignal`]
pub enum MaybeReactive<T: PartialEq + 'static> {
    Inert(Inert<T>),
    Signal(Signal<T>),
    Memo(Memo<T>),
    MemoChain(MemoChain<T>),
    Derived(Rc<dyn Fn() -> T>),
}

impl_read_signal_traits!(MaybeReactive<T>: PartialEq);

impl<T: PartialEq + 'static> MaybeReactive<T> {
    pub fn new_static(value: T) -> Self {
        Self::Inert(value.inert())
    }

    pub fn new_derived(f: impl Fn() -> T + 'static) -> Self {
        Self::Derived(Rc::new(f))
    }
}

impl<T: PartialEq + 'static> ReactiveValue for MaybeReactive<T> {
    type Value = T;

    #[track_caller]
    fn is_alive(&self) -> bool {
        match self {
            MaybeReactive::Inert(static_signal) => static_signal.is_alive(),
            MaybeReactive::Signal(signal) => signal.is_alive(),
            MaybeReactive::Memo(memo) => memo.is_alive(),
            MaybeReactive::MemoChain(memo_chain) => memo_chain.is_alive(),
            MaybeReactive::Derived(_) => true,
        }
    }

    #[track_caller]
    fn dispose(self) {
        match self {
            MaybeReactive::Inert(static_signal) => static_signal.dispose(),
            MaybeReactive::Signal(signal) => signal.dispose(),
            MaybeReactive::Memo(memo) => memo.dispose(),
            MaybeReactive::MemoChain(memo_chain) => memo_chain.dispose(),
            MaybeReactive::Derived(derived) => core::mem::drop(derived),
        }
    }
}

impl<T: PartialEq + 'static> ReadSignal<T> for MaybeReactive<T> {
    #[track_caller]
    fn track(&self) {
        match self {
            MaybeReactive::Inert(_) => {},
            MaybeReactive::Signal(signal) => signal.track(),
            MaybeReactive::Memo(memo) => memo.track(),
            MaybeReactive::MemoChain(memo_chain) => memo_chain.track(),
            MaybeReactive::Derived(_) => {},
        }
    }

    #[track_caller]
    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        match self {
            MaybeReactive::Inert(inert) => f(inert),
            MaybeReactive::Signal(signal) => signal.with_untracked(f),
            MaybeReactive::Memo(memo) => memo.with_untracked(f),
            MaybeReactive::MemoChain(memo_chain) => {
                memo_chain.with_untracked(f)
            },
            MaybeReactive::Derived(derived) => f(&derived()),
        }
    }
}

// TODO: This is inconsistent with MaybeSignal SignalMapper implementation for [`Inert`]. Here it requires `Clone` and allows reactivity, but in MaybeSignal mapper is not reactive.
impl<T: PartialEq + Clone + 'static> SignalMap<T> for MaybeReactive<T> {
    type Output<U: PartialEq + 'static> = MaybeReactive<U>;

    #[track_caller]
    fn map<U: PartialEq + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Self::Output<U> {
        match self {
            MaybeReactive::Inert(inert) => {
                // FIXME: TODO
                let inert = inert.clone();
                MaybeReactive::Derived(Rc::new(move || map(&inert)))
            },
            MaybeReactive::Signal(signal) => {
                MaybeReactive::Memo(signal.map(map))
            },
            MaybeReactive::Memo(memo) => MaybeReactive::Memo(memo.map(map)),
            MaybeReactive::MemoChain(memo_chain) => {
                MaybeReactive::Memo(memo_chain.map(map))
            },
            MaybeReactive::Derived(derived) => {
                let derived = Rc::clone(derived);
                MaybeReactive::Derived(Rc::new(move || map(&derived())))
            },
        }
    }
}

impl<T: PartialEq + 'static> From<Signal<T>> for MaybeReactive<T> {
    fn from(value: Signal<T>) -> Self {
        Self::Signal(value)
    }
}

impl<T: PartialEq + 'static> From<Memo<T>> for MaybeReactive<T> {
    fn from(value: Memo<T>) -> Self {
        Self::Memo(value)
    }
}

impl<T: PartialEq + 'static> From<MemoChain<T>> for MaybeReactive<T> {
    fn from(value: MemoChain<T>) -> Self {
        Self::MemoChain(value)
    }
}

impl<T: PartialEq + 'static, F: Fn() -> T + 'static> From<F>
    for MaybeReactive<T>
{
    fn from(value: F) -> Self {
        Self::Derived(Rc::new(value))
    }
}

impl<T: PartialEq + 'static> From<Inert<T>> for MaybeReactive<T> {
    fn from(value: Inert<T>) -> Self {
        Self::Inert(value)
    }
}

impl<T: PartialEq + Clone + 'static> Clone for MaybeReactive<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Inert(arg0) => Self::Inert(arg0.clone()),
            Self::Signal(arg0) => Self::Signal(arg0.clone()),
            Self::Memo(arg0) => Self::Memo(arg0.clone()),
            Self::MemoChain(arg0) => Self::MemoChain(arg0.clone()),
            Self::Derived(arg0) => Self::Derived(arg0.clone()),
        }
    }
}

macro_rules! impl_inert_into_maybe_reactive {
    ($($ty: ty),* $(,)?) => {
        $(
            impl From<$ty> for MaybeReactive<$ty> {
                fn from(value: $ty) -> Self {
                    Self::new_static(value)
                }
            }
        )*
    };
}

impl_inert_into_maybe_reactive!(
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    char,
    bool,
    f32,
    f64,
    (),
    alloc::string::String,
);

macro_rules! impl_static_into_maybe_reactive_tuple {
    () => {};

    ($first: ident, $($alphas: ident,)*) => {
        impl<$first: PartialEq, $($alphas: PartialEq,)*> From<($first, $($alphas,)*)> for MaybeReactive<($first, $($alphas,)*)> {
            fn from(value: ($first, $($alphas,)*)) -> Self {
                Self::new_static(value)
            }
        }

        impl_static_into_maybe_reactive_tuple!($($alphas,)*);
    };
}

impl_static_into_maybe_reactive_tuple!(A, B, C, D, E, F, G, H, I, J, K, L,);

impl<T: PartialEq> From<Option<T>> for MaybeReactive<Option<T>> {
    fn from(value: Option<T>) -> Self {
        Self::new_static(value)
    }
}

impl<T: PartialEq, E: PartialEq> From<Result<T, E>>
    for MaybeReactive<Result<T, E>>
{
    fn from(value: Result<T, E>) -> Self {
        Self::new_static(value)
    }
}

impl<T> From<PhantomData<T>> for MaybeReactive<PhantomData<T>> {
    fn from(value: PhantomData<T>) -> Self {
        Self::new_static(value)
    }
}

impl<T: PartialEq> From<Vec<T>> for MaybeReactive<Vec<T>> {
    fn from(value: Vec<T>) -> Self {
        Self::new_static(value)
    }
}

impl<T: PartialEq> From<&'static [T]> for MaybeReactive<&'static [T]> {
    fn from(value: &'static [T]) -> Self {
        Self::new_static(value)
    }
}

pub enum MaybeSignal<T: 'static> {
    /// Option needed to deal with conversion from [`Inert`] into [`Signal`]
    /// Optimize: Can be replaced with MaybeUninit for performance
    Inert(Option<T>),
    Signal(Signal<T>),
}

impl_read_signal_traits!(MaybeSignal<T>);

impl<T: 'static> IntoSignal<T> for MaybeSignal<T> {
    #[track_caller]
    fn signal(self) -> Signal<T> {
        match self {
            MaybeSignal::Inert(inert) => create_signal(inert.unwrap()),
            MaybeSignal::Signal(signal) => signal,
        }
    }
}

impl<T: 'static> MaybeSignal<T> {
    pub fn new_inert(value: T) -> Self {
        Self::Inert(Some(value))
    }

    #[track_caller]
    fn now_reactive(&mut self) -> Signal<T> {
        match self {
            MaybeSignal::Inert(inert) => {
                let signal = create_signal(inert.take().unwrap());
                *self = MaybeSignal::Signal(signal);
                signal
            },
            MaybeSignal::Signal(signal) => *signal,
        }
    }
}

impl<T: 'static> ReadSignal<T> for MaybeSignal<T> {
    #[track_caller]
    fn track(&self) {
        match self {
            MaybeSignal::Inert(_) => {},
            MaybeSignal::Signal(signal) => signal.track(),
        }
    }

    #[track_caller]
    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        match self {
            MaybeSignal::Inert(inert) => f(inert.as_ref().unwrap()),
            MaybeSignal::Signal(signal) => signal.with_untracked(f),
        }
    }
}

impl<T: 'static> WriteSignal<T> for MaybeSignal<T> {
    #[track_caller]
    fn notify(&self) {
        match self {
            MaybeSignal::Inert(_) => {},
            MaybeSignal::Signal(signal) => signal.notify(),
        }
    }

    #[track_caller]
    fn update_untracked<U>(&mut self, f: impl FnOnce(&mut T) -> U) -> U {
        match self {
            MaybeSignal::Inert(inert) => f(inert.as_mut().unwrap()),
            MaybeSignal::Signal(signal) => signal.update_untracked(f),
        }
    }
}

impl<T: 'static> SignalMap<T> for MaybeSignal<T> {
    type Output<U: PartialEq + 'static> = MaybeReactive<U>;

    #[track_caller]
    fn map<U: PartialEq + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Self::Output<U> {
        match self {
            MaybeSignal::Inert(inert) => {
                MaybeReactive::new_static(map(inert.as_ref().unwrap()))
            },
            MaybeSignal::Signal(signal) => MaybeReactive::Memo(signal.map(map)),
        }
    }
}

/// Here's interesting part. SignalSetter on MaybeSignal turns this MaybeSignal into Signal in case when reactive setter passed
impl<T: 'static, U: PartialEq + 'static> SignalSetter<T, MaybeReactive<U>>
    for MaybeSignal<T>
{
    #[track_caller]
    fn setter(
        &mut self,
        source: MaybeReactive<U>,
        set_map: impl Fn(&mut T, &<MaybeReactive<U> as ReactiveValue>::Value)
            + 'static,
    ) {
        match source {
            MaybeReactive::Inert(inert) => {
                // TODO: Use [`Inert`] setter
                self.update(|this| set_map(this, &inert))
            },
            MaybeReactive::Signal(signal) => {
                self.now_reactive().setter(signal, set_map)
            },
            MaybeReactive::Memo(memo) => {
                self.now_reactive().setter(memo, set_map)
            },
            MaybeReactive::MemoChain(memo_chain) => {
                self.now_reactive().setter(memo_chain, set_map)
            },
            MaybeReactive::Derived(derived) => {
                // TODO: use_effect or not to use effect? See [`Signal: SignalSetter<T, MaybeReactive<U>>`] case for Derived
                let derived = Rc::clone(&derived);
                self.update(|this| set_map(this, &derived()))
            },
        }
    }
}

// TODO: Other setters
// impl<T: PartialEq + 'static> SignalSetter<T, StaticSignal<T>>
//     for MaybeSignal<T>
// {
//     fn setter(
//         &mut self,
//         source: StaticSignal<T>,
//         set_map: impl Fn(&mut T, &<StaticSignal<T> as SignalValue>::Value) + 'static,
//     ) {
//         match self {
//             MaybeSignal::Static(raw) => {
//                 source.with(|source| set_map(&mut raw.borrow_mut(), source))
//             },
//             MaybeSignal::Signal(signal) => signal.setter(source, set_map),
//         }
//     }
// }

// impl<T: PartialEq + 'static> SignalSetter<T, Signal<T>> for MaybeSignal<T> {
//     fn setter(
//         &mut self,
//         source: Signal<T>,
//         set_map: impl Fn(&mut T, &<Signal<T> as SignalValue>::Value) + 'static,
//     ) {
//         match self {
//             MaybeSignal::Static(raw) => {
//                 source.with(|source| set_map(&mut raw.borrow_mut(), source))
//             },
//             MaybeSignal::Signal(signal) => signal.setter(source, set_map),
//         }
//     }
// }

// impl<T: PartialEq + 'static> SignalSetter<T, Memo<T>> for MaybeSignal<T> {
//     fn setter(
//         &mut self,
//         source: Memo<T>,
//         set_map: impl Fn(&mut T, &<Memo<T> as SignalValue>::Value) + 'static,
//     ) {
//         match self {
//             MaybeSignal::Static(raw) => {
//                 source.with(|source| set_map(&mut raw.borrow_mut(), source))
//             },
//             MaybeSignal::Signal(signal) => signal.setter(source, set_map),
//         }
//     }
// }

// impl<T: PartialEq + 'static> SignalSetter<T, MemoChain<T>> for MaybeSignal<T> {
//     fn setter(
//         &mut self,
//         source: MemoChain<T>,
//         set_map: impl Fn(&mut T, &<MemoChain<T> as SignalValue>::Value) + 'static,
//     ) {
//         match self {
//             MaybeSignal::Static(raw) => {
//                 source.with(|source| set_map(&mut raw.borrow_mut(), source))
//             },
//             MaybeSignal::Signal(signal) => signal.setter(source, set_map),
//         }
//     }
// }

impl<T: 'static> From<T> for MaybeSignal<T> {
    fn from(value: T) -> Self {
        Self::new_inert(value)
    }
}

impl<T: 'static> From<Signal<T>> for MaybeSignal<T> {
    fn from(value: Signal<T>) -> Self {
        Self::Signal(value)
    }
}

pub trait SignalMapReactive<T> {
    fn map_reactive<U: PartialEq + Clone + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Memo<U>;
}

impl<T: 'static> SignalMapReactive<T> for MaybeSignal<T> {
    #[track_caller]
    fn map_reactive<U: PartialEq + Clone + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Memo<U> {
        match self {
            MaybeSignal::Inert(inert) => {
                let mapped = map(&inert.as_ref().unwrap());
                create_memo(move |_| mapped.clone())
            },
            MaybeSignal::Signal(signal) => signal.map(map),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        prelude::*, read::ReadSignal as _, runtime::new_deny_new_scope,
    };

    #[test]
    fn static_wrapper() {
        let _deny_new_reactive = new_deny_new_scope();

        let s = 123.inert();
        s.get();
        s.get_cloned();
        s.get_untracked();
        s.map(|s| *s);
    }

    // Type-check tests
    #[test]
    fn conversions() {
        fn accept_maybe_reactive<T: PartialEq + 'static>(
            mr: impl Into<MaybeReactive<T>>,
        ) {
            // Assert no reactive value created on conversion
            let _deny_new_reactive = new_deny_new_scope();

            let _ = mr.into();
        }

        // Inert<()>
        // Inert values need explicit conversion into Inert wrapper
        accept_maybe_reactive(().inert());
        // Derived signal
        accept_maybe_reactive(|| {});
        // Memo<()>
        accept_maybe_reactive(create_memo(move |_| {}));
        // Signal<()>
        accept_maybe_reactive(create_signal(()));
        // MemoChain<()>
        accept_maybe_reactive(create_memo_chain(move |_| {}));
    }

    #[test]
    fn maybe_signal_mapper() {
        let mut maybe = MaybeSignal::new_inert(123);

        // Warning: Non-reactive map
        let map = maybe.map(|value| *value);

        assert_eq!(map.get(), 123);

        maybe.set(234);

        // Map is not reactive
        assert_eq!(map.get(), 123);
        assert!(matches!(map, MaybeReactive::Inert(Inert { value: 123 })));
    }

    #[test]
    fn maybe_signal_setter_from_reactive() {
        let mut maybe = MaybeSignal::new_inert(123);

        assert_eq!(maybe.get(), 123);

        let reactive = create_signal(69);

        maybe.set_from(reactive.into());

        // Setter turns MaybeSignal into reactive
        assert_eq!(maybe.get(), 69);
        assert!(matches!(maybe, MaybeSignal::Signal(_)));
    }
}