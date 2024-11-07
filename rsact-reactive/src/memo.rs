use crate::{
    callback::AnyCallback,
    prelude::create_memo,
    runtime::with_current_runtime,
    signal::{marker, MaybeSignal, ReadSignal, Signal},
    storage::ValueId,
    with,
};
use alloc::{rc::Rc, vec::Vec};
use core::{
    cell::RefCell,
    fmt::{Debug, Display},
    marker::PhantomData,
    ops::Deref,
    panic::Location,
};

pub struct MemoCallback<T, F>
where
    F: Fn(Option<&T>) -> T,
{
    pub(crate) f: F,
    pub(crate) ty: PhantomData<T>,
}

impl<T, F> AnyCallback for MemoCallback<T, F>
where
    F: Fn(Option<&T>) -> T,
    T: PartialEq + 'static,
{
    #[track_caller]
    fn run(&self, value: Rc<RefCell<dyn core::any::Any>>) -> bool {
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

impl<T: PartialEq + Display + 'static> Display for Memo<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.with(|this| write!(f, "{this}"))
    }
}

impl<T: PartialEq + Debug + 'static> Debug for Memo<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.with(|value| f.debug_tuple("Memo").field(value).finish())
    }
}

impl<T: PartialEq> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), ty: self.ty.clone() }
    }
}

impl<T: PartialEq> Copy for Memo<T> {}

impl<T: PartialEq + 'static> Memo<T> {
    #[track_caller]
    pub fn new(f: impl Fn(Option<&T>) -> T + 'static) -> Self {
        let caller = Location::caller();
        Self {
            id: with_current_runtime(|rt| rt.create_memo(f, caller)),
            ty: PhantomData,
        }
    }

    pub fn is_alive(self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }
}

impl<T: PartialEq + 'static> PartialEq for Memo<T> {
    fn eq(&self, other: &Self) -> bool {
        // self.id == other.id
        let this = self;
        with!(|this, other| this == other)
    }
}

impl<T: PartialEq + 'static> ReadSignal<T> for Memo<T> {
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

impl<T: PartialEq + 'static> MaybeSignal<T> for Memo<T> {
    type S = Memo<T>;

    fn maybe_signal(self) -> Self::S {
        self
    }
}

pub trait AsStaticMemo<T: PartialEq> {
    fn as_static_memo(self) -> Memo<T>;
}

impl<T: PartialEq + Clone + 'static> AsStaticMemo<T> for T {
    fn as_static_memo(self) -> Memo<T> {
        create_memo(move |_| self.clone())
    }
}

pub trait AsMemo<T: PartialEq> {
    fn as_memo(self) -> Memo<T>;
}

impl<T: PartialEq + Clone + 'static, M: marker::CanRead + 'static> AsMemo<T>
    for Signal<T, M>
{
    fn as_memo(self) -> Memo<T> {
        create_memo(move |_| self.get_cloned())
    }
}

impl<T: PartialEq + Clone + 'static, F> AsMemo<T> for F
where
    F: Fn() -> T + 'static,
{
    fn as_memo(self) -> Memo<T> {
        create_memo(move |_| (self)())
    }
}

// impl<T: !IsMemo + PartialEq + Clone + 'static> AsMemo<T> for T {
//     fn as_memo(self) -> Memo<T> {
//         use_memo(move |_| self.clone())
//     }
// }

impl<T: PartialEq + Clone + 'static> AsMemo<T> for Memo<T> {
    // Should never be called directly being redundant
    fn as_memo(self) -> Memo<T> {
        self
    }
}

pub struct MemoTree<T: PartialEq + 'static> {
    pub data: Memo<T>,
    pub children: Memo<Vec<MemoTree<T>>>,
}

impl<T: PartialEq + Default + 'static> Default for MemoTree<T> {
    fn default() -> Self {
        Self {
            data: create_memo(|_| T::default()),
            children: create_memo(|_| Vec::new()),
        }
    }
}

impl<T: PartialEq + 'static> Copy for MemoTree<T> {}

impl<T: PartialEq + 'static> PartialEq for MemoTree<T> {
    fn eq(&self, other: &Self) -> bool {
        let data = self.data;
        let other_data = other.data;
        let children = self.children;
        let other_children = other.children;
        with!(|data, other_data, children, other_children| {
            data == other_data && children == other_children
        })
    }
}

impl<T: PartialEq + 'static> Clone for MemoTree<T> {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), children: self.children.clone() }
    }
}

impl<T: PartialEq + 'static> MemoTree<T> {
    pub fn childless(data: impl AsMemo<T>) -> Self {
        Self { data: data.as_memo(), children: create_memo(|_| alloc::vec![]) }
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

// pub struct MemoTreeIter<'a> {
//     stack: Vec<&'a Memo2>
// }

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
    use crate::{
        memo::AsMemo,
        prelude::{create_memo, create_signal},
        signal::{ReadSignal, WriteSignal},
    };

    #[test]
    fn single_run() {
        let signal = create_signal(1);

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
        let signal = create_signal(1);

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
