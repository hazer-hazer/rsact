use crate::{
    effect::use_effect,
    prelude::{use_computed, use_signal, use_static},
    runtime::with_current_runtime,
    storage::ValueId,
};
use alloc::vec::Vec;
use core::{
    fmt::{Debug, Display},
    marker::PhantomData,
    ops::{
        Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor,
        BitXorAssign, ControlFlow, Deref, Div, DivAssign, Mul, MulAssign, Neg,
        Not, Rem, RemAssign, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
    },
    panic::Location,
};

/**
 * TODO: SmartSignal -- the structure that is just a stack-allocated data
 * until its first write, then it allocates new signal in runtime.
 */

pub trait UpdateNotification {
    fn is_updated(&self) -> bool;
}

// Maybe better only add this to ControlFlow without `UpdateNotification` trait
impl<B, C> UpdateNotification for ControlFlow<B, C> {
    fn is_updated(&self) -> bool {
        matches!(self, ControlFlow::Break(_))
    }
}

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
    fn get_cloned(&self) -> T
    where
        T: Clone,
    {
        self.with(T::clone)
    }

    #[track_caller]
    fn mapped<U: 'static>(self, f: impl Fn(&T) -> U + 'static) -> Signal<U>
    where
        Self: Sized + 'static,
    {
        let this = self;
        use_computed(move || this.with(|this| f(this)))
    }
}

pub trait WriteSignal<T> {
    fn notify(&self);
    fn update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> U;

    #[track_caller]
    fn control_flow<U: UpdateNotification>(
        &self,
        f: impl FnOnce(&mut T) -> U,
    ) -> U {
        let result = self.update_untracked(f);
        if result.is_updated() {
            self.notify();
        }
        result
    }

    #[track_caller]
    fn update<U>(&self, f: impl FnOnce(&mut T) -> U) -> U {
        let result = self.update_untracked(f);
        self.notify();
        result
    }

    #[track_caller]
    fn set(&self, new: T) {
        self.update(|value| *value = new)
    }
}

pub trait RwSignal<T>: ReadSignal<T> + WriteSignal<T> {}

impl<S, T> RwSignal<T> for S where S: ReadSignal<T> + WriteSignal<T> {}

pub mod marker {
    pub struct ReadOnly;
    pub struct WriteOnly;
    pub struct Rw;

    pub trait Any {}
    impl Any for Rw {}
    impl Any for ReadOnly {}
    impl Any for WriteOnly {}

    pub trait CanRead: Any {}
    impl CanRead for Rw {}
    impl CanRead for ReadOnly {}

    pub trait CanWrite: Any {}
    impl CanWrite for Rw {}
    impl CanWrite for WriteOnly {}
}

pub struct Signal<T, M: marker::Any = marker::Rw> {
    id: ValueId,
    ty: PhantomData<T>,
    rw: PhantomData<M>,
}

impl<T, M: marker::CanRead + marker::CanWrite> Signal<T, M> {
    // pub fn bound(&self, other: Signal<T>) -> Signal<T> {
    //     use_effect(move || {
    //         self.update_untracked(other.)
    //     });
    // }

    // pub fn aliased(&self, other: Signal<T>) -> Self {
    //     // with_current_runtime(|rt| rt.s)
    //     self
    // }

    #[track_caller]
    pub fn sync_with(self, other: Signal<T>) -> Self
    where
        Self: Sized + 'static,
        T: Copy + 'static,
    {
        use_effect(move |_| {
            self.update(|this| *this = other.get());
        });
        use_effect(move |_| {
            other.update(|other| *other = self.get());
        });
        self
    }
}

impl<T: 'static, M: marker::Any> Signal<T, M> {
    #[track_caller]
    pub fn new(value: T) -> Self {
        let caller = Location::caller();

        Self {
            id: with_current_runtime(|runtime| {
                runtime.storage.create_signal(value, caller)
            }),
            ty: PhantomData,
            rw: PhantomData,
        }
    }
}

impl<T: 'static, M: marker::CanRead> Signal<T, M> {
    pub fn read_only(self) -> Signal<T, marker::ReadOnly> {
        Signal { id: self.id, ty: PhantomData, rw: PhantomData }
    }
}

impl<T: 'static, M: marker::CanWrite> Signal<T, M> {
    pub fn write_only(self) -> Signal<T, marker::WriteOnly> {
        Signal { id: self.id, ty: PhantomData, rw: PhantomData }
    }
}

impl<T: 'static, M: marker::CanRead> ReadSignal<T> for Signal<T, M> {
    #[track_caller]
    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        with_current_runtime(|runtime| self.id.with_untracked(runtime, f))
    }

    #[track_caller]
    fn track(&self) {
        with_current_runtime(|rt| self.id.subscribe(rt))
    }
}

