use crate::{
    callback::AnyCallback, effect::EffectOrder, runtime::with_current_runtime,
    signal::ReadSignal, storage::ValueId,
};
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell, marker::PhantomData};

pub struct MemoChainCallback<T, F>
where
    F: Fn(&T) -> T,
{
    pub(crate) f: F,
    pub(crate) ty: PhantomData<T>,
}

impl<T, F> MemoChainCallback<T, F>
where
    F: Fn(&T) -> T,
{
    pub fn new(f: F) -> Self {
        Self { f, ty: PhantomData }
    }
}

// TODO: Optimize, should not set the value but accept it, change it and then
// pass to the next MemoChainCallback
impl<T, F> AnyCallback for MemoChainCallback<T, F>
where
    F: Fn(&T) -> T,
    T: PartialEq + 'static,
{
    fn run(&self, value: &mut dyn Any) -> bool {
        let (new_value, changed) = {
            // let value = value.borrow();
            let value = value
                .downcast_mut::<Option<T>>()
                .unwrap()
                .as_ref()
                .expect("Must already been set");

            let new_value = (self.f)(value);
            let changed = PartialEq::ne(value, &new_value);
            (new_value, changed)
        };

        if changed {
            // let mut value = value.borrow_mut();
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

impl<T: PartialEq + Send + 'static> MemoChain<T> {
    pub fn new(f: impl Fn(Option<&T>) -> T + Send + 'static) -> Self {
        Self {
            id: with_current_runtime(|rt| rt.storage.create_memo_chain(f)),
            ty: PhantomData,
        }
    }

    pub fn chain(
        self,
        order: EffectOrder,
        map: impl Fn(&T) -> T + Send + 'static,
    ) -> Self {
        with_current_runtime(|rt| rt.add_memo_chain(self.id, order, map));
        self
    }

    pub fn then(self, map: impl Fn(&T) -> T + Send + 'static) -> Self {
        self.chain(EffectOrder::Normal, map)
    }

    pub fn first(self, map: impl Fn(&T) -> T + Send + 'static) -> Self {
        self.chain(EffectOrder::First, map)
    }

    pub fn last(self, map: impl Fn(&T) -> T + Send + 'static) -> Self {
        self.chain(EffectOrder::Last, map)
    }
}

impl<T: PartialEq + 'static> ReadSignal<T> for MemoChain<T> {
    fn track(&self) {
        with_current_runtime(|rt| self.id.subscribe(rt))
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        with_current_runtime(|rt| {
            self.id.with_untracked(rt, |memoized: &Option<T>| {
                f(memoized.as_ref().expect("Must already been set"))
            })
        })
    }
}

impl<T: PartialEq> Clone for MemoChain<T> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), ty: self.ty.clone() }
    }
}

impl<T: PartialEq> Copy for MemoChain<T> {}

pub fn use_memo_chain<T: PartialEq + Send + 'static>(
    f: impl Fn(Option<&T>) -> T + Send + 'static,
) -> MemoChain<T> {
    MemoChain::new(f)
}

pub trait IntoMemoChain<T: PartialEq> {
    fn into_memo_chain(self) -> MemoChain<T>;
}

impl<T: PartialEq> IntoMemoChain<T> for MemoChain<T> {
    fn into_memo_chain(self) -> MemoChain<T> {
        self
    }
}

impl<T: PartialEq + Clone + Send + 'static> IntoMemoChain<T> for T {
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
