use crate::{
    ReactiveValue,
    inert::Inert,
    memo::{IntoMemo, Memo},
    read::{ReadSignal, SignalMap, impl_read_signal_traits},
    signal::Signal,
};
use alloc::vec::Vec;
use core::marker::PhantomData;

// TODO: Can we hack reactive values so PartialEq won't be required? One way is
// to make ValueId generic over reactive value type, for example
// `as_memo_with_untracked` and `as_signal_with_untracked` implementations that
// will be dispatched based on the MaybeReactive variant.
/// An optionally reactive, **read-only** value.
///
/// Unifies two read sources under one type:
/// - [`MaybeReactive::Inert`] — a static value; reads are never tracked and do
///   not register the caller as a subscriber.
/// - [`MaybeReactive::Memo`] — a derived reactive value; reads inside a
///   reactive context register a dependency.
///
/// # Common pattern
///
/// Declare a function or widget field as `impl IntoMaybeReactive<T>`;
/// callers can pass either a plain constant or a live reactive value without
/// changing the field type:
///
/// ```rust
/// # use rsact_reactive::prelude::*;
/// # use rsact_reactive::maybe::{IntoInert, IntoMaybeReactive, MaybeReactive};
/// fn slider(range: impl IntoMaybeReactive<core::ops::RangeInclusive<f32>>) {
///     let range: MaybeReactive<core::ops::RangeInclusive<f32>> =
///         range.maybe_reactive();
///     // Static caller:   slider(0.0..=100.0)
///     // Reactive caller: slider(range_signal.maybe_reactive())
/// }
/// ```
///
/// # Mapping
///
/// [`SignalMap::map`] preserves reactivity: an [`Inert`] source produces
/// another [`Inert`] output (no allocation); a [`Memo`] source produces a new
/// [`Memo`] whose closure re-evaluates whenever the source changes.
///
/// For a **read-write** optionally reactive value see [`MaybeSignal`].
pub enum MaybeReactive<T: PartialEq + 'static> {
    Inert(Inert<T>),
    Memo(Memo<T>),
    // Derived(Rc<RefCell<dyn FnMut() -> T>>),
}

impl_read_signal_traits!(MaybeReactive<T>: PartialEq);

// WS4.1: `Inert` now stores `T` inline, so `MaybeReactive<T>` is `Clone` iff
// `T: Clone` and `Copy` iff `T: Copy` (G1 — blanket `Copy` dropped). `Memo<T>`
// remains a `Copy` handle for all `T`.
impl<T: Clone + PartialEq + 'static> Clone for MaybeReactive<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Inert(arg0) => Self::Inert(arg0.clone()),
            Self::Memo(arg0) => Self::Memo(*arg0),
            // Self::Derived(arg0) => Self::Derived(arg0.clone()),
        }
    }
}
impl<T: Copy + PartialEq + 'static> Copy for MaybeReactive<T> {}

impl<T: PartialEq + 'static> MaybeReactive<T> {
    pub fn new_inert(value: T) -> Self {
        Self::Inert(Inert::from(value))
    }

    // pub fn new_derived(f: impl FnMut() -> T + 'static) -> Self {
    //     Self::Derived(Rc::new(RefCell::new(f)))
    // }
}

impl<T: PartialEq + 'static> ReactiveValue for MaybeReactive<T> {
    type Value = T;

    fn id(&self) -> Option<crate::storage::ValueId> {
        match self {
            MaybeReactive::Inert(inert) => inert.id(),
            MaybeReactive::Memo(memo) => memo.id(),
        }
    }

    #[track_caller]
    fn is_alive(&self) -> bool {
        match self {
            MaybeReactive::Inert(inert) => inert.is_alive(),
            MaybeReactive::Memo(memo) => memo.is_alive(),
            // MaybeReactive::Derived(_) => true,
        }
    }

    #[track_caller]
    unsafe fn dispose(self) {
        match self {
            MaybeReactive::Inert(inert) => unsafe { inert.dispose() },
            MaybeReactive::Memo(memo) => unsafe { memo.dispose() },
            // MaybeReactive::Derived(derived) => core::mem::drop(derived),
        }
    }
}

impl<T: PartialEq + 'static> ReadSignal<T> for MaybeReactive<T> {
    #[track_caller]
    fn track(&self) {
        match self {
            MaybeReactive::Inert(_) => {},
            MaybeReactive::Memo(memo) => memo.track(),
            // MaybeReactive::Derived(_) => {},
        }
    }

    #[track_caller]
    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        match self {
            MaybeReactive::Inert(inert) => inert.with_untracked(f),
            MaybeReactive::Memo(memo) => memo.with_untracked(f),
            // MaybeReactive::Derived(derived) => f(&derived.borrow_mut()()),
        }
    }
}

impl<T: PartialEq + 'static, U: PartialEq + 'static> SignalMap<T, U>
    for MaybeReactive<T>
{
    type Output = MaybeReactive<U>;

    #[track_caller]
    fn map(&self, map: impl FnMut(&T) -> U + 'static) -> Self::Output {
        match self {
            MaybeReactive::Inert(inert) => {
                // FIXME: TODO
                // let inert = inert.clone();
                // MaybeReactive::Derived(Rc::new(RefCell::new(move || {
                //     map(&inert)
                // })))
                inert.map(map).maybe_reactive()
            },
            MaybeReactive::Memo(memo) => MaybeReactive::Memo(memo.map(map)),
            // MaybeReactive::Derived(derived) => {
            //     let derived = Rc::clone(derived);
            //     MaybeReactive::new_derived(move ||
            // map(&derived.borrow_mut()())) },
        }
    }
}