impl<T: 'static, M: marker::CanWrite> WriteSignal<T> for Signal<T, M> {
    #[track_caller]
    fn notify(&self) {
        let caller = Location::caller();
        let result = with_current_runtime(|rt| self.id.notify(rt, caller));

        if let Err(err) = result {
            match err {
                crate::storage::NotifyError::Cycle(_) => {},
                // crate::storage::NotifyError::Cycle(debug_info) => panic!(
                //     "Reactivity cycle at {}\nValue {}",
                //     core::panic::Location::caller(),
                //     debug_info
                // ),
            }
        }
    }

    #[track_caller]
    fn update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> U {
        with_current_runtime(|rt| self.id.update_untracked(rt, f))
    }
}

impl<T: 'static, M: marker::Any> Clone for Signal<T, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static, M: marker::Any> Copy for Signal<T, M> {}

#[repr(transparent)]
pub struct StaticSignal<T> {
    value: T,
}

impl<T> StaticSignal<T> {
    pub fn new(value: T) -> Self {
        Self { value }
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

impl<T> Deref for StaticSignal<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

// SmartSignal won't work, requires mutable captures in closures :(

// pub enum SmartSignal<T> {
//     Static(StaticSignal<T>),
//     Dynamic(Signal<T>),
// }

// impl<T> WriteSignal<T> for SmartSignal<T> {
//     fn notify(&self) {
//         if matches!(self, SmartSignal::Static(_)) {
//             *se
//         }
//     }

//     fn update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> U {
//         todo!()
//     }
// }

// impl<T: 'static> ReadSignal<T> for SmartSignal<T> {
//     fn track(&self) {
//         match self {
//             SmartSignal::Static(stat) => stat.track(),
//             SmartSignal::Dynamic(dynamic) => dynamic.track(),
//         }
//     }

//     fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
//         match self {
//             SmartSignal::Static(stat) => stat.with_untracked(f),
//             SmartSignal::Dynamic(dynamic) => dynamic.with_untracked(f),
//         }
//     }
// }

// impl<T> SmartSignal<T> {
//     pub fn new(value: T) -> Self {
//         Self::Static(StaticSignal::new(value))
//     }
// }

// Impl's //
macro_rules! impl_arith_with_assign {
    ($($trait: ident: $method: ident, $assign_trait: ident: $assign_method: ident);* $(;)?) => {
        $(
            impl<T, M> $trait for Signal<T, M>
            where
                T: $trait<Output = T> + Clone + 'static,
                M: marker::CanRead + 'static,
            {
                type Output = Signal<T>;

                #[track_caller]
                fn $method(self, rhs: Self) -> Self::Output {
                    use_computed(move || self.get_cloned().$method(rhs.get_cloned()))
                }
            }

            impl<T, M> $assign_trait for Signal<T, M>
            where
                T: $trait<Output = T> + Clone + 'static,
                M: marker::CanRead + marker::CanWrite + 'static,
            {
                #[track_caller]
                fn $assign_method(&mut self, rhs: Self) {
                    self.set(self.get_cloned().$method(rhs.get_cloned()))
                }
            }

            impl<T, M> $trait<T> for Signal<T, M>
            where
                T: $trait<Output = T> + Clone + 'static,
                M: marker::CanRead + 'static,
            {
                type Output = T;

                #[track_caller]
                fn $method(self, rhs: T) -> Self::Output {
                    self.get_cloned().$method(rhs)
                }
            }

            impl<T, M> $assign_trait<T> for Signal<T, M>
            where
                T: $trait<Output = T> + Clone + 'static,
                M: marker::CanRead + marker::CanWrite + 'static,
            {
                #[track_caller]
                fn $assign_method(&mut self, rhs: T) {
                    self.set(self.get_cloned().$method(rhs))
                }
            }
        )*
    };
}

impl_arith_with_assign! {
    Add: add, AddAssign: add_assign;
    Sub: sub, SubAssign: sub_assign;
    Mul: mul, MulAssign: mul_assign;
    Div: div, DivAssign: div_assign;
    Rem: rem, RemAssign: rem_assign;
    BitAnd: bitand, BitAndAssign: bitand_assign;
    BitOr: bitor, BitOrAssign: bitor_assign;
    BitXor: bitxor, BitXorAssign: bitxor_assign;
    Shl: shl, ShlAssign: shl_assign;
    Shr: shr, ShrAssign: shr_assign;
}

impl<T, M> Neg for Signal<T, M>
where
    T: Neg<Output = T> + Clone + 'static,
    M: marker::CanRead + 'static,
{
    type Output = Signal<T>;

    #[track_caller]
    fn neg(self) -> Self::Output {
        use_computed(move || self.get_cloned().neg())
    }
}

impl<T, M> Not for Signal<T, M>
where
    T: Not<Output = T> + Clone + 'static,
    M: marker::CanRead + 'static,
{
    type Output = Signal<T>;

    #[track_caller]
    fn not(self) -> Self::Output {
        use_computed(move || self.get_cloned().not())
    }
}

impl<T, M> PartialEq for Signal<T, M>
where
    T: PartialEq + Clone + 'static,
    M: marker::CanRead + 'static,
{
    #[track_caller]
    fn eq(&self, other: &Self) -> bool {
        self.get_cloned() == other.get_cloned()
    }
}

impl<T, M> PartialOrd for Signal<T, M>
where
    T: PartialEq + PartialOrd + Clone + 'static,
    M: marker::CanRead + 'static,
{
    #[track_caller]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.with(|this| other.with(|other| this.partial_cmp(other)))
    }
}

impl<T, M> Debug for Signal<T, M>
where
    T: Debug + 'static,
    M: marker::CanRead + 'static,
{
    #[track_caller]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.with(|this| this.fmt(f))
    }
}

