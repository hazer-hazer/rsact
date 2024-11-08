use crate::{
    effect::use_effect, prelude::*, runtime::with_current_runtime,
    storage::ValueId,
};
use alloc::{rc::Rc, vec::Vec};
use core::{
    fmt::{Debug, Display},
    marker::PhantomData,
    ops::{
        Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor,
        BitXorAssign, ControlFlow, Div, DivAssign, Mul, MulAssign, Neg,
        Not, Rem, RemAssign, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
    },
    panic::Location,
};

#[track_caller]
pub fn create_signal<T: 'static>(value: T) -> Signal<T> {
    Signal::new(value)
}

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

// TODO: Add `replace` method for rw which will take current value leaving Default if default implemented

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

    #[track_caller]
    fn set_untracked(&self, new: T) {
        self.update_untracked(|value| *value = new)
    }
}

pub trait RwSignal<T>: ReadSignal<T> + WriteSignal<T> {}

impl<S, T> RwSignal<T> for S where S: ReadSignal<T> + WriteSignal<T> {}

pub mod marker {
    pub struct ReadOnly;
    pub struct WriteOnly;
    pub struct Rw;

    pub trait Any: 'static {}
    impl Any for Rw {}
    impl Any for ReadOnly {}
    impl Any for WriteOnly {}

    pub trait CanRead: Any + 'static {}
    impl CanRead for Rw {}
    impl CanRead for ReadOnly {}

    pub trait CanWrite: Any + 'static {}
    impl CanWrite for Rw {}
    impl CanWrite for WriteOnly {}
}

pub struct Signal<T, M: marker::Any = marker::Rw> {
    id: ValueId,
    ty: PhantomData<T>,
    rw: PhantomData<M>,
}

impl<T: 'static, M: marker::Any> Signal<T, M> {
    #[track_caller]
    pub fn new(value: T) -> Self {
        let caller = Location::caller();

        Self {
            id: with_current_runtime(|runtime| {
                runtime.create_signal(value, caller)
            }),
            ty: PhantomData,
            rw: PhantomData,
        }
    }

    pub fn is_alive(self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }

    pub fn dispose(self) {
        with_current_runtime(|rt| rt.dispose(self.id))
    }
}

/**
 * SignalSetter must be as follows
 * enum SignalSetter<T, P> {
 *     Default,
 *     Map(Box<FnMut(&T, P)>),
 *     Target(Signal<T>),
 * }
 *
 * - Default always sets to default
 * - Map sets type by passed parameter
 * - Target just sets signal with new value
 */

// Note: This SignalSetter is shit

// pub trait SignalSetter<T: 'static, D> {
//     fn setter<U: 'static>(
//         self,
//         other: impl ReadSignal<U> + 'static,
//         f: impl Fn(&U, &mut T) + 'static,
//     );
//     fn set_from(self, other: impl ReadSignal<T> + 'static)
//     where
//         T: Clone,
//         Self: Sized + 'static,
//     {
//         self.setter(other, |new, this| *this = new.clone());
//     }
// }

// // TODO: It looks very bad with `use_effect`, need a SignalSetter value kind.
// /// Setting reactive value by other reactive value
// impl<S, T> SignalSetter<T, S> for S
// where
//     S: WriteSignal<T> + 'static,
//     T: 'static,
// {
//     fn setter<U: 'static>(
//         self,
//         other: impl ReadSignal<U> + 'static,
//         f: impl Fn(&U, &mut T) + 'static,
//     ) {
//         use_effect(move |_| {
//             other.with(|other| {
//                 self.update(|this| f(other, this));
//             });
//         });
//     }
// }

/// SignalValue is used as HKT abstraction over reactive (or not) types such as Signal<T> (Value = T), Memo<T>, MaybeReactive<T>, etc.
pub trait SignalValue: 'static {
    type Value;
}

impl<T: 'static> SignalValue for Signal<T> {
    type Value = T;
}

pub trait SignalSetter<T: 'static, I: SignalValue> {
    fn setter(&self, source: I, set_map: impl Fn(&mut T, &I::Value) + 'static);

    fn set_from(&self, source: I)
    where
        T: Clone,
        I: SignalValue<Value = T>,
        Self: Sized + 'static,
    {
        self.setter(source, |this, new| *this = new.clone());
    }
}

/**
 * Set Signal<T> from Signal<U> mapped by `set_map`.
 */