/// Conversion into [`MaybeReactive<T>`].
///
/// Implemented for:
/// - [`MaybeReactive<T>`] — identity.
/// - [`Signal<T>`] — wraps in a thin [`Memo`] (zero-overhead delegation).
/// - [`Memo<T>`] — identity.
/// - [`Inert<T>`] — wraps as [`MaybeReactive::Inert`].
/// - Primitive types (`u8`–`u128`, `i8`–`i128`, `f32`, `f64`, `bool`, `char`,
///   `()`, `String`, tuples up to 12 elements, `Option<T>`, `Result<T,E>`,
///   `Vec<T>`, `&'static [T]`) — automatically wrapped as inert.
///
/// A derive macro is available for user-defined copy types in rsact-macros:
///
/// ```rust,ignore
/// #[derive(Clone, Copy, Debug, PartialEq, IntoMaybeReactive)]
/// pub enum FontSize { Small, Medium, Large }
/// ```
///
/// Call `.maybe_reactive()` to perform the conversion.
pub trait IntoMaybeReactive<T: PartialEq> {
    fn maybe_reactive(self) -> MaybeReactive<T>;
}

impl<T: PartialEq + 'static> IntoMaybeReactive<T> for MaybeReactive<T> {
    fn maybe_reactive(self) -> MaybeReactive<T> {
        self
    }
}

impl<T: PartialEq + 'static> IntoMaybeReactive<T> for Signal<T> {
    fn maybe_reactive(self) -> MaybeReactive<T> {
        MaybeReactive::Memo(self.memo())
    }
}

impl<T: PartialEq + 'static> IntoMaybeReactive<T> for Memo<T> {
    fn maybe_reactive(self) -> MaybeReactive<T> {
        MaybeReactive::Memo(self)
    }
}

// impl<T: PartialEq + 'static, F: FnMut() -> T + 'static> IntoMaybeReactive<T>
//     for F
// {
//     fn maybe_reactive(self) -> MaybeReactive<T> {
//         MaybeReactive::new_derived(self)
//     }
// }

impl<T: PartialEq + 'static> IntoMaybeReactive<T> for Inert<T> {
    fn maybe_reactive(self) -> MaybeReactive<T> {
        MaybeReactive::Inert(self)
    }
}

macro_rules! impl_inert_into_maybe_reactive {
    ($($ty: ty),* $(,)?) => {
        $(
            impl IntoMaybeReactive<$ty> for $ty {
                fn maybe_reactive(self) -> MaybeReactive<$ty> {
                    MaybeReactive::new_inert(self)
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
        impl<$first: PartialEq, $($alphas: PartialEq,)*> IntoMaybeReactive<($first, $($alphas,)*)> for ($first, $($alphas,)*)  {
            fn maybe_reactive(self) -> MaybeReactive<($first, $($alphas,)*)> {
                MaybeReactive::new_inert(self)
            }
        }

        impl_static_into_maybe_reactive_tuple!($($alphas,)*);
    };
}

impl_static_into_maybe_reactive_tuple!(A, B, C, D, E, F, G, H, I, J, K, L,);

impl<T: PartialEq> IntoMaybeReactive<Option<T>> for Option<T> {
    fn maybe_reactive(self) -> MaybeReactive<Option<T>> {
        MaybeReactive::new_inert(self)
    }
}

impl<T: PartialEq, E: PartialEq> IntoMaybeReactive<Result<T, E>>
    for Result<T, E>
{
    fn maybe_reactive(self) -> MaybeReactive<Result<T, E>> {
        MaybeReactive::new_inert(self)
    }
}

impl<T> IntoMaybeReactive<PhantomData<T>> for PhantomData<T> {
    fn maybe_reactive(self) -> MaybeReactive<PhantomData<T>> {
        MaybeReactive::new_inert(self)
    }
}

impl<T: PartialEq> IntoMaybeReactive<Vec<T>> for Vec<T> {
    fn maybe_reactive(self) -> MaybeReactive<Vec<T>> {
        MaybeReactive::new_inert(self)
    }
}

impl<T: PartialEq> IntoMaybeReactive<&'static [T]> for &'static [T] {
    fn maybe_reactive(self) -> MaybeReactive<&'static [T]> {
        MaybeReactive::new_inert(self)
    }
}

impl<T: PartialEq + Clone> IntoMemo<T> for MaybeReactive<T> {
    fn memo(self) -> Memo<T> {
        // TODO: Check this
        match self {
            MaybeReactive::Inert(inert) => inert.memo(),
            MaybeReactive::Memo(memo) => memo,
            // MaybeReactive::Derived(derived) => {
            //     let derived = Rc::clone(&derived);
            //     create_memo(move || derived.borrow_mut()())
            // },
        }
    }
}

pub trait SignalMapMaybeReactive<T, U: PartialEq + 'static> {
    fn map_maybe_reactive(
        &self,
        map: impl FnMut(&T) -> U + 'static,
    ) -> MaybeReactive<U>;
}

impl<T: 'static, U: PartialEq + 'static, S: SignalMap<T, U>>
    SignalMapMaybeReactive<T, U> for S
where
    S::Output: IntoMaybeReactive<U>,
{
    fn map_maybe_reactive(
        &self,
        map: impl FnMut(&T) -> U + 'static,
    ) -> MaybeReactive<U> {
        self.map(map).maybe_reactive()
    }
}
