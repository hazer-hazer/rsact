use crate::{
    effect::create_effect,
    prelude::*,
    read::impl_read_signal_traits,
    runtime::with_current_runtime,
    storage::ValueId,
    write::{SignalSetter, WriteSignal},
    ReactiveValue,
};
use alloc::rc::Rc;
use core::{marker::PhantomData, panic::Location};

#[track_caller]
pub fn create_signal<T: 'static>(value: T) -> Signal<T> {
    Signal::new(value)
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

impl<T: 'static, M: marker::Any> Clone for Signal<T, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static, M: marker::Any> Copy for Signal<T, M> {}

impl<T: 'static> ReactiveValue for Signal<T> {
    type Value = T;

    #[track_caller]
    fn is_alive(&self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }

    #[track_caller]
    fn dispose(self) {
        with_current_runtime(|rt| rt.dispose(self.id))
    }
}

impl_read_signal_traits!(Signal<T, marker::ReadOnly>, Signal<T, marker::Rw>);

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

    #[track_caller]
    pub fn is_alive(self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }

    #[track_caller]
    pub fn dispose(self) {
        with_current_runtime(|rt| rt.dispose(self.id))
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
    fn update_untracked<U>(&mut self, f: impl FnOnce(&mut T) -> U) -> U {
        let caller = Location::caller();
        with_current_runtime(|rt| self.id.update_untracked(rt, f, Some(caller)))
    }
}

/**
 * Set Signal<T> from Signal<U> mapped by `set_map`.
 */
impl<T: 'static, U: 'static> SignalSetter<T, Signal<U>> for Signal<T> {
    #[track_caller]
    fn setter(
        &mut self,
        source: Signal<U>,
        set_map: impl Fn(&mut T, &<Signal<U> as ReactiveValue>::Value) + 'static,
    ) {
        let this = *self;
        create_effect(move |_| {
            let mut this = this;
            source.with(|source| this.update(|this| set_map(this, source)))
        });
    }
}

impl<T: 'static, U: PartialEq + 'static> SignalSetter<T, Memo<U>>
    for Signal<T>
{
    #[track_caller]
    fn setter(
        &mut self,
        source: Memo<U>,
        set_map: impl Fn(&mut T, &<Memo<U> as ReactiveValue>::Value) + 'static,
    ) {
        let this = *self;
        create_effect(move |_| {
            let mut this = this;
            source.with(|source| this.update(|this| set_map(this, source)))
        });
    }
}

impl<T: 'static, U: PartialEq + 'static> SignalSetter<T, MemoChain<U>>
    for Signal<T>
{
    #[track_caller]
    fn setter(
        &mut self,
        source: MemoChain<U>,
        set_map: impl Fn(&mut T, &<Memo<U> as ReactiveValue>::Value) + 'static,
    ) {
        let this = *self;
        create_effect(move |_| {
            let mut this = this;
            source.with(|source| this.update(|this| set_map(this, source)))
        });
    }
}

impl<T: 'static, U: PartialEq + 'static> SignalSetter<T, Inert<U>>
    for Signal<T>
{
    #[track_caller]
    fn setter(
        &mut self,
        source: Inert<U>,
        set_map: impl Fn(&mut T, &<Inert<U> as ReactiveValue>::Value) + 'static,
    ) {
        self.update(|this| set_map(this, &source))
    }
}

impl<T: 'static, U: PartialEq + 'static> SignalSetter<T, MaybeReactive<U>>
    for Signal<T>
{
    #[track_caller]
    fn setter(
        &mut self,
        source: MaybeReactive<U>,
        set_map: impl Fn(&mut T, &<MaybeReactive<U> as ReactiveValue>::Value)
            + 'static,
    ) {
        match source {
            MaybeReactive::Inert(raw) => self.setter(raw, set_map),
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

impl<T: 'static, M: marker::CanRead> SignalMap<T> for Signal<T, M> {
    type Output<U: PartialEq + 'static> = Memo<U>;

    #[track_caller]
    fn map<U: PartialEq + 'static>(
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

/// Helper trait which converts anything except [`Signal`] into signal, and leaves [`Signal`] as it is.
pub trait IntoSignal<T: 'static> {
    fn signal(self) -> Signal<T>;
}

impl<T: 'static> IntoSignal<T> for Signal<T> {
    #[track_caller]
    fn signal(self) -> Signal<T> {
        self
    }
}

impl<T: 'static> IntoSignal<T> for T {
    #[track_caller]
    fn signal(self) -> Signal<T> {
        create_signal(self)
    }
}

// TODO: Remove, unused, does not makes sense unlike MemoTree
// #[derive(Clone, Copy)]
// pub struct SignalTree<T: 'static> {
//     pub data: Signal<T>,
//     pub children: Signal<Vec<SignalTree<T>>>,
// }

// impl<T: 'static> SignalTree<T> {
//     pub fn childless(data: Signal<T>) -> Self {
//         Self { data, children: create_signal(Vec::new()) }
//     }
// }

/**
 * Most tests are stolen from Reactively framework :)
 *
 * Important notes:
 * - To count effect/memo calls, use [`WriteSignal::update_untracked`] and [`ReadSignal::get_untracked`]
 *   on counters, as they should not affect reactive context dependencies.
 */
#[cfg(test)]
mod tests {
    use super::{ReadSignal, SignalSetter, WriteSignal};
    use crate::{
        effect::create_effect,
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
        let mut a = create_signal(5);
        let mut b_calls = create_signal(0);
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
        let mut a = create_signal(7);
        let mut b = create_signal(1);
        let mut memo_calls = create_signal(0);

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
        let mut a = create_signal(7);
        let b = create_signal(1);

        let mut c_memo_calls = create_signal(0);
        let c = create_memo(move |_| {
            c_memo_calls.update_untracked(|calls| *calls += 1);

            a.get() * b.get()
        });

        let mut d_memo_calls = create_signal(0);
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
        let mut memo_calls = create_signal(0);
        let mut a = create_signal(7);
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
        let mut a = create_signal(Some(1));
        let mut b = create_signal(Some(2));

        let mut m_a_calls = create_signal(0);
        let mut m_b_calls = create_signal(0);
        let mut m_ab_calls = create_signal(0);

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
        let mut a = create_signal(0);
        let b = create_memo(move |_| a.get() > 0);

        let mut c_calls = create_signal(0);
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
        let mut s = create_signal(1);
        let a = create_memo(move |_| s.get());
        let b = create_memo(move |_| a.get() * 2);
        let c = create_memo(move |_| a.get() * 3);

        let mut calls = create_signal(0);
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
        let mut s = create_signal(1);
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
        let mut a = create_signal(true);
        let mut b = create_signal(2);
        let mut calls = create_signal(0);

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
        let mut s = create_signal(2);
        let a = create_memo(move |_| s.get() + 1);
        let mut b_calls = create_signal(0);
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
        let mut s = create_signal(1);
        let mut done = create_signal(false);
        let mut calls = create_signal(0);

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
        let mut x = create_signal(0);

        let mut y = create_signal(0);
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
        let mut s = create_signal(1);

        let mut runs = create_signal(0);
        create_effect(move |_| {
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

    #[test]
    fn signal_setter() {
        let mut s1 = create_signal(1);
        let mut s2 = create_signal(2);

        assert_eq!(s1.get(), 1);
        assert_eq!(s2.get(), 2);

        s1.set_from(s2);

        assert_eq!(s1.get(), 2);
        assert_eq!(s2.get(), 2);

        s2.set(3);

        assert_eq!(s1.get(), 3);
        assert_eq!(s2.get(), 3);
    }
}
