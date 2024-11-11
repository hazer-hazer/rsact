pub trait ReadSignal<T> {
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

pub trait SignalMap<T: 'static> {
    type Output<U: PartialEq + 'static>;

    fn map<U: PartialEq + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Self::Output<U>;

    // TODO: Is this needed?
    #[track_caller]
    fn map_cloned<U: PartialEq + 'static>(
        &self,
        map: impl Fn(T) -> U + 'static,
    ) -> Self::Output<U>
    where
        Self: Sized + 'static,
        T: Clone,
    {
        self.map(move |this| map(this.clone()))
    }
}

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
        $param.mapped(|$param| $body)
    };

    (|$param: ident, $($rest: ident),+ $(,)?| $body: expr) => {
        $param.mapped(|$param| $crate::read::with!(|$($rest),+| $body))
    };

    (move |$param: ident $(,)?| $body: expr) => {
        $param.mapped(move |$param| $body)
    };

    (move |$param: ident, $($rest: ident),+ $(,)?| $body: expr) => {
        $param.mapped(move |$param| $crate::read::with!(move |$($rest),+| $body))
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