impl<T, M> Display for Signal<T, M>
where
    T: Display + 'static,
    M: marker::CanRead + 'static,
{
    #[track_caller]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.with(|this| this.fmt(f))
    }
}

// impl<T: 'static, M: marker::CanRead, I: 'static> Signal<T, M>
// where
//     T: Deref<Target = [I]>,
// {
//     pub fn iter(&self) -> impl Iterator<Item = &I> {
//         self.with(|this| this.iter())
//     }
// }

pub trait EcoSignal<T> {
    type S: ReadSignal<T>;

    fn eco_signal(self) -> Self::S;
}

impl<T: 'static> EcoSignal<T> for Signal<T> {
    type S = Signal<T>;

    fn eco_signal(self) -> Self::S {
        self
    }
}

impl<T: 'static> EcoSignal<T> for T {
    type S = StaticSignal<T>;

    fn eco_signal(self) -> Self::S {
        use_static(self)
    }
}

pub trait IntoSignal<T: 'static> {
    fn signal(self) -> Signal<T>;
}

impl<T: 'static> IntoSignal<T> for Signal<T> {
    fn signal(self) -> Signal<T> {
        self
    }
}

impl<T: 'static> IntoSignal<T> for T {
    fn signal(self) -> Signal<T> {
        use_signal(self)
    }
}

#[derive(Clone, Copy)]
pub struct SignalTree<T: 'static> {
    pub data: Signal<T>,
    pub children: Signal<Vec<SignalTree<T>>>,
}

impl<T: 'static> SignalTree<T> {
    pub fn childless(data: Signal<T>) -> Self {
        Self { data, children: use_computed(Vec::new) }
    }
}

#[cfg(test)]
mod tests {
    use super::{ReadSignal, WriteSignal};
    use crate::{effect::use_effect, prelude::use_signal, signal::Signal};

    // #[test]
    // fn codependency() {
    //     let signal1 = use_signal(1);
    //     let signal2 = use_signal(2);

    //     use_effect(move |_| {
    //         signal1.update_untracked(move |signal1| *signal1 =
    // signal2.get());     });

    //     use_effect(move |_| {
    //         signal2.update_untracked(move |signal2| *signal2 =
    // signal1.get());     });

    //     signal1.set(3);

    //     assert_eq!(signal1.get(), 3);
    //     assert_eq!(signal1.get(), signal2.get());

    //     signal2.set(4);

    //     assert_eq!(signal2.get(), 4);
    //     assert_eq!(signal1.get(), signal2.get());
    // }

    #[test]
    fn sync_with() {
        let signal1 = use_signal(1);
        let signal2 = use_signal(2);
        signal2.sync_with(signal1);

        signal1.set(3);
        assert_eq!(signal1.get(), 3);
        assert_eq!(signal1.get(), signal2.get());

        signal2.set(4);
        assert_eq!(signal2.get(), 4);
        assert_eq!(signal1.get(), signal2.get());
    }

    #[test]
    fn sync_with_reactivity() {
        let main = use_signal(1);
        let secondary = use_signal(0);

        secondary.sync_with(main);

        let affected = use_signal(0);
        use_effect(move |_| {
            affected.set(secondary.get());
        });

        main.set(123);

        assert_eq!(affected.get(), 123);
    }
}
