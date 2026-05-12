use crate::{
    ReactiveValue,
    maybe::{IntoMaybeReactive, MaybeReactive},
};

/// Read access to a reactive value.
///
/// The two required methods are [`ReadSignal::track`] and
/// [`ReadSignal::with_untracked`]. Everything else — `get`, `get_cloned`,
/// `with`, etc. — is provided by default implementations.
///
/// Calling `with` (or `get`, `get_cloned`) inside a reactive context
/// (an effect or memo closure) registers the caller as a subscriber of this
/// value.  Calling the `_untracked` variants skips registration.
///
/// Implemented by: [`crate::signal::Signal`], [`crate::memo::Memo`], [`crate::memo_chain::MemoChain`], [`crate::computed::Computed`],
/// [`crate::trigger::Trigger`], [`crate::maybe::Inert`], [`crate::maybe::MaybeReactive`], [`crate::maybe::MaybeSignal`].
pub trait ReadSignal<T: 'static>: ReactiveValue {
    fn track(&self);
    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U;

    #[track_caller]
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.track();
        self.with_untracked(f)
    }

    #[track_caller]
    fn get(&self) -> T
    where
        T: Copy,
    {
        self.with(|value| *value)
    }

    #[track_caller]
    fn get_untracked(&self) -> T
    where
        T: Copy,
    {
        self.with_untracked(|value| *value)
    }

    #[track_caller]
    fn get_cloned(&self) -> T
    where
        T: Clone,
    {
        self.with(T::clone)
    }
}

/// Transform the value inside a reactive type into a new reactive output.
///
/// The output type is determined by the implementor:
/// - `Signal<T>::map` → `Memo<U>` (tracked, re-evaluates when the signal changes).
/// - `Memo<T>::map` → `Memo<U>` (chained memo).
/// - `MemoChain<T>::map` → `Memo<U>`.
/// - `MaybeSignal<T>::map` → `MaybeReactive<U>` (preserves inert/reactive distinction).
/// - `Inert<T>::map` → `Inert<U>` (pure, non-allocating).
///
/// See also [`crate::maybe::SignalMapReactive`] when you always need a `Memo<U>`
/// regardless of source reactivity.
pub trait SignalMap<T: 'static> {
    type Output<U: PartialEq + 'static>;

    fn map<U: PartialEq + 'static>(
        &self,
        map: impl FnMut(&T) -> U + 'static,
    ) -> Self::Output<U>;

    // TODO: Is this needed?
    #[track_caller]
    fn map_cloned<U: PartialEq + 'static>(
        &self,
        mut map: impl FnMut(T) -> U + 'static,
    ) -> Self::Output<U>
    where
        Self: Sized + 'static,
        T: Clone,
    {
        self.map(move |this| map(this.clone()))
    }
}

// /// Used to access deep signal values, such as `Memo<Memo<Memo<T>>>`. Be careful with this, only use if `signal.with(|signal| signal.with(|signal| signal.with(f)))` is the behavior you need.
// pub trait WithDeep<T: 'static>: ReadSignal<T> {
//     #[track_caller]
//     fn deep_with<U>(&self, f: impl FnOnce(&T) -> U) -> U;
// }

// TODO: Implement SignalMap for tuple of signals or map! macro is enough?
// macro_rules! impl_signal_map_tuple {
//     () => {};

//     ($first: ident, $($alphas: ident,)*) => {
//         impl<$first, $($alphas,)*> SignalMap<($first, $($alphas,)*)> for ($first, $($alphas,)*)  {
//             fn map(self) -> MaybeReactive<($first, $($alphas,)*)> {
//                 MaybeReactive::new_inert(self)
//             }
//         }

//         impl_static_into_maybe_reactive_tuple!($($alphas,)*);
//     };
// }

// impl<T: 'static> SignalMap<(T,)> for (crate::signal::Signal<T>,) {
//     type Output<U: PartialEq + 'static> = Memo<U>;

//     fn map<U: PartialEq + 'static>(
//         &self,
//         map: impl FnMut(&(T,)) -> U + 'static,
//     ) -> Self::Output<U> {
//         let (T,) = *self;
//         create_memo(move || with!(move |T,| ))
//     }
// }

#[macro_export]
macro_rules! with {
    (|$param: ident $(,)?| $body: expr) => {
        $param.with(|$param| $body)
    };

    (|$param: ident, $($rest: ident),+ $(,)?| $body: expr) => {
        $param.with(|$param| $crate::read::with!(|$($rest),+| $body))
    };

    (move |$param: ident $(,)?| $body: expr) => {
        $param.with(move |$param| $body)
    };

    (move |$param: ident, $($rest: ident),+ $(,)?| $body: expr) => {
        $param.with(move |$param| $crate::read::with!(move |$($rest),+| $body))
    };
}

