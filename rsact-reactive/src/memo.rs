use crate::{
    callback::AnyCallback,
    read::{impl_read_signal_traits, ReadSignal, SignalMap},
    runtime::with_current_runtime,
    signal::{marker, Signal},
    storage::ValueId,
    ReactiveValue,
};
use alloc::{rc::Rc, vec::Vec};
use core::{cell::RefCell, marker::PhantomData, ops::Deref, panic::Location};

#[track_caller]
pub fn create_memo<T: PartialEq + 'static>(
    f: impl FnMut(Option<&T>) -> T + 'static,
) -> Memo<T> {
    Memo::new(f)
}

pub struct MemoCallback<T, F>
where
    F: FnMut(Option<&T>) -> T,
{
    pub(crate) f: F,
    pub(crate) ty: PhantomData<T>,
}

impl<T, F> AnyCallback for MemoCallback<T, F>
where
    F: FnMut(Option<&T>) -> T,
    T: PartialEq + 'static,
{
    #[track_caller]
    fn run(&mut self, value: Rc<RefCell<dyn core::any::Any>>) -> bool {
        let (new_value, changed) = {
            let value = value.borrow();
            let value = value.downcast_ref::<Option<T>>().unwrap().as_ref();

            let new_value = (self.f)(value);
            let changed = Some(&new_value) != value;
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

pub struct Memo<T: PartialEq> {
    id: ValueId,
    ty: PhantomData<T>,
}

impl_read_signal_traits!(Memo<T>: PartialEq);

impl<T: PartialEq + 'static> Memo<T> {
    #[track_caller]
    pub fn new(f: impl FnMut(Option<&T>) -> T + 'static) -> Self {
        let caller = Location::caller();
        Self {
            id: with_current_runtime(|rt| rt.create_memo(f, caller)),
            ty: PhantomData,
        }
    }

    // TODO: As a simplification and replacement of MemoChain
    // pub fn after_map(&mut self, f: impl FnMut(&T) -> T + 'static) -> Self {
    //     // Replace old callback with new one. Now, passed callback is called first. [`replace_callback`] removes all subs and sources
    //     // let old_callback = runtime.replace_callback(self.id, f);
    //     // create_memo(move |_| )
    // }
}

impl<T: PartialEq> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), ty: self.ty.clone() }
    }
}

impl<T: PartialEq> Copy for Memo<T> {}

impl<T: PartialEq + 'static> ReactiveValue for Memo<T> {
    type Value = T;

    fn is_alive(&self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }

    fn dispose(self) {
        with_current_runtime(|rt| rt.dispose(self.id))
    }
}

impl<T: PartialEq + 'static> ReadSignal<T> for Memo<T> {
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

impl<T: PartialEq + 'static> SignalMap<T> for Memo<T> {
    type Output<U: PartialEq + 'static> = Memo<U>;

    #[track_caller]
    fn map<U: PartialEq + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Self::Output<U> {
        let this = *self;
        create_memo(move |_| this.with(&map))
    }
}

pub trait IntoMemo<T: PartialEq> {
    fn memo(self) -> Memo<T>;
}

impl<T: PartialEq + Clone + 'static, M: marker::CanRead + 'static> IntoMemo<T>
    for Signal<T, M>
{
    #[track_caller]
    fn memo(self) -> Memo<T> {
        create_memo(move |_| self.get_cloned())
    }
}

impl<T: PartialEq + Clone + 'static, F> IntoMemo<T> for F
where
    F: Fn() -> T + 'static,
{
    #[track_caller]
    fn memo(self) -> Memo<T> {
        create_memo(move |_| (self)())
    }
}

impl<T: PartialEq + Clone + 'static> IntoMemo<T> for Memo<T> {
    /// Should never be called directly being redundant
    fn memo(self) -> Memo<T> {
        self
    }
}

#[derive(PartialEq)]
pub struct MemoTree<T: PartialEq + 'static> {
    pub data: Memo<T>,
    pub children: Memo<Vec<MemoTree<T>>>,
}

impl<T: PartialEq + Default + 'static> Default for MemoTree<T> {
    #[track_caller]
    fn default() -> Self {
        Self {
            data: create_memo(|_| T::default()),
            children: create_memo(|_| Vec::new()),
        }
    }
}

impl<T: PartialEq + 'static> Clone for MemoTree<T> {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), children: self.children.clone() }
    }
}

impl<T: PartialEq + 'static> Copy for MemoTree<T> {}

impl<T: PartialEq + 'static> MemoTree<T> {
    #[track_caller]
    pub fn childless(data: impl IntoMemo<T>) -> Self {
        Self { data: data.memo(), children: create_memo(|_| alloc::vec![]) }
    }

    // pub fn fold<A>(&self, acc: A, mut f: impl FnMut(A, &T) -> A) -> A {
    //     let acc = self.data.with(|data| f(acc, data));

    //     self.children.with(move |children| {
    //         children.iter().fold(acc, |acc, child| child.fold(acc, f))
    //     })
    // }

    #[track_caller]
    pub fn flat_collect(&self) -> Vec<Memo<T>> {
        self.children.with(|children| {
            core::iter::once(self.data)
                .chain(children.iter().map(MemoTree::flat_collect).flatten())
                .collect()
        })
    }
}

/// [`Keyed`] is a helper for memos which you can use to avoid computationally expensive comparisons in some cases. It is a pair of data and its key, where, unlike in raw memo, key is used for memoization comparisons.
pub struct Keyed<K: PartialEq, V> {
    key: K,
    value: V,
}

impl<K: PartialEq, V> Deref for Keyed<K, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<K: PartialEq, V> PartialEq for Keyed<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn single_run() {
        let mut signal = create_signal(1);

        let runs = create_memo(move |runs| {
            signal.get();

            runs.unwrap_or(&0) + 1
        });

        signal.set(1);
        signal.set(1);
        signal.set(1);

        assert_eq!(runs.get(), 1);
    }

    #[test]
    fn exact_runs_count() {
        let mut signal = create_signal(1);

        let runs = create_memo(move |runs| {
            signal.get();

            runs.unwrap_or(&0) + 1
        });

        signal.set(1);
        runs.get();

        signal.set(2);
        runs.get();

        signal.set(3);
        runs.get();

        assert_eq!(runs.get(), 3);
    }

    // No longer works
    // #[test]
    // fn shortcut_typing() {
    //     fn maybe_memo<T: PartialEq>(var: impl AsMemo<T>) {}

    //     let var = 1;
    //     maybe_memo(var);

    //     let memo = use_memo(move |_| 1);
    //     maybe_memo(memo);
    // }
}
