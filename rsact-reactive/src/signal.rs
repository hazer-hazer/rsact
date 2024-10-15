use crate::{
    composables::use_memo,
    effect::use_effect,
    memo::Memo,
    prelude::{use_signal, use_static},
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

impl<T: Send + 'static, M: marker::Any> Signal<T, M> {
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

pub trait SignalSetter<T: 'static> {
    fn setter<U: 'static>(
        self,
        other: impl ReadSignal<U> + Send + 'static,
        f: impl Fn(&U, &mut T) + Send + 'static,
    );
    fn set_from(self, other: impl ReadSignal<T> + Send + 'static)
    where
        T: Clone,
        Self: Sized + 'static,
    {
        self.setter(other, |new, this| *this = new.clone());
    }
}

impl<S, T> SignalSetter<T> for S
where
    S: WriteSignal<T> + Send + 'static,
    T: Send + 'static,
{
    fn setter<U: 'static>(
        self,
        other: impl ReadSignal<U> + Send + 'static,
        f: impl Fn(&U, &mut T) + Send + 'static,
    ) {
        use_effect(move |_| {
            other.with(|other| {
                self.update(|this| f(other, this));
            });
        });
    }
}

pub trait SignalMapper<T: Send + 'static> {
    fn mapped<U: PartialEq + Send + 'static>(
        self,
        map: impl Fn(&T) -> U + Send + 'static,
    ) -> Memo<U>;

    fn mapped_clone<U: PartialEq + Send + 'static>(
        self,
        map: impl Fn(T) -> U + Send + 'static,
    ) -> Memo<U>
    where
        Self: Sized + 'static,
        T: Clone,
    {
        self.mapped(move |this| map(this.clone()))
    }
}

