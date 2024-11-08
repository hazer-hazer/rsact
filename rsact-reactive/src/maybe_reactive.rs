use crate::{
    memo::Memo,
    prelude::MemoChain,
    signal::{
        ReadSignal, Signal, SignalMapper, SignalSetter, SignalValue,
        WriteSignal,
    },
    with,
};
use alloc::rc::Rc;
use core::{
    cell::RefCell,
    fmt::{Debug, Display},
    ops::{Deref, DerefMut},
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct StaticSignal<T: 'static> {
    value: T,
}

impl<T: 'static> SignalValue for StaticSignal<T> {
    type Value = T;
}

impl<T> StaticSignal<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T> From<T> for StaticSignal<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> ReadSignal<T> for StaticSignal<T> {
    fn track(&self) {
        // Static signal never changes thus scope does not need to subscribe to
        // its changes
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        f(&self.value)
    }
}

impl<T: PartialEq + 'static> SignalMapper<T> for StaticSignal<T> {
    type Output<U: PartialEq + 'static> = StaticSignal<U>;

    fn mapped<U: PartialEq + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Self::Output<U> {
        map(&self.value).into_static_signal()
    }
}

// impl<T: PartialEq + 'static> SignalSetter<T, Self> for StaticSignal<T> {
//     fn setter(
//         &mut self,
//         source: Self,
//         set_map: impl Fn(&mut T, &<Self as SignalValue>::Value) + 'static,
//     ) {
//         set_map(&mut self.value, &source.value)
//     }
// }

impl<T> Deref for StaticSignal<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for StaticSignal<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

pub trait IntoStaticSignal<T> {
    fn into_static_signal(self) -> StaticSignal<T>;
}

impl<T> IntoStaticSignal<T> for T {
    fn into_static_signal(self) -> StaticSignal<T> {
        StaticSignal::from(self)
    }
}

/// Maybe reactive read-only value, i.e. anything from static values to writable signals.
/// For RW version of [`MaybeReactive`] see [`MaybeSignal`]
pub enum MaybeReactive<T: PartialEq + 'static> {
    Static(StaticSignal<T>),
    Signal(Signal<T>),
    Memo(Memo<T>),
    MemoChain(MemoChain<T>),
    Derived(Rc<dyn Fn() -> T>),
}

impl<T: PartialEq + 'static> SignalValue for MaybeReactive<T> {
    type Value = T;
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

impl<T: PartialEq + Clone + 'static> SignalMapper<T> for MaybeReactive<T> {
    type Output<U: PartialEq + 'static> = MaybeReactive<U>;

    fn mapped<U: PartialEq + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Self::Output<U> {
        match self {
            MaybeReactive::Static(raw) => {
                let raw = raw.clone();
                MaybeReactive::Derived(Rc::new(move || map(&raw)))
            },
            MaybeReactive::Signal(signal) => {
                MaybeReactive::Memo(signal.mapped(map))
            },
            MaybeReactive::Memo(memo) => MaybeReactive::Memo(memo.mapped(map)),
            MaybeReactive::MemoChain(memo_chain) => {
                MaybeReactive::Memo(memo_chain.mapped(map))
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

impl<T: PartialEq + 'static> From<StaticSignal<T>> for MaybeReactive<T> {
    fn from(value: StaticSignal<T>) -> Self {
        Self::Static(value)
    }
}

// TODO: Move to global implementation on ReadSignal
impl<T: PartialEq + 'static> PartialEq for MaybeReactive<T> {
    fn eq(&self, other: &Self) -> bool {
        let this = self;
        with!(|this, other| { this.eq(other) })
    }
}

// TODO: Move to global implementation on ReadSignal
impl<T: PartialEq + Display + 'static> Display for MaybeReactive<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.with(|inner| inner.fmt(f))
    }
}

// TODO: Move to global implementation on ReadSignal
impl<T: PartialEq + Debug + 'static> Debug for MaybeReactive<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.with(|inner| inner.fmt(f))
    }
}

