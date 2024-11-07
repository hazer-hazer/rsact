use crate::{
    memo::Memo,
    prelude::MemoChain,
    signal::{ReadSignal, Signal},
};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::marker::PhantomData;

pub enum MaybeReactive<T: PartialEq> {
    Static(T),
    Signal(Signal<T>),
    Memo(Memo<T>),
    MemoChain(MemoChain<T>),
    Derived(Box<dyn Fn() -> T>),
}

impl<T: PartialEq + 'static> From<Signal<T>> for MaybeReactive<T> {
    fn from(value: Signal<T>) -> Self {
        Self::Signal(value)
    }
}

impl<T: PartialEq + 'static> ReadSignal<T> for MaybeReactive<T> {
    fn track(&self) {
        match self {
            MaybeReactive::Static(_) => {},
            MaybeReactive::Signal(signal) => signal.track(),
            MaybeReactive::Memo(memo) => memo.track(),
            MaybeReactive::MemoChain(memo_chain) => memo_chain.track(),
            MaybeReactive::Derived(_) => {},
        }
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        match self {
            MaybeReactive::Static(raw) => f(raw),
            MaybeReactive::Signal(signal) => signal.with_untracked(f),
            MaybeReactive::Memo(memo) => memo.with_untracked(f),
            MaybeReactive::MemoChain(memo_chain) => {
                memo_chain.with_untracked(f)
            },
            MaybeReactive::Derived(derived) => f(&derived()),
        }
    }
}

macro_rules! impl_static_into_maybe_reactive {
    ($($ty: ty),* $(,)?) => {
        $(
            impl From<$ty> for MaybeReactive<$ty> {
                fn from(value: $ty) -> Self {
                    Self::Static(value)
                }
            }
        )*
    };
}

impl_static_into_maybe_reactive!(
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
    String,
);

macro_rules! impl_static_into_maybe_reactive_tuple {
    () => {};

    ($first: ident, $($alphas: ident,)*) => {
        impl<$first: PartialEq, $($alphas: PartialEq,)*> From<($first, $($alphas,)*)> for MaybeReactive<($first, $($alphas,)*)> {
            fn from(value: ($first, $($alphas,)*)) -> Self {
                Self::Static(value)
            }
        }

        impl_static_into_maybe_reactive_tuple!($($alphas,)*);
    };
}

impl_static_into_maybe_reactive_tuple!(A, B, C, D, E, F, G, H, I, J, K, L,);

impl<T: PartialEq> From<Option<T>> for MaybeReactive<Option<T>> {
    fn from(value: Option<T>) -> Self {
        Self::Static(value)
    }
}

impl<T: PartialEq, E: PartialEq> From<Result<T, E>>
    for MaybeReactive<Result<T, E>>
{
    fn from(value: Result<T, E>) -> Self {
        Self::Static(value)
    }
}

impl<T> From<PhantomData<T>> for MaybeReactive<PhantomData<T>> {
    fn from(value: PhantomData<T>) -> Self {
        Self::Static(value)
    }
}

impl<T: PartialEq> From<Vec<T>> for MaybeReactive<Vec<T>> {
    fn from(value: Vec<T>) -> Self {
        Self::Static(value)
    }
}

impl<T: PartialEq> From<Box<T>> for MaybeReactive<Box<T>> {
    fn from(value: Box<T>) -> Self {
        Self::Static(value)
    }
}

impl<T: PartialEq + ?Sized> From<&'static T> for MaybeReactive<&'static T> {
    fn from(value: &'static T) -> Self {
        Self::Static(value)
    }
}
