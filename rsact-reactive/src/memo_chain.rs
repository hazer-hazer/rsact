use crate::{
    callback::AnyCallback,
    effect::EffectOrder,
    memo::Memo,
    prelude::create_memo,
    runtime::with_current_runtime,
    signal::{ReadSignal, SignalMapper, SignalValue},
    storage::ValueId,
};
use alloc::rc::Rc;
use core::{
    any::Any,
    cell::RefCell,
    fmt::{Debug, Pointer},
    marker::PhantomData,
    panic::Location,
};

pub fn use_memo_chain<T: PartialEq + 'static>(
    f: impl Fn(Option<&T>) -> T + 'static,
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

impl<T: PartialEq + 'static> SignalValue for MemoChain<T> {
    type Value = T;
}

impl<T: PartialEq + 'static> SignalMapper<T> for MemoChain<T> {
    type Output<U: PartialEq + 'static> = Memo<U>;

    fn mapped<U: PartialEq + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Self::Output<U> {
        let this = *self;
        create_memo(move |_| this.with(&map))
    }
}

impl<T: Debug + PartialEq + 'static> Debug for MemoChain<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.with(|value| value.fmt(f))
    }
}

impl<T: PartialEq + 'static> MemoChain<T> {
    #[track_caller]
    pub fn new(f: impl Fn(Option<&T>) -> T + 'static) -> Self {
        let caller = Location::caller();
        Self {
            id: with_current_runtime(|rt| rt.create_memo_chain(f, caller)),
            ty: PhantomData,
        }
    }

    pub fn is_alive(self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }

    pub fn chain(
        self,
        order: EffectOrder,
        map: impl Fn(&T) -> T + 'static,
    ) -> Self {
        with_current_runtime(|rt| rt.add_memo_chain(self.id, order, map));
        self
    }

    pub fn then(self, map: impl Fn(&T) -> T + 'static) -> Self {
        self.chain(EffectOrder::Normal, map)
    }

    pub fn first(self, map: impl Fn(&T) -> T + 'static) -> Self {
        self.chain(EffectOrder::First, map)
    }

    pub fn last(self, map: impl Fn(&T) -> T + 'static) -> Self {
        self.chain(EffectOrder::Last, map)
    }
}

impl<T: PartialEq + 'static> ReadSignal<T> for MemoChain<T> {
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

pub trait IntoMemoChain<T: PartialEq> {
    fn into_memo_chain(self) -> MemoChain<T>;
}

impl<T: PartialEq> IntoMemoChain<T> for MemoChain<T> {
    fn into_memo_chain(self) -> MemoChain<T> {
        self
    }
}

impl<T: PartialEq + Clone + 'static> IntoMemoChain<T> for T {
    fn into_memo_chain(self) -> MemoChain<T> {
        use_memo_chain(move |_| self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::use_memo_chain;
    use crate::signal::ReadSignal;

    #[test]
    fn math_precedence() {
        {
            // Must be (2 + 3) * 2, not 2 * 2 + 3
            let memo = use_memo_chain(|_| 2)
                .then(|value| value * 2)
                .first(|value| value + 3);

            assert_eq!(memo.get(), 10);
        }

        {
            // Same expression but with order as it is
            let memo = use_memo_chain(|_| 2)
                .then(|value| value * 2)
                .then(|value| value + 3);

            assert_eq!(memo.get(), 7);
        }
    }
}