impl<T: 'static, U: 'static> SignalSetter<T, Signal<U>> for Signal<T> {
    fn setter(
        &self,
        source: Signal<U>,
        set_map: impl Fn(&mut T, &<Signal<U> as SignalValue>::Value) + 'static,
    ) {
        let this = *self;
        use_effect(move |_| {
            source.with(|source| this.update(|this| set_map(this, source)))
        });
    }
}

impl<T: 'static, U: PartialEq + 'static> SignalSetter<T, Memo<U>>
    for Signal<T>
{
    fn setter(
        &self,
        source: Memo<U>,
        set_map: impl Fn(&mut T, &<Memo<U> as SignalValue>::Value) + 'static,
    ) {
        let this = *self;
        use_effect(move |_| {
            source.with(|source| this.update(|this| set_map(this, source)))
        });
    }
}

impl<T: 'static, U: PartialEq + 'static> SignalSetter<T, MemoChain<U>>
    for Signal<T>
{
    fn setter(
        &self,
        source: MemoChain<U>,
        set_map: impl Fn(&mut T, &<Memo<U> as SignalValue>::Value) + 'static,
    ) {
        let this = *self;
        use_effect(move |_| {
            source.with(|source| this.update(|this| set_map(this, source)))
        });
    }
}

impl<T: 'static, U: PartialEq + 'static> SignalSetter<T, MaybeReactive<U>>
    for Signal<T>
{
    fn setter(
        &self,
        source: MaybeReactive<U>,
        set_map: impl Fn(&mut T, &<MaybeReactive<U> as SignalValue>::Value)
            + 'static,
    ) {
        match source {
            MaybeReactive::Static(raw) => {
                self.update(|this| set_map(this, &raw))
            },
            MaybeReactive::Signal(signal) => self.setter(signal, set_map),
            MaybeReactive::Memo(memo) => self.setter(memo, set_map),
            MaybeReactive::MemoChain(memo_chain) => {
                self.setter(memo_chain, set_map)
            },
            MaybeReactive::Derived(derived) => {
                // TODO: use_effect or not to use effect? How do we know if derived function is using reactive values or not
                let derived = Rc::clone(&derived);
                self.update(|this| set_map(this, &derived()));
            },
        }
    }
}

pub trait SignalMapper<T: 'static> {
    type Output<U: PartialEq + 'static>;

    fn mapped<U: PartialEq + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Self::Output<U>;

    #[track_caller]
    fn mapped_clone<U: PartialEq + 'static>(
        &self,
        map: impl Fn(T) -> U + 'static,
    ) -> Self::Output<U>
    where
        Self: Sized + 'static,
        T: Clone,
    {
        self.mapped(move |this| map(this.clone()))
    }
}

// TODO: Implement only for the Signal struct, not all ReadSignal's.
impl<T: 'static, M: marker::CanRead> SignalMapper<T> for Signal<T, M> {
    type Output<U: PartialEq + 'static> = Memo<U>;

    #[track_caller]
    fn mapped<U: PartialEq + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Memo<U> {
        let this = *self;
        create_memo(move |_| this.with(&map))
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
        let caller = Location::caller();
        with_current_runtime(|runtime| {
            self.id.with_untracked(runtime, f, caller)
        })
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
                // crate::storage::NotifyError::Cycle(_) => {},
                crate::storage::NotifyError::Cycle(debug_info) => panic!(
                    "Reactivity cycle at {}\nValue {}",
                    core::panic::Location::caller(),
                    debug_info
                ),
            }
        }
    }

    #[track_caller]
    fn update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> U {
        let caller = Location::caller();
        with_current_runtime(|rt| self.id.update_untracked(rt, f, Some(caller)))
    }
}

impl<T: 'static, M: marker::Any> Clone for Signal<T, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static, M: marker::Any> Copy for Signal<T, M> {}