pub use with;

// Note: with! macro call inside is intentional to avoid creation of many memos
#[macro_export]
macro_rules! map {
    (|$param: ident $(,)?| $body: expr) => {
        $param.map(|$param| $body)
    };

    (|$param: ident, $($rest: ident),+ $(,)?| $body: expr) => {
        $param.map(|$param| $crate::read::with!(|$($rest),+| $body))
    };

    (move |$param: ident $(,)?| $body: expr) => {
        $param.map(move |$param| $body)
    };

    (move |$param: ident, $($rest: ident),+ $(,)?| $body: expr) => {
        $param.map(move |$param| $crate::read::with!(move |$($rest),+| $body))
    };
}

pub use map;

/// All ReadSignal structs implement common operations and core traits
/// Macro is used because we cannot implement core traits for all types "S: ReadSignal"
/// All operations on signals are not reactive. I thought it would be nice to have `signal + signal` resulting in memo but, firstly, it is inconsistent with PartialEq and PartialOrd which cannot result in memo as they don't have custom Output type, so they cannot be made reactive, secondly, I want to help user to create as least reactive values as possible, so almost all places where new reactive values is created should be explicit. Just use `create_memo`.
// All ReadSignal's always receive single generic determining inner value, so no need to deal with generics in macro parameter. But the Signal and MaybeSignal are special cases because of `M` marker. Just implementing traits for both RW and R signals separately, this is much easier to pass generics to macro.
macro_rules! impl_read_signal_traits {
    ($($ty: ty $(: $($generics: tt),*)?),* $(,)?) => {
        $(
            impl<T: PartialEq + 'static + $($($generics+)*)?> PartialEq for $ty {
                fn eq(&self, other: &Self) -> bool {
                    let this = self;
                    crate::read::with!(|this, other| { this.eq(other) })
                }
            }

            impl<T: core::fmt::Display + 'static + $($($generics+)*)?> core::fmt::Display for $ty {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    self.with(|inner| inner.fmt(f))
                }
            }

            impl<T: core::fmt::Debug + 'static + $($($generics+)*)?> core::fmt::Debug for $ty {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    self.with(|inner| inner.fmt(f))
                }
            }

            impl<T> core::ops::Neg for $ty
            where
                T: core::ops::Neg<Output = T> + PartialEq + Clone + 'static,
            {
                // type Output = <$ty as SignalMapper<T>>::Output<T>;
                type Output = T;

                #[track_caller]
                fn neg(self) -> Self::Output {
                    self.with(|this| this.clone().neg())
                }
            }

            impl<T> core::ops::Not for $ty
            where
                T: core::ops::Not<Output = T> + PartialEq + Clone + 'static,
            {
                // type Output = <$ty as SignalMapper<T>>::Output<T>;
                type Output = T;

                #[track_caller]
                fn not(self) -> Self::Output {
                    self.with(|this| this.clone().not())
                }
            }

            impl<T> PartialOrd for $ty
            where
                T: PartialEq + PartialOrd + Clone + 'static,
            {
                #[track_caller]
                fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                    self.with(|this| other.with(|other| this.partial_cmp(other)))
                }
            }

            $crate::read::impl_read_signal_traits!(@op $ty $(: $($generics),*)?; Add: add);
            $crate::read::impl_read_signal_traits!(@op $ty $(: $($generics),*)?; Sub: sub);
            $crate::read::impl_read_signal_traits!(@op $ty $(: $($generics),*)?; Mul: mul);
            $crate::read::impl_read_signal_traits!(@op $ty $(: $($generics),*)?; Div: div);
            $crate::read::impl_read_signal_traits!(@op $ty $(: $($generics),*)?; Rem: rem);
            $crate::read::impl_read_signal_traits!(@op $ty $(: $($generics),*)?; BitAnd: bitand);
            $crate::read::impl_read_signal_traits!(@op $ty $(: $($generics),*)?; BitOr: bitor);
            $crate::read::impl_read_signal_traits!(@op $ty $(: $($generics),*)?; BitXor: bitxor);
            $crate::read::impl_read_signal_traits!(@op $ty $(: $($generics),*)?; Shl: shl);
            $crate::read::impl_read_signal_traits!(@op $ty $(: $($generics),*)?; Shr: shr);
        )*
    };

    (@op $ty: ty $(: $($generics: tt),*)?; $trait: ident: $method: ident) => {
        impl<T> core::ops::$trait for $ty
        where
            T: core::ops::$trait<Output = T> + PartialEq + Clone + 'static + $($($generics+)*)?,
        {
            type Output = T;

            #[track_caller]
            fn $method(self, rhs: Self) -> Self::Output {
                self.with(|lhs| rhs.with(|rhs| lhs.clone().$method(rhs.clone())))
            }
        }

        impl<T> core::ops::$trait<T> for $ty
        where
            T: core::ops::$trait<Output = T> + Clone + 'static + $($($generics+)*)?,
        {
            type Output = T;

            #[track_caller]
            fn $method(self, rhs: T) -> Self::Output {
                self.with(|lhs| lhs.clone().$method(rhs))
            }
        }
    };
}

