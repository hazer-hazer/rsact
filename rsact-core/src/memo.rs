use crate::{
    callback::AnyCallback,
    prelude::use_memo,
    runtime::with_current_runtime,
    signal::{marker, MaybeSignal, ReadSignal, Signal},
    storage::ValueId,
};
use alloc::{rc::Rc, vec::Vec};
use core::{cell::RefCell, marker::PhantomData};

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

impl<T: PartialEq> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), ty: self.ty.clone() }
    }
}

impl<T: PartialEq> Copy for Memo<T> {}

impl<T: PartialEq + 'static> Memo<T> {
    pub fn new(f: impl Fn(Option<&T>) -> T + 'static) -> Self {
        Self {
            id: with_current_runtime(|rt| rt.storage.create_memo(f)),
            ty: PhantomData,
        }
    }
}

impl<T: PartialEq> PartialEq for Memo<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: PartialEq + 'static> ReadSignal<T> for Memo<T> {
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

impl<T: PartialEq + 'static> MaybeSignal<T> for Memo<T> {
    type S = Memo<T>;

    fn maybe_signal(self) -> Self::S {
        self
    }
}

pub trait IntoMemo<T: PartialEq> {
    fn into_memo(self) -> Memo<T>;
}

impl<T: PartialEq + Clone + 'static, M: marker::CanRead + 'static> IntoMemo<T>
    for Signal<T, M>
{
    fn into_memo(self) -> Memo<T> {
        use_memo(move |_| self.get_cloned())
    }
}

// impl<T: PartialEq + Clone + 'static, F> IntoMemo<T> for F
// where
//     F: Fn() -> T + 'static,
// {
//     fn into_memo(self) -> Memo<T> {
//         use_memo(move |_| (self)())
//     }
// }

impl<T: PartialEq + Clone + 'static> IntoMemo<T> for T {
    fn into_memo(self) -> Memo<T> {
        use_memo(move |_| self.clone())
    }
}

impl<T: PartialEq + Clone + 'static> IntoMemo<T> for Memo<T> {
    fn into_memo(self) -> Memo<T> {
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct MemoTree<T: PartialEq + 'static> {
    pub data: Memo<T>,
    pub children: Memo<Vec<MemoTree<T>>>,
}

impl<T: PartialEq + 'static> MemoTree<T> {
    pub fn childless(data: Memo<T>) -> Self {
        Self { data, children: use_memo(|_| alloc::vec![]) }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        prelude::{use_memo, use_signal},
        signal::{ReadSignal, WriteSignal},
    };

    #[test]
    fn single_run() {
        let signal = use_signal(1);

        let runs = use_memo(move |runs| {
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
        let signal = use_signal(1);

        let runs = use_memo(move |runs| {
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
}