// Impl's //
macro_rules! impl_arith_with_assign {
    ($($trait: ident: $method: ident, $assign_trait: ident: $assign_method: ident);* $(;)?) => {
        $(
            impl<T, M> $trait for Signal<T, M>
            where
                T: $trait<Output = T> + PartialEq + Clone + 'static,
                M: marker::CanRead + 'static,
            {
                type Output = Memo<T>;

                #[track_caller]
                fn $method(self, rhs: Self) -> Self::Output {
                    create_memo(move |_| self.get_cloned().$method(rhs.get_cloned()))
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
    T: Neg<Output = T> + PartialEq + Clone + 'static,
    M: marker::CanRead + 'static,
{
    type Output = Memo<T>;

    #[track_caller]
    fn neg(self) -> Self::Output {
        create_memo(move |_| self.get_cloned().neg())
    }
}

impl<T, M> Not for Signal<T, M>
where
    T: Not<Output = T> + PartialEq + Clone + 'static,
    M: marker::CanRead + 'static,
{
    type Output = Memo<T>;

    #[track_caller]
    fn not(self) -> Self::Output {
        create_memo(move |_| self.get_cloned().not())
    }
}

// TODO: Remove PartialEq and PartialOrd to avoid conflicting with `AsMemo` and to require this methods not to implicitly get reactive value. User should always know if reactive value is unwrapped.
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

// pub trait MaybeSignal<T> {
//     type S: ReadSignal<T> + 'static;

//     fn maybe_signal(self) -> Self::S;
// }

// impl<T: 'static> MaybeSignal<T> for Signal<T> {
//     type S = Signal<T>;

//     fn maybe_signal(self) -> Self::S {
//         self
//     }
// }

// impl<T: 'static> MaybeSignal<T> for T {
//     type S = StaticSignal<T>;

//     fn maybe_signal(self) -> Self::S {
//         create_static(self)
//     }
// }

pub trait IntoSignal<T: 'static> {
    fn into_signal(self) -> Signal<T>;
}

impl<T: 'static> IntoSignal<T> for Signal<T> {
    #[track_caller]
    fn into_signal(self) -> Signal<T> {
        self
    }
}

impl<T: 'static> IntoSignal<T> for T {
    #[track_caller]
    fn into_signal(self) -> Signal<T> {
        create_signal(self)
    }
}

#[derive(Clone, Copy)]
pub struct SignalTree<T: 'static> {
    pub data: Signal<T>,
    pub children: Signal<Vec<SignalTree<T>>>,
}

impl<T: 'static> SignalTree<T> {
    pub fn childless(data: Signal<T>) -> Self {
        Self { data, children: create_signal(Vec::new()) }
    }
}

/**
 * Most tests are lost from Reactively framework :)
 *
 * Important notes:
 * - To count effect/memo calls, use [`WriteSignal::update_untracked`] and [`ReadSignal::get_untracked`]
 *   on counters, as they should not affect reactive context dependencies.
 */
#[cfg(test)]
mod tests {
    use super::{ReadSignal, WriteSignal};
    use crate::{
        effect::use_effect,
        prelude::{create_memo, create_signal},
    };

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
    fn one_level_memo() {
        let a = create_signal(5);
        let b_calls = create_signal(0);
        let b = create_memo(move |_| {
            b_calls.update_untracked(|calls| *calls += 1);

            a.get() * 10
        });

        assert_eq!(b_calls.get(), 0);

        assert_eq!(b.get(), 50);
        b.get();
        assert_eq!(b_calls.get(), 1);

        a.set(10);
        assert_eq!(b.get(), 100);
        assert_eq!(b_calls.get(), 2);
    }

    /*
       a  b
       | /
       c
    */
    #[test]
    fn two_signals() {
        let a = create_signal(7);
        let b = create_signal(1);
        let memo_calls = create_signal(0);

        let c = create_memo(move |_| {
            memo_calls.update_untracked(|calls| *calls += 1);
            a.get() * b.get()
        });

        assert_eq!(c.get(), 7);

        // After first access to a memo, it is called
        assert_eq!(memo_calls.get(), 1);

        a.set(2);
        assert_eq!(c.get(), 2);

        assert_eq!(memo_calls.get(), 2);

        b.set(3);
        assert_eq!(c.get(), 6);

        assert_eq!(memo_calls.get(), 3);
        c.get();
        c.get();
        assert_eq!(memo_calls.get(), 3);
    }

    /*
       a  b
       | /
       c
       |
       d
    */
    #[test]
    fn dependent_memos() {
        let a = create_signal(7);
        let b = create_signal(1);

        let c_memo_calls = create_signal(0);
        let c = create_memo(move |_| {
            c_memo_calls.update_untracked(|calls| *calls += 1);

            a.get() * b.get()
        });

        let d_memo_calls = create_signal(0);
        let d = create_memo(move |_| {
            d_memo_calls.update_untracked(|calls| *calls += 1);
            c.get() + 1
        });

        assert_eq!(c_memo_calls.get(), 0);
        assert_eq!(d_memo_calls.get(), 0);

        assert_eq!(d.get(), 8);
        assert_eq!(c_memo_calls.get(), 1);
        assert_eq!(d_memo_calls.get(), 1);

        a.set(3);
        assert_eq!(d.get(), 4);
        assert_eq!(c_memo_calls.get(), 2);
        assert_eq!(d_memo_calls.get(), 2);
    }

