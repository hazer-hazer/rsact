use crate::{
    callback::AnyCallback, effect::EffectOrder, memo::Memo,
    runtime::with_current_runtime, signal::ReadSignal, storage::ValueId,
};
use alloc::rc::Rc;
use core::{
    any::Any,
    cell::{RefCell, RefMut},
    marker::PhantomData,
};

pub trait MemoChainCallback {
    fn run(&self, value: &dyn Any) -> T;
}

pub trait MemoChainImpl: AnyCallback {
    fn bind(&self, order: EffectOrder, cb: dyn MemoChainCallback);
}

pub type MemoChainCallbackList<T> = Rc<RefCell<Vec<Rc<dyn Fn(&mut T)>>>>;

#[derive(Clone)]
pub struct StoredMemoChain<T> {
    pub(crate) initial: Rc<dyn Fn() -> T>,
    pub(crate) first: MemoChainCallbackList<T>,
    pub(crate) normal: MemoChainCallbackList<T>,
    pub(crate) last: MemoChainCallbackList<T>,
}

impl<T: PartialEq + 'static> StoredMemoChain<T> {
    pub fn new(initial: impl Fn() -> T + 'static) -> Self {
        Self {
            initial: Rc::new(initial),
            first: Default::default(),
            normal: Default::default(),
            last: Default::default(),
        }
    }

    fn order(&self, order: EffectOrder) -> RefMut<'_, Vec<Rc<dyn Fn(&mut T)>>> {
        match order {
            EffectOrder::First => self.first.borrow_mut(),
            EffectOrder::Normal => self.normal.borrow_mut(),
            EffectOrder::Last => self.last.borrow_mut(),
        }
    }

    pub(crate) fn add(
        &self,
        order: EffectOrder,
        cb: impl Fn(&mut T) + 'static,
    ) {
        self.order(order).push(Rc::new(cb))
    }

    pub fn run(&self, value: Rc<RefCell<dyn Any>>) -> bool {
        let mut new_value = (self.initial)();

        EffectOrder::each().for_each(|order| {
            self.order(order).iter().for_each(|cb| cb(&mut new_value))
        });

        let changed = {
            PartialEq::ne(
                value
                    .borrow()
                    .downcast_ref::<Option<T>>()
                    .unwrap()
                    .as_ref()
                    .unwrap(),
                &new_value,
            )
        };

        if changed {
            value
                .borrow_mut()
                .downcast_mut::<Option<T>>()
                .unwrap()
                .replace(new_value);
        }

        changed
    }
}

// pub struct MemoChainCallback<T, F>
// where
//     F: Fn(&T) -> T,
// {
//     pub(crate) f: F,
//     ty: PhantomData<T>,
// }

// impl<T, F> MemoChainCallback<T, F>
// where
//     F: Fn(&T) -> T,
// {
//     pub fn new(f: F) -> Self {
//         Self { f, ty: PhantomData }
//     }
// }

// impl<T> AnyCallback for MemoChainCallbacks<T>
// where
//     T: PartialEq + 'static,
// {
//     fn run(&self, value: Rc<RefCell<dyn Any>>) -> bool {
//         let mut new_value = (self.initial)();
//         MemoChainCallbacks::run(self, &mut new_value);

//         let changed = {
//             PartialEq::ne(
//                 value
//                     .borrow()
//                     .downcast_ref::<Option<T>>()
//                     .unwrap()
//                     .as_ref()
//                     .unwrap(),
//                 &new_value,
//             )
//         };

//         if changed {
//             value
//                 .borrow_mut()
//                 .downcast_mut::<Option<T>>()
//                 .unwrap()
//                 .replace(new_value);
//         }

//         changed
//     }
// }

pub struct MemoChain<T: PartialEq> {
    id: ValueId,
    ty: PhantomData<T>,
}

impl<T: PartialEq + 'static> MemoChain<T> {
    pub fn new(f: impl Fn() -> T + 'static) -> Self {
        Self {
            id: with_current_runtime(|rt| rt.storage.create_memo_chain(f)),
            ty: PhantomData,
        }
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

pub fn use_memo_chain<T: PartialEq + 'static>(
    f: impl Fn() -> T + 'static,
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

impl<T: PartialEq + Clone + 'static> IntoMemoChain<T> for T {
    fn into_memo_chain(self) -> MemoChain<T> {
        use_memo_chain(move || self.clone())
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
            let memo = use_memo_chain(|| 2)
                .then(|value| value * 2)
                .first(|value| value + 3);

            assert_eq!(memo.get(), 10);
        }

        {
            // Same expression but with order as it is
            let memo = use_memo_chain(|| 2)
                .then(|value| value * 2)
                .then(|value| value + 3);

            assert_eq!(memo.get(), 7);
        }
    }
}