impl<T: PartialEq + Clone + 'static> Clone for MaybeReactive<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Static(arg0) => Self::Static(arg0.clone()),
            Self::Signal(arg0) => Self::Signal(arg0.clone()),
            Self::Memo(arg0) => Self::Memo(arg0.clone()),
            Self::MemoChain(arg0) => Self::MemoChain(arg0.clone()),
            Self::Derived(arg0) => Self::Derived(arg0.clone()),
        }
    }
}

// pub trait IntoMaybeReactive<T: PartialEq>: 'static {
//     fn into_maybe_reactive(self) -> MaybeReactive<T>;
// }

// impl<T: PartialEq + 'static> IntoMaybeReactive<T> for Signal<T> {
//     fn into_maybe_reactive(self) -> MaybeReactive<T> {
//         MaybeReactive::Signal(self)
//     }
// }

// impl<T: PartialEq + 'static> IntoMaybeReactive<T> for Memo<T> {
//     fn into_maybe_reactive(self) -> MaybeReactive<T> {
//         MaybeReactive::Memo(self)
//     }
// }

// impl<T: PartialEq + 'static> IntoMaybeReactive<T> for MemoChain<T> {
//     fn into_maybe_reactive(self) -> MaybeReactive<T> {
//         MaybeReactive::MemoChain(self)
//     }
// }

// // TODO: Add some `Derived` wrapper around function so user can use `.into_derived().into_maybe_reactive()`
// // impl<T: PartialEq + 'static> IntoMaybeReactive<T> for fn() -> T {
// //     fn into_maybe_reactive(self) -> MaybeReactive<T> {
// //         MaybeReactive::Derived(Rc::new(self))
// //     }
// // }

// impl<T: PartialEq + 'static> IntoMaybeReactive<T> for T {
//     fn into_maybe_reactive(self) -> MaybeReactive<T> {
//         MaybeReactive::Static(self)
//     }
// }

// fn foo(maybe_reactive: impl IntoMaybeReactive<String>) {}

// fn bar() {
//     foo(String::new());
//     foo(create_signal(String::new()));
// }

// macro_rules! impl_static_into_maybe_reactive {
//     ($($ty: ty),* $(,)?) => {
//         $(
//             impl From<$ty> for MaybeReactive<$ty> {
//                 fn from(value: $ty) -> Self {
//                     Self::Static(value)
//                 }
//             }
//         )*
//     };
// }

// impl_static_into_maybe_reactive!(
//     i8,
//     i16,
//     i32,
//     i64,
//     i128,
//     isize,
//     u8,
//     u16,
//     u32,
//     u64,
//     u128,
//     usize,
//     char,
//     bool,
//     f32,
//     f64,
//     (),
//     String,
// );

// macro_rules! impl_static_into_maybe_reactive_tuple {
//     () => {};

//     ($first: ident, $($alphas: ident,)*) => {
//         impl<$first: PartialEq, $($alphas: PartialEq,)*> From<($first, $($alphas,)*)> for MaybeReactive<($first, $($alphas,)*)> {
//             fn from(value: ($first, $($alphas,)*)) -> Self {
//                 Self::Static(value)
//             }
//         }

//         impl_static_into_maybe_reactive_tuple!($($alphas,)*);
//     };
// }

// impl_static_into_maybe_reactive_tuple!(A, B, C, D, E, F, G, H, I, J, K, L,);

// impl<T: PartialEq> From<Option<T>> for MaybeReactive<Option<T>> {
//     fn from(value: Option<T>) -> Self {
//         Self::Static(value)
//     }
// }

// impl<T: PartialEq, E: PartialEq> From<Result<T, E>>
//     for MaybeReactive<Result<T, E>>
// {
//     fn from(value: Result<T, E>) -> Self {
//         Self::Static(value)
//     }
// }

// impl<T> From<PhantomData<T>> for MaybeReactive<PhantomData<T>> {
//     fn from(value: PhantomData<T>) -> Self {
//         Self::Static(value)
//     }
// }