pub(crate) use impl_read_signal_traits;

pub trait WithRef<T: ?Sized> {
    fn with_ref<U>(&self, f: impl FnOnce(&T) -> U) -> U;
}

impl<T: ?Sized, C, R> WithRef<T> for R
where
    C: AsRef<T> + 'static,
    R: ReactiveValue<Value = C> + ReadSignal<C>,
{
    fn with_ref<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.with(|c| f(c.as_ref()))
    }
}

pub trait SignalWithSlice<T> {
    fn with_slice<U>(&self, f: impl FnOnce(&[T]) -> U) -> U;
}

impl<T, R> SignalWithSlice<T> for R
where
    R: WithRef<[T]>,
{
    fn with_slice<U>(&self, f: impl FnOnce(&[T]) -> U) -> U {
        self.with_ref(f)
    }
}

pub trait SignalMapRef<T: ?Sized> {
    type Output<U: PartialEq + 'static>;

    fn map_ref<U: PartialEq + 'static>(
        &self,
        map: impl FnMut(&T) -> U + 'static,
    ) -> Self::Output<U>;
}

impl<T: ?Sized, C, R> SignalMapRef<T> for R
where
    C: AsRef<T> + 'static,
    R: ReactiveValue<Value = C> + SignalMap<C>,
{
    type Output<U: PartialEq + 'static> = <Self as SignalMap<C>>::Output<U>;

    fn map_ref<U: PartialEq + 'static>(
        &self,
        mut map: impl FnMut(&T) -> U + 'static,
    ) -> Self::Output<U> {
        self.map(move |c| map(c.as_ref()))
    }
}

// pub trait SignalMapRefMaybeReactive<T: ?Sized> {
//     fn map_ref_maybe_reactive<U: PartialEq + 'static>(
//         &self,
//         map: impl FnMut(&T) -> U + 'static,
//     ) -> MaybeReactive<U>;
// }

// impl<T: ?Sized, C, R> SignalMapRefMaybeReactive<T> for R
// where
//     C: AsRef<T> + 'static,
//     R: ReactiveValue<Value = C> + SignalMap<C>,
// {
//     fn map_ref_maybe_reactive<U: PartialEq + 'static>(
//         &self,
//         mut map: impl FnMut(&T) -> U + 'static,
//     ) -> MaybeReactive<U> {
//         self.map(move |c| map(c.as_ref())).maybe_reactive()
//     }
// }

pub trait SignalMapSlice<T> {
    type Output<U: PartialEq + 'static>;

    fn map_slice<U: PartialEq + 'static>(
        &self,
        map: impl FnMut(&[T]) -> U + 'static,
    ) -> Self::Output<U>;
}

