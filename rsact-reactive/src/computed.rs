/*!
 * Computed is a lens to a Signal.
 * It is needed to avoid making every part of a structure stored in signal to be reactive.
 * By setting computed, some signal is updated by passed parameters (or without any).
 * By getting computed, signal data is retrieved and possibly mapped.
 */

// TODO: Static setter/getter?

// pub enum SignalGetter<G: PartialEq> {
//     Signal(Signal<G>),
//     Memo(Memo<G>),
//     Derived(Box<dyn Fn()>),
// }

// pub enum SignalSetter<S> {
//     Signal(Signal<S>),
//     // TODO: Use StoredValue
//     Map(Box<dyn Fn(S)>),
// }

// impl<S: 'static> SignalSetter<S> {
//     pub fn signal(signal: Signal<S>) -> Self {
//         Self::Signal(signal)
//     }

//     pub fn map(f: impl Fn(S) + 'static) -> Self {
//         Self::Map(Box::new(f))
//     }
// }

// pub struct Computed<G: PartialEq, S> {
//     getter: SignalGetter<G>,
//     setter: SignalSetter<S>,
// }

use core::{marker::PhantomData, panic::Location};

use crate::{
    ReactiveValue,
    callback::{AnyCallback, CallbackFn},
    memo::{Memo, create_memo},
    read::{ReadSignal, SignalMap},
    runtime::with_current_runtime,
    storage::ValueId,
};

pub(crate) struct ComputedCallback<T, F, P>
where
    F: CallbackFn<T, P>,
{
    pub f: F,
    pub ty: PhantomData<T>,
    pub p: PhantomData<P>,
}

impl<T: 'static, F, P> AnyCallback for ComputedCallback<T, F, P>
where
    F: CallbackFn<T, P>,
{
    fn run(
        &mut self,
        value: alloc::rc::Rc<core::cell::RefCell<dyn core::any::Any>>,
    ) -> bool {
        let new_value = {
            let value = value.borrow();
            let value = value.downcast_ref::<Option<T>>().unwrap().as_ref();

            let new_value = self.f.run(value);
            new_value
        };

        let mut value = value.borrow_mut();
        let value = value.downcast_mut::<Option<T>>().unwrap();
        value.replace(new_value);

        true
    }
}

pub struct Computed<T> {
    id: ValueId,
    ty: PhantomData<T>,
}

impl<T: PartialEq + 'static> SignalMap<T> for Computed<T> {
    type Output<U: PartialEq + 'static> = Memo<U>;

    fn map<U: PartialEq + 'static>(
        &self,
        mut map: impl FnMut(&T) -> U + 'static,
    ) -> Self::Output<U> {
        let this = *self;
        create_memo(move || this.with(&mut map))
    }
}

impl<T: 'static> ReactiveValue for Computed<T> {
    type Value = T;

    fn is_alive(&self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }

    unsafe fn dispose(self) {
        with_current_runtime(|rt| rt.dispose(self.id))
    }
}

impl<T: 'static> Computed<T> {
    #[track_caller]
    pub fn new(f: impl FnMut(Option<&T>) -> T + 'static) -> Self {
        let caller = Location::caller();
        Self {
            id: with_current_runtime(|rt| rt.create_computed(f, caller)),
            ty: PhantomData,
        }
    }

    pub fn id(&self) -> ValueId {
        self.id
    }
}

impl<T: 'static> ReadSignal<T> for Computed<T> {
    #[track_caller]
    fn track(&self) {
        with_current_runtime(|rt| {
            self.id.subscribe(rt);
        })
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

impl<T> Clone for Computed<T> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), ty: self.ty.clone() }
    }
}

impl<T> Copy for Computed<T> {}