    /*
       a
       |
       c
    */
    #[test]
    fn equality_check() {
        let memo_calls = create_signal(0);
        let a = create_signal(7);
        let c = create_memo(move |_| {
            memo_calls.update_untracked(|calls| *calls += 1);

            a.get() + 10
        });
        c.get();
        c.get();

        assert_eq!(memo_calls.get(), 1);
        a.set(123);
        assert_eq!(memo_calls.get(), 1);
    }

    /*
       a     b
       |     |
       cA   cB
       |   / (dynamically depends on cB)
       cAB
    */
    #[test]
    fn dynamic_memo_dep() {
        let a = create_signal(Some(1));
        let b = create_signal(Some(2));

        let m_a_calls = create_signal(0);
        let m_b_calls = create_signal(0);
        let m_ab_calls = create_signal(0);

        let m_a = create_memo(move |_| {
            m_a_calls.update_untracked(|calls| *calls += 1);

            a.get()
        });

        let m_b = create_memo(move |_| {
            m_b_calls.update_untracked(|calls| *calls += 1);
            b.get()
        });

        let m_ab = create_memo(move |_| {
            m_ab_calls.update_untracked(|calls| *calls += 1);
            m_a.get().or_else(|| m_b.get())
        });

        assert_eq!(m_ab.get(), Some(1));

        a.set(Some(2));
        b.set(Some(3));
        assert_eq!(m_ab.get(), Some(2));
        assert_eq!(m_a_calls.get(), 2);
        assert_eq!(m_b_calls.get(), 0);
        assert_eq!(m_ab_calls.get(), 2);

        a.set(None);
        assert_eq!(m_ab.get(), Some(3));
        assert_eq!(m_a_calls.get(), 3);
        assert_eq!(m_b_calls.get(), 1);
        assert_eq!(m_ab_calls.get(), 3);

        b.set(Some(4));
        assert_eq!(m_ab.get(), Some(4));
        assert_eq!(m_a_calls.get(), 3);
        assert_eq!(m_b_calls.get(), 2);
        assert_eq!(m_ab_calls.get(), 4);
    }

    /*
         a
         |
         b (=)
         |
         c
    */
    #[test]
    fn bool_equality_check() {
        let a = create_signal(0);
        let b = create_memo(move |_| a.get() > 0);

        let c_calls = create_signal(0);
        let c = create_memo(move |_| {
            c_calls.update_untracked(|calls| *calls += 1);
            if b.get() {
                1
            } else {
                0
            }
        });

        assert_eq!(c.get(), 0);
        assert_eq!(c_calls.get(), 1);

        a.set(1);
        assert_eq!(c.get(), 1);
        assert_eq!(c_calls.get(), 2);

        a.set(2);
        assert_eq!(c.get(), 1);
        assert_eq!(c_calls.get(), 2);
    }

    #[test]
    fn simple_diamond() {
        let a = create_signal(10);
        let b = create_memo(move |_| a.get() * 10);
        let c = create_memo(move |_| a.get() * 20);
        let d = create_memo(move |_| b.get() + c.get());

        assert_eq!(d.get(), 300);
    }

    /*
       s
       |
       a
       | \
       b  c
        \ |
          d
    */
    #[test]
    fn diamond() {
        let s = create_signal(1);
        let a = create_memo(move |_| s.get());
        let b = create_memo(move |_| a.get() * 2);
        let c = create_memo(move |_| a.get() * 3);

        let calls = create_signal(0);
        let d = create_memo(move |_| {
            calls.update_untracked(|calls| *calls += 1);
            b.get() + c.get()
        });

        assert_eq!(d.get(), 5);
        assert_eq!(calls.get(), 1);

        s.set(2);
        assert_eq!(d.get(), 10);
        assert_eq!(calls.get(), 2);

        s.set(3);
        assert_eq!(d.get(), 15);
        assert_eq!(calls.get(), 3);
    }

    /*
       s
       |
       l  a (sets s)
    */
    #[test]
    fn set_inside_memo() {
        let s = create_signal(1);
        let a = create_memo(move |_| s.set(2));
        let l = create_memo(move |_| s.get() + 100);

        a.get();
        assert_eq!(l.get(), 102);
    }