impl<T, R> SignalMapSlice<T> for R
where
    R: SignalMapRef<[T]>,
{
    type Output<U: PartialEq + 'static> =
        <Self as SignalMapRef<[T]>>::Output<U>;

    fn map_slice<U: PartialEq + 'static>(
        &self,
        mut map: impl FnMut(&[T]) -> U + 'static,
    ) -> Self::Output<U> {
        self.map_ref(move |c| map(c))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        prelude::*,
        read::{SignalMapRef, SignalMapSlice, WithRef},
    };
    use alloc::vec;
    use core::iter::Sum;

    #[test]
    fn with_slice_parameter() {
        fn accept_slice(slice: impl WithRef<[u32]>) -> u32 {
            slice.with_ref(|slice| slice.iter().sum())
        }

        let inert = vec![1, 2, 3].inert();
        assert_eq!(accept_slice(inert), 6);

        let signal = create_signal(vec![1, 2, 3]);
        assert_eq!(accept_slice(signal), 6);

        let maybe_signal_inert = MaybeSignal::new_inert(vec![1, 2, 3]);
        assert_eq!(accept_slice(maybe_signal_inert), 6);

        let maybe_signal_reactive = create_signal(vec![1, 2, 3]).maybe_signal();
        assert_eq!(accept_slice(maybe_signal_reactive), 6);

        let maybe_reactive_inert = vec![1, 2, 3].inert().maybe_reactive();
        assert_eq!(accept_slice(maybe_reactive_inert), 6);

        let maybe_reactive_signal =
            create_signal(vec![1, 2, 3]).maybe_reactive();
        assert_eq!(accept_slice(maybe_reactive_signal), 6);

        let boxed_slice = vec![1, 2, 3].into_boxed_slice().inert();
        assert_eq!(accept_slice(boxed_slice), 6);

        let array = [1, 2, 3].inert();
        assert_eq!(accept_slice(array), 6);
    }

    #[test]
    fn with_slice_inert_iter() {
        let inert = vec![1, 2, 3].inert();
        let sum = inert.with_slice(|slice| slice.iter().sum::<i32>());
        assert_eq!(sum, 6);
    }

    #[test]
    fn with_slice_signal_iter() {
        let signal = create_signal(vec![1, 2, 3]);
        let sum = signal.with_slice(|slice| slice.iter().sum::<i32>());
        assert_eq!(sum, 6);
    }

    #[test]
    fn signal_map_ref_memo_sum() {
        fn sum<T: Sum + Copy + PartialEq + 'static>(
            slice: impl SignalMapRef<[T], Output<T> = Memo<T>>,
        ) -> Memo<T> {
            slice.map_ref(|slice| slice.into_iter().copied().sum::<T>())
        }

        let signal = create_signal(vec![1, 2, 3]);
        let sum_memo = sum(signal);
        assert_eq!(sum_memo.get(), 6);

        let memo = create_memo(|| vec![1, 2, 3]);
        let sum_memo = sum(memo);
        assert_eq!(sum_memo.get(), 6);
    }

    #[test]
    fn signal_map_slice_memo_sum() {
        fn sum<T: Sum + Copy + PartialEq + 'static>(
            slice: impl SignalMapSlice<T, Output<T> = Memo<T>>,
        ) -> Memo<T> {
            slice.map_slice(|slice| slice.into_iter().copied().sum::<T>())
        }

        let signal = create_signal(vec![1, 2, 3]);
        let sum_memo = sum(signal);
        assert_eq!(sum_memo.get(), 6);

        let memo = create_memo(|| vec![1, 2, 3]);
        let sum_memo = sum(memo);
        assert_eq!(sum_memo.get(), 6);
    }

    #[test]
    fn signal_map_ref_maybe_reactive() {
        fn sum<T: Sum + Copy + PartialEq + 'static>(
            slice: impl SignalMapRef<[T], Output<T> = MaybeReactive<T>>,
        ) -> MaybeReactive<T> {
            slice.map_ref(|slice| slice.into_iter().copied().sum::<T>())
        }

        let signal = create_signal(vec![1, 2, 3]);
        let sum_maybe_reactive = sum(signal.maybe_reactive());
        assert_eq!(sum_maybe_reactive.get_untracked(), 6);

        let memo = create_memo(|| vec![1, 2, 3]);
        let sum_maybe_reactive = sum(memo.maybe_reactive());
        assert_eq!(sum_maybe_reactive.get_untracked(), 6);
    }

    #[test]
    fn signal_map_slice_maybe_reactive() {
        fn sum<T: Sum + Copy + PartialEq + 'static>(
            slice: impl SignalMapSlice<T, Output<T> = MaybeReactive<T>>,
        ) -> MaybeReactive<T> {
            slice.map_slice(|slice| slice.into_iter().copied().sum::<T>())
        }

        let signal = create_signal(vec![1, 2, 3]);
        let sum_maybe_reactive = sum(signal.maybe_reactive());
        assert_eq!(sum_maybe_reactive.get_untracked(), 6);

        let memo = create_memo(|| vec![1, 2, 3]);
        let sum_maybe_reactive = sum(memo.maybe_reactive());
        assert_eq!(sum_maybe_reactive.get_untracked(), 6);

        let inert = vec![1, 2, 3].inert();
        let sum_maybe_reactive = sum(inert.maybe_reactive());
        assert_eq!(sum_maybe_reactive.get_untracked(), 6);
    }

    // #[test]
    // fn signal_map_slice_into_maybe_reactive() {
    //     fn sum<
    //         T: Sum + Copy + PartialEq + 'static,
    //         S: SignalMapSlice<T, Output<T> = MaybeReactive<T>> + PartialEq,
    //     >(
    //         slice: S,
    //     ) -> MaybeReactive<T> {
    //         slice.map_slice(|slice| slice.into_iter().copied().sum::<T>())
    //     }

    //     let signal = create_signal(vec![1, 2, 3]);
    //     let sum_maybe_reactive = sum(signal);
    //     assert_eq!(sum_maybe_reactive.get_untracked(), 6);

    //     let memo = create_memo(|| vec![1, 2, 3]);
    //     let sum_maybe_reactive = sum(memo);
    //     assert_eq!(sum_maybe_reactive.get_untracked(), 6);
    // }
}