impl<S, T: Send + 'static> SignalMapper<T> for S
where
    S: ReadSignal<T> + Send + 'static,
{
    fn mapped<U: PartialEq + Send + 'static>(
        self,
        map: impl Fn(&T) -> U + Send + 'static,
    ) -> Memo<U> {
        use_memo(move |_| self.with(|value| map(value)))
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

// Impl's //
macro_rules! impl_arith_with_assign {
    ($($trait: ident: $method: ident, $assign_trait: ident: $assign_method: ident);* $(;)?) => {
        $(
            impl<T, M> $trait for Signal<T, M>
            where
                T: $trait<Output = T> + PartialEq + Clone + Send + 'static,
                M: marker::CanRead + Send + 'static,
            {
                type Output = Memo<T>;

                #[track_caller]
                fn $method(self, rhs: Self) -> Self::Output {
                    use_memo(move |_| self.get_cloned().$method(rhs.get_cloned()))
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
    T: Neg<Output = T> + PartialEq + Clone + Send + 'static,
    M: marker::CanRead + Send + 'static,
{
    type Output = Memo<T>;

    #[track_caller]
    fn neg(self) -> Self::Output {
        use_memo(move |_| self.get_cloned().neg())
    }
}

impl<T, M> Not for Signal<T, M>
where
    T: Not<Output = T> + PartialEq + Clone + Send + 'static,
    M: marker::CanRead + Send + 'static,
{
    type Output = Memo<T>;

    #[track_caller]
    fn not(self) -> Self::Output {
        use_memo(move |_| self.get_cloned().not())
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

pub trait MaybeSignal<T> {
    type S: ReadSignal<T> + 'static;

    fn maybe_signal(self) -> Self::S;
}

impl<T: 'static> MaybeSignal<T> for Signal<T> {
    type S = Signal<T>;

    fn maybe_signal(self) -> Self::S {
        self
    }
}

impl<T: 'static> MaybeSignal<T> for T {
    type S = StaticSignal<T>;

    fn maybe_signal(self) -> Self::S {
        use_static(self)
    }
}

pub trait IntoSignal<T: 'static> {
    fn into_signal(self) -> Signal<T>;
}

impl<T: 'static> IntoSignal<T> for Signal<T> {
    fn into_signal(self) -> Signal<T> {
        self
    }
}

impl<T: Send + 'static> IntoSignal<T> for T {
    fn into_signal(self) -> Signal<T> {
        use_signal(self)
    }
}

#[derive(Clone, Copy)]
pub struct SignalTree<T: 'static> {
    pub data: Signal<T>,
    pub children: Signal<Vec<SignalTree<T>>>,
}

impl<T: Send + 'static> SignalTree<T> {
    pub fn childless(data: Signal<T>) -> Self {
        Self { data, children: use_signal(Vec::new()) }
    }
}

/**
 * Most tests are lost from Reactively framework :)
 *
 * Important notes:
 * - To count effect/memo calls, use `update_untracked` and `get_untracked`
 *   on counters, as they should not affect reactive context dependencies.
 */
#[cfg(test)]
mod tests {
    use super::{ReadSignal, WriteSignal};
    use crate::prelude::{use_memo, use_signal};

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
    fn simple_memo() {
        let a = use_memo(move |_| 123);

        assert_eq!(a.get(), 123);
    }

    #[test]
    fn one_level_memo() {
        let a = use_signal(5);
        let b_calls = use_signal(0);
        let b = use_memo(move |_| {
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
        let a = use_signal(7);
        let b = use_signal(1);
        let memo_calls = use_signal(0);

        let c = use_memo(move |_| {
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
        let a = use_signal(7);
        let b = use_signal(1);

        let c_memo_calls = use_signal(0);
        let c = use_memo(move |_| {
            c_memo_calls.update_untracked(|calls| *calls += 1);

            a.get() * b.get()
        });

        let d_memo_calls = use_signal(0);
        let d = use_memo(move |_| {
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
        let memo_calls = use_signal(0);
        let a = use_signal(7);
        let c = use_memo(move |_| {
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
        let a = use_signal(Some(1));
        let b = use_signal(Some(2));

        let m_a_calls = use_signal(0);
        let m_b_calls = use_signal(0);
        let m_ab_calls = use_signal(0);

        let m_a = use_memo(move |_| {
            m_a_calls.update_untracked(|calls| *calls += 1);

            a.get()
        });

        let m_b = use_memo(move |_| {
            m_b_calls.update_untracked(|calls| *calls += 1);
            b.get()
        });

        let m_ab = use_memo(move |_| {
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
        let a = use_signal(0);
        let b = use_memo(move |_| a.get() > 0);

        let c_calls = use_signal(0);
        let c = use_memo(move |_| {
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
        let a = use_signal(10);
        let b = use_memo(move |_| a.get() * 10);
        let c = use_memo(move |_| a.get() * 20);
        let d = use_memo(move |_| b.get() + c.get());

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
        let s = use_signal(1);
        let a = use_memo(move |_| s.get());
        let b = use_memo(move |_| a.get() * 2);
        let c = use_memo(move |_| a.get() * 3);

        let calls = use_signal(0);
        let d = use_memo(move |_| {
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
        let s = use_signal(1);
        let a = use_memo(move |_| s.set(2));
        let l = use_memo(move |_| s.get() + 100);

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
        let a = use_signal(true);
        let b = use_signal(2);
        let calls = use_signal(0);

        let c = use_memo(move |_| {
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
        let s = use_signal(2);
        let a = use_memo(move |_| s.get() + 1);
        let b_calls = use_signal(0);
        let b = use_memo(move |_| {
            b_calls.update_untracked(|calls| *calls += 1);
            s.get() + 10
        });
        let l = use_memo(move |_| {
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
        let s = use_signal(1);
        let done = use_signal(false);
        let calls = use_signal(0);

        let c = use_memo(move |_| {
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
        let z = use_signal(3);
        let x = use_signal(0);

        let y = use_signal(0);
        let i = use_memo(move |_| {
            let a = y.get();
            z.get();
            if a == 0 {
                x.get()
            } else {
                a
            }
        });
        let j = use_memo(move |_| {
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
}