    // Dynamic memos //
    /*
        a  b          a
        | /     or    |
        c             c
    */
    #[test]
    fn dynamic() {
        let a = create_signal(true);
        let b = create_signal(2);
        let calls = create_signal(0);

        let c = create_memo(move |_| {
            calls.update_untracked(|calls| *calls += 1);
            a.get().then(|| b.get())
        });

        assert_eq!(calls.get(), 0);

        c.get();
        assert_eq!(calls.get(), 1);

        a.set(false);
        c.get();
        assert_eq!(calls.get(), 2);

        // Even changing `b` which is used inside `c`, the `c` isn't called,
        // because `b` is cleared from dependency tree and isn't used as `a` is
        // `false`
        b.set(4);
        c.get();
        assert_eq!(calls.get(), 2);
    }

    /*
     dependency is dynamic: sometimes l depends on b, sometimes not.
     s          s
     / \        / \
     a   b  or  a   b
     \ /        \
     l          l
    */
    #[test]
    fn no_unnecessary_recompute() {
        let s = create_signal(2);
        let a = create_memo(move |_| s.get() + 1);
        let b_calls = create_signal(0);
        let b = create_memo(move |_| {
            b_calls.update_untracked(|calls| *calls += 1);
            s.get() + 10
        });
        let l = create_memo(move |_| {
            let mut result = a.get();
            if result % 2 == 1 {
                result += b.get();
            }
            result
        });

        assert_eq!(l.get(), 15);
        assert_eq!(b_calls.get(), 1);

        s.set(3);
        assert_eq!(l.get(), 4);
        assert_eq!(b_calls.get(), 1);
    }

    /*
       s
       |
       l
    */
    #[test]
    fn vanishing_dependency() {
        let s = create_signal(1);
        let done = create_signal(false);
        let calls = create_signal(0);

        let c = create_memo(move |_| {
            calls.update_untracked(|calls| *calls += 1);

            if done.get() {
                0
            } else {
                let value = s.get();
                if value > 2 {
                    done.set(true);
                }
                value
            }
        });

        assert_eq!(c.get(), 1);
        assert_eq!(calls.get(), 1);

        s.set(3);
        assert_eq!(c.get(), 3);
        assert_eq!(calls.get(), 2);

        s.set(1); // we've now locked into 'done' state
        assert_eq!(c.get(), 0);
        assert_eq!(calls.get(), 3);

        // we're still locked into 'done' state, and count no longer advances
        // in fact, c() will never execute again...
        s.set(0);
        assert_eq!(c.get(), 0);
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn dynamic_graph_does_not_crash() {
        let z = create_signal(3);
        let x = create_signal(0);

        let y = create_signal(0);
        let i = create_memo(move |_| {
            let a = y.get();
            z.get();
            if a == 0 {
                x.get()
            } else {
                a
            }
        });
        let j = create_memo(move |_| {
            let a = i.get();
            z.get();
            if a == 0 {
                x.get()
            } else {
                a
            }
        });

        j.get();
        x.set(1);
        j.get();
        y.set(1);
        j.get();
    }

    // Effects //
    #[test]
    fn effect_run_order() {
        let s = create_signal(1);

        let runs = create_signal(0);
        use_effect(move |_| {
            runs.update_untracked(|runs| *runs += 1);

            s.get();
        });

        assert_eq!(runs.get(), 1);

        s.set(2);
        assert_eq!(runs.get(), 2);

        s.update(|s| {
            *s = 123;
            assert_eq!(runs.get(), 2);
        });
        assert_eq!(runs.get(), 3);
    }

    // #[test]
    // fn borrow_in_effect() {
    //     struct ValueUser {
    //         value: Signal<i32>,
    //     }

    //     let user = create_signal(ValueUser { value: create_signal(123) });
    //     let runs = create_signal(0);
    //     use_effect(move |_| {
    //         runs.update_untracked(|runs| *runs += 1);

    //         // Use value
    //         user.with(|user| user.value.get());
    //     });

    //     // This is how user "should" do it. Not borrowing user mutably
    //     user.with(|user| {
    //         user.value.update(|_| {});
    //     });

    //     // This could panic with borrowing error, but we run effects only in non-reactive contexts.
    //     user.update(|user| {
    //         user.value.update(|_| {});
    //     });
    // }
}