// impl<T: PartialEq> From<Vec<T>> for MaybeReactive<Vec<T>> {
//     fn from(value: Vec<T>) -> Self {
//         Self::Static(value)
//     }
// }

// impl<T: PartialEq> From<Box<T>> for MaybeReactive<Box<T>> {
//     fn from(value: Box<T>) -> Self {
//         Self::Static(value)
//     }
// }

// impl<T: PartialEq + ?Sized> From<&'static T> for MaybeReactive<&'static T> {
//     fn from(value: &'static T) -> Self {
//         Self::Static(value)
//     }
// }

pub enum MaybeSignal<T: 'static> {
    Static(RefCell<T>),
    Signal(Signal<T>),
}

impl<T: 'static> MaybeSignal<T> {
    pub fn non_reactive(value: T) -> Self {
        Self::Static(RefCell::new(value))
    }
}

impl<T: 'static> ReadSignal<T> for MaybeSignal<T> {
    fn track(&self) {
        match self {
            MaybeSignal::Static(_) => {},
            MaybeSignal::Signal(signal) => signal.track(),
        }
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        match self {
            MaybeSignal::Static(raw) => f(&raw.borrow()),
            MaybeSignal::Signal(signal) => signal.with_untracked(f),
        }
    }
}

impl<T: 'static> WriteSignal<T> for MaybeSignal<T> {
    fn notify(&self) {
        match self {
            MaybeSignal::Static(_) => {},
            MaybeSignal::Signal(signal) => signal.notify(),
        }
    }

    fn update_untracked<U>(&mut self, f: impl FnOnce(&mut T) -> U) -> U {
        match self {
            MaybeSignal::Static(raw) => f(&mut raw.borrow_mut()),
            MaybeSignal::Signal(signal) => signal.update_untracked(f),
        }
    }
}

// impl<T: PartialEq + 'static> SignalSetter<T, MaybeReactive<T>>
//     for MaybeSignal<T>
// {
//     fn setter(
//         &mut self,
//         source: MaybeReactive<T>,
//         set_map: impl Fn(&mut T, &<MaybeReactive<T> as SignalValue>::Value)
//             + 'static,
//     ) {
//         match source {
//             MaybeReactive::Static(static_signal) => {
//                 self.update(|this| set_map(this, &static_signal))
//             },
//             MaybeReactive::Signal(signal) => self.setter(signal, set_map),
//             MaybeReactive::Memo(memo) => todo!(),
//             MaybeReactive::MemoChain(memo_chain) => todo!(),
//             MaybeReactive::Derived(rc) => todo!(),
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
//             MaybeSignal::Static(ref_cell) => ,
//             MaybeSignal::Signal(signal) => todo!(),
//         }
//     }
// }

impl<T: 'static> From<T> for MaybeSignal<T> {
    fn from(value: T) -> Self {
        Self::non_reactive(value)
    }
}

impl<T: 'static> From<Signal<T>> for MaybeSignal<T> {
    fn from(value: Signal<T>) -> Self {
        Self::Signal(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        prelude::{create_memo, create_signal, use_memo_chain, MaybeReactive},
        runtime::new_deny_new_scope,
        signal::{ReadSignal, SignalMapper},
    };

    use super::IntoStaticSignal;

    #[test]
    fn static_wrapper() {
        let _deny_new_reactive = new_deny_new_scope();

        let s = 123.into_static_signal();
        s.get();
        s.get_cloned();
        s.get_untracked();
        s.mapped(|s| *s);
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

        // StaticSignal<()>
        // Static values need explicit conversion to StaticSignal wrapper
        accept_maybe_reactive(().into_static_signal());
        // Derived signal
        accept_maybe_reactive(|| {});
        // Memo<()>
        accept_maybe_reactive(create_memo(move |_| {}));
        // Signal<()>
        accept_maybe_reactive(create_signal(()));
        // MemoChain<()>
        accept_maybe_reactive(use_memo_chain(move |_| {}));
    }
}
