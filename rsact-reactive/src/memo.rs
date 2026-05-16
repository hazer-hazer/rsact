use crate::{
    ReactiveValue,
    callback::{AnyCallback, CallbackFn},
    inert::Inert,
    read::{ReadSignal, SignalMap, impl_read_signal_traits},
    runtime::with_current_runtime,
    signal::Signal,
    storage::ValueId,
};
use alloc::{rc::Rc, vec::Vec};
use core::{cell::RefCell, marker::PhantomData, ops::Deref, panic::Location};

/// Create a new [`Memo<T>`] in the current runtime scope.
///
/// The closure `f` is run immediately and then re-run whenever any reactive
/// value accessed inside it changes. The new value is compared with the
/// previous one using `PartialEq`; if equal, downstream subscribers are
/// **not** notified (memoization / glitch-free propagation).
///
/// # Example
///
/// ```rust
/// # use rsact_reactive::prelude::*;
/// # use rsact_reactive::runtime::with_new_runtime;
/// # with_new_runtime(|_| {
/// let mut sig = create_signal(2u32);
/// let squared = create_memo(move || sig.get() * sig.get());
/// assert_eq!(squared.get(), 4);
/// sig.set(3);
/// assert_eq!(squared.get(), 9);
/// # });
/// ```
#[track_caller]
pub fn create_memo<T, P: 'static>(f: impl CallbackFn<T, P> + 'static) -> Memo<T>
where
    T: PartialEq + 'static,
{
    Memo::new(f)
}

pub(crate) struct MemoCallback<T, F, P>
where
    F: CallbackFn<T, P>,
{
    pub f: F,
    pub ty: PhantomData<T>,
    pub p: PhantomData<P>,
}

impl<T, F, P> AnyCallback for MemoCallback<T, F, P>
where
    F: CallbackFn<T, P>,
    T: PartialEq + 'static,
{
    #[track_caller]
    fn run(&mut self, value: Rc<RefCell<dyn core::any::Any>>) -> bool {
        let (new_value, changed) = {
            let value = value.borrow();
            let value = value.downcast_ref::<Option<T>>().unwrap().as_ref();

            let new_value = self.f.run(value);
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

/// A derived reactive value that caches its result until its dependencies change.
///
/// A `Memo<T>` is either:
/// - `Memo::Memo` — a proper memoized computation created by [`create_memo`].
/// - `Memo::Signal` — a read-only view of a [`Signal<T>`] stored without an
///   extra runtime node (zero-overhead identity wrap via [`IntoMemo`]).
///
/// `Memo` re-runs its closure only when a source signal or memo it reads
/// has changed.  The result is compared with `PartialEq`; if unchanged,
/// subscribers of the memo are not notified, cutting off unnecessary
/// re-computation downstream.
///
/// `Memo<T>` is `Copy` (it is a handle, not an owner).
///
/// # Glitch-freedom
///
/// The runtime topologically sorts pending memos before flushing effects, so
/// a memo is always recomputed at most once per reactive update cycle and
/// effects never observe a stale intermediate value.
pub enum Memo<T: ?Sized + PartialEq> {
    Inert(Inert<T>),
    Memo {
        id: ValueId,
        ty: PhantomData<T>,
    },
    /// Identity-mapped signal as memo. Stored in memo as is to avoid creation of new memos for signals mapped as readonly identity values.
    Signal(Signal<T, crate::signal::marker::ReadOnly>),
}

impl_read_signal_traits!(Memo<T>: PartialEq);

impl<T: PartialEq + 'static> Memo<T> {
    #[track_caller]
    pub fn new<P: 'static>(f: impl CallbackFn<T, P> + 'static) -> Self {
        let caller = Location::caller();
        Self::Memo {
            id: with_current_runtime(|rt| rt.create_memo(f, caller)),
            ty: PhantomData,
        }
    }

    // TODO: As a simplification and replacement of MemoChain
    // pub fn after_map(&mut self, f: impl FnMut(&T) -> T + 'static) -> Self {
    //     // Replace old callback with new one. Now, passed callback is called first. [`replace_callback`] removes all subs and sources
    //     // let old_callback = runtime.replace_callback(self.id, f);
    //     // create_memo(move || )
    // }
}

impl<T: PartialEq + 'static> Clone for Memo<T> {
    fn clone(&self) -> Self {
        match self {
            &Memo::Memo { id, ty } => Self::Memo { id, ty },
            &Memo::Signal(signal) => Memo::Signal(signal),
            &Memo::Inert(inert) => Memo::Inert(inert),
        }
    }
}

