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

// TODO: FnMut in memos is a bad idea!
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

/**
 * TODO: Possible optimization is to make Memo an enum of a real memo and signal identity map.
 * Sometimes, Memo is used as read-only signal lens, but it creates additional memo, which is just `signal.map(|value| value)`, i.e. `create_memo(|_| signal.get_cloned())`. Memo isn't required here, just store the signal as read-only value!
 *
 * Or better introduce `ReadSignal` (not a trait) which is an enum of readable signals, but this is very similar to `MaybeReactive`...
 */

pub enum Memo<T: PartialEq> {
    Memo {
        id: ValueId,
        ty: PhantomData<T>,
    },
    /// Identity-mapped signal as memo. Stored in memo as is to avoid creation of new memo for signals mapped as readonly identity values.
    Signal(Signal<T>),
}

impl_read_signal_traits!(Memo<T>: PartialEq);

impl<T: PartialEq + 'static> Memo<T> {
    #[track_caller]
    pub fn new(f: impl FnMut(Option<&T>) -> T + 'static) -> Self {
        let caller = Location::caller();
        Self::Memo {
            id: with_current_runtime(|rt| rt.create_memo(f, caller)),
            ty: PhantomData,
        }
    }

    pub fn id(&self) -> ValueId {
        match self {
            Memo::Memo { id, ty } => *id,
            Memo::Signal(signal) => signal.id(),
        }
    }

    // TODO: As a simplification and replacement of MemoChain
    // pub fn after_map(&mut self, f: impl FnMut(&T) -> T + 'static) -> Self {
    //     // Replace old callback with new one. Now, passed callback is called first. [`replace_callback`] removes all subs and sources
    //     // let old_callback = runtime.replace_callback(self.id, f);
    //     // create_memo(move |_| )
    // }
}

impl<T: PartialEq + 'static> Clone for Memo<T> {
    fn clone(&self) -> Self {
        match self {
            &Memo::Memo { id, ty } => Self::Memo { id, ty },
            &Memo::Signal(signal) => Memo::Signal(signal),
        }
    }
}

impl<T: PartialEq + 'static> Copy for Memo<T> {}

impl<T: PartialEq + 'static> ReactiveValue for Memo<T> {
    type Value = T;

    fn is_alive(&self) -> bool {
        match self {
            &Memo::Memo { id, ty: _ } => {
                with_current_runtime(|rt| rt.is_alive(id))
            },
            Memo::Signal(signal) => signal.is_alive(),
        }
    }

    fn dispose(self) {
        match self {
            Memo::Memo { id, ty: _ } => {
                with_current_runtime(|rt| rt.dispose(id))
            },
            Memo::Signal(signal) => signal.dispose(),
        }
    }
}

impl<T: PartialEq + 'static> ReadSignal<T> for Memo<T> {
    #[track_caller]
    fn track(&self) {
        match self {
            Memo::Memo { id, ty: _ } => {
                with_current_runtime(|rt| id.subscribe(rt))
            },
            Memo::Signal(signal) => signal.track(),
        }
    }

    #[track_caller]
    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        match self {
            &Memo::Memo { id, ty: _ } => {
                let caller = Location::caller();
                with_current_runtime(|rt| {
                    id.with_untracked(
                        rt,
                        |memoized: &Option<T>| {
                            f(memoized.as_ref().expect("Must already been set"))
                        },
                        caller,
                    )
                })
            },
            Memo::Signal(signal) => signal.with_untracked(f),
        }
    }
}

impl<T: PartialEq + 'static> SignalMap<T> for Memo<T> {
    type Output<U: PartialEq + 'static> = Memo<U>;

    #[track_caller]
    fn map<U: PartialEq + 'static>(
        &self,
        mut map: impl FnMut(&T) -> U + 'static,
    ) -> Self::Output<U> {
        let this = *self;
        create_memo(move |_| this.with(&mut map))
    }
}

pub trait IntoMemo<T: PartialEq> {
    fn memo(self) -> Memo<T>;
}

// TODO: Optimize identity memos. Memo should allow storing signal as it is, without creation of new Memo value.
impl<T: PartialEq + 'static> IntoMemo<T> for Signal<T> {
    /// Converting Signal to Memo is cheap, and does not actually create new memo instance!
    #[track_caller]
    fn memo(self) -> Memo<T> {
        Memo::Signal(self)
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

impl<T: PartialEq + 'static> IntoMemo<T> for Memo<T> {
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

#[derive(Clone, Copy, Debug)]
pub struct NeverEqual;

impl PartialEq for NeverEqual {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

/// [`Keyed`] is a helper for memos which you can use to avoid computationally expensive comparisons in some cases. It is a pair of data and its key, where, unlike in raw memo, key is used for memoization comparisons.
#[derive(Debug)]
pub struct Keyed<K: PartialEq, V> {
    key: K,
    value: V,
}

impl<K: PartialEq, V> Keyed<K, V> {
    pub fn new(key: K, value: V) -> Self {
        Self { key, value }
    }

    pub fn value(&self) -> &V {
        &self.value
    }

    pub fn key(&self) -> &K {
        &self.key
    }
}

impl<V> Keyed<NeverEqual, V> {
    pub fn never_equal(value: V) -> Self {
        Self { key: NeverEqual, value }
    }
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

    #[test]
    fn signal_into_memo() {
        let mut signal = create_signal(1);

        let memo = signal.memo();

        assert_eq!(memo.get(), 1);

        signal.set(2);

        assert_eq!(memo.get(), 2);
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
