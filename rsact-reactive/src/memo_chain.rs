use crate::{
    ReactiveValue,
    callback::{AnyCallback, CallbackFn},
    memo::{IntoMemo, Memo},
    prelude::create_memo,
    read::{ReadSignal, SignalMap, impl_read_signal_traits},
    runtime::with_current_runtime,
    storage::ValueId,
};
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell, marker::PhantomData, panic::Location};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MemoChainErr {
    FirstRedefined,
    LastRedefined,
}

#[track_caller]
pub fn create_memo_chain<T: PartialEq + 'static, P: 'static>(
    f: impl CallbackFn<T, P>,
) -> MemoChain<T> {
    MemoChain::new(f)
}

pub struct MemoChainCallback<T, F>
where
    F: FnMut(&T) -> T,
{
    pub(crate) f: F,
    pub(crate) ty: PhantomData<T>,
}

impl<T, F> MemoChainCallback<T, F>
where
    F: FnMut(&T) -> T,
{
    pub fn new(f: F) -> Self {
        Self { f, ty: PhantomData }
    }
}

// TODO: Optimize, should not set the value but accept it, change it and then
// pass to the next MemoChainCallback
impl<T, F> AnyCallback for MemoChainCallback<T, F>
where
    F: FnMut(&T) -> T,
    T: PartialEq + 'static,
{
    fn run(&mut self, value: Rc<RefCell<dyn Any>>) -> bool {
        let (new_value, changed) = {
            let value = value.borrow();
            let value = value
                .downcast_ref::<Option<T>>()
                .unwrap()
                .as_ref()
                .expect("Must already been set");

            let new_value = (self.f)(value);
            let changed = PartialEq::ne(value, &new_value);
            (new_value, changed)
        };

        if changed {
            let mut value = value.borrow_mut();
            let value = value.downcast_mut::<Option<T>>().unwrap();
            value.replace(new_value);
        }

        changed
    }
}

pub struct MemoChain<T: PartialEq> {
    id: ValueId,
    ty: PhantomData<T>,
}

impl_read_signal_traits!(MemoChain<T>: PartialEq);

impl<T: PartialEq + 'static> MemoChain<T> {
    #[track_caller]
    pub fn new<P: 'static>(f: impl CallbackFn<T, P>) -> Self {
        let caller = Location::caller();
        Self {
            id: with_current_runtime(|rt| rt.create_memo_chain(f, caller)),
            ty: PhantomData,
        }
    }

    pub fn is_alive(self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }

    // TODO: Add methods `first_mapped` and `last_mapped` which will wrap first and last mappers with new mapping memos. This will allow infinite depth of chain.
    // Also it is better to add common logic for such case as memo replacement -- allow replacing memo with new function functionality. It seems very bad practice, but I see cases where it might be needed. Of course, preserving initial return type.

    // TODO: Should these methods require mutable access?
    #[must_use = "Setting memo chain can fail"]
    pub fn first(
        self,
        f: impl Fn(&T) -> T + 'static,
    ) -> Result<Self, MemoChainErr> {
        with_current_runtime(|rt| rt.set_memo_chain(self.id, true, f))
            .map(|_| self)
    }

    #[must_use = "Setting memo chain can fail"]
    pub fn last(
        self,
        f: impl Fn(&T) -> T + 'static,
    ) -> Result<Self, MemoChainErr> {
        with_current_runtime(|rt| rt.set_memo_chain(self.id, false, f))
            .map(|_| self)
    }
}

impl<T: PartialEq + 'static> ReactiveValue for MemoChain<T> {
    type Value = T;

    fn is_alive(&self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }

    unsafe fn dispose(self) {
        with_current_runtime(|rt| rt.dispose(self.id))
    }
}

impl<T: PartialEq + 'static> SignalMap<T> for MemoChain<T> {
    type Output<U: PartialEq + 'static> = Memo<U>;

    #[track_caller]
    fn map<U: PartialEq + 'static>(
        &self,
        mut map: impl FnMut(&T) -> U + 'static,
    ) -> Self::Output<U> {
        let this = *self;
        create_memo(move || this.with(&mut map))
    }
}

impl<T: PartialEq + 'static> ReadSignal<T> for MemoChain<T> {
    #[track_caller]
    fn track(&self) {
        with_current_runtime(|rt| self.id.subscribe(rt))
    }

    #[track_caller]
    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        let caller = Location::caller();
        with_current_runtime(|rt| {
            self.id.with_untracked(
                rt,
                |memoized: &Option<T>| {
                    f(memoized.as_ref().expect("Must already been set"))
                },
                caller,
            )
        })
    }
}

impl<T: PartialEq> Clone for MemoChain<T> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), ty: self.ty.clone() }
    }
}

impl<T: PartialEq> Copy for MemoChain<T> {}

impl<T: PartialEq + Clone + 'static> IntoMemo<T> for MemoChain<T> {
    fn memo(self) -> Memo<T> {
        self.map(|value| value.clone())
    }
}

pub trait IntoMemoChain<T: PartialEq> {
    fn memo_chain(self) -> MemoChain<T>;
}

impl<T: PartialEq> IntoMemoChain<T> for MemoChain<T> {
    fn memo_chain(self) -> MemoChain<T> {
        self
    }
}

impl<T: PartialEq + Clone + 'static> IntoMemoChain<T> for T {
    #[track_caller]
    fn memo_chain(self) -> MemoChain<T> {
        create_memo_chain(move || self.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn math_precedence() {
        {
            // Must be (2 + 3) * 2, not 2 * 2 + 3
            let memo = create_memo_chain(|| 2)
                .last(|value| value * 2)
                .unwrap()
                .first(|value| value + 3)
                .unwrap();

            assert_eq!(memo.get(), 10);
        }

        {
            // Same expression but with order as it is
            let memo = create_memo_chain(|| 2)
                .first(|value| value * 2)
                .unwrap()
                .last(|value| value + 3)
                .unwrap();

            assert_eq!(memo.get(), 7);
        }
    }

    // // Just some ideas to get rid of MemoChain which only usage is rsact_ui styles
    // #[test]
    // fn replace_memo_chain_with_memos() {
    //     #[derive(Default, PartialEq)]
    //     struct S {
    //         foo: i32,
    //         bar: i32,
    //     }

    //     let base = move |mut s: S| {
    //         s.foo = 666;
    //         s
    //     };

    //     struct Widget {
    //         style: Memo<S>,
    //     }

    //     let widget = Widget {
    //         // User-defined style initialized first, but must be chained after `base`
    //         style: create_memo(move || {
    //             s.bar = 123;
    //             s
    //         },
    //     };
    // }
}