impl<T: PartialEq + 'static> Copy for Memo<T> {}

impl<T: PartialEq + 'static> ReactiveValue for Memo<T> {
    type Value = T;

    fn id(&self) -> Option<ValueId> {
        match self {
            Memo::Memo { id, ty: _ } => Some(*id),
            Memo::Signal(signal) => signal.id(),
            Memo::Inert(inert) => inert.id(),
        }
    }

    fn is_alive(&self) -> bool {
        match self {
            &Memo::Memo { id, ty: _ } => {
                with_current_runtime(|rt| rt.is_alive(id))
            },
            Memo::Signal(signal) => signal.is_alive(),
            Memo::Inert(inert) => inert.is_alive(),
        }
    }

    unsafe fn dispose(self) {
        match self {
            Memo::Memo { id, ty: _ } => {
                with_current_runtime(|rt| rt.dispose(id))
            },
            Memo::Signal(signal) => signal.dispose(),
            Memo::Inert(inert) => unsafe { inert.dispose() },
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
            Memo::Inert(inert) => inert.track(),
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
            Memo::Inert(inert) => inert.with_untracked(f),
        }
    }
}

impl<T: PartialEq + 'static, U: PartialEq + 'static> SignalMap<T, U>
    for Memo<T>
{
    type Output = Memo<U>;

    #[track_caller]
    fn map(&self, mut map: impl FnMut(&T) -> U + 'static) -> Self::Output {
        let this = *self;
        create_memo(move || this.with(&mut map))
    }
}

/// Convert a value into a [`Memo<T>`].
///
/// Implemented for:
/// - `Memo<T>` — identity.
/// - `Signal<T>` — zero-cost wrap as `Memo::Signal` (no new node allocated).
/// - `Fn() -> T` — wraps the closure in [`create_memo`].
/// - `Inert<T: Clone>` — wraps in a constant memo (allocates one node).
/// - `MaybeReactive<T: Clone>` — converts each variant appropriately.
/// - `MemoChain<T: Clone>` — maps via identity clone.
pub trait IntoMemo<T: PartialEq> {
    fn memo(self) -> Memo<T>;
}

impl<T: PartialEq + 'static> IntoMemo<T> for Memo<T> {
    /// Should never be called directly being redundant
    fn memo(self) -> Memo<T> {
        self
    }
}

impl<T: PartialEq + Clone + 'static, F> IntoMemo<T> for F
where
    F: Fn() -> T + 'static,
{
    #[track_caller]
    fn memo(self) -> Memo<T> {
        create_memo(move || (self)())
    }
}

/// A tree of reactive values where both the node data and children list are
/// memoized.
///
/// Used by `rsact-ui` to represent widget layout trees: the `data` memo
/// holds the node's value and `children` holds the reactive child list.  
/// Reads on any part of the tree are tracked normally; the tree only
/// re-evaluates subtrees whose sources changed.
///
/// Construct leaf nodes with [`MemoTree::childless`]. The `data` and
/// `children` fields are public so you can build arbitrary tree shapes.
#[derive(PartialEq)]
pub struct MemoTree<T: PartialEq + 'static> {
    pub data: Memo<T>,
    pub children: Memo<Vec<MemoTree<T>>>,
}

impl<T: PartialEq + Default + 'static> Default for MemoTree<T> {
    #[track_caller]
    fn default() -> Self {
        Self { data: create_memo(T::default), children: create_memo(Vec::new) }
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
        Self { data: data.memo(), children: create_memo(Vec::new) }
    }

    // pub fn fold<A>(&self, acc: A, mut f: impl FnMut(A, &T) -> A) -> A {
    //     let acc = self.data.with(|data| f(acc, data));

    //     self.children.with(move |children| {
    //         children.iter().fold(acc, |acc, child| child.fold(acc, f))
    //     })
    // }

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

        let runs = create_memo(move |runs: Option<&i32>| {
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

        let runs = create_memo(move |runs: Option<&i32>| {
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

    //     let memo = use_memo(move || 1);
    //     maybe_memo(memo);
    // }
}
