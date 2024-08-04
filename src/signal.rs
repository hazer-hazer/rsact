use core::marker::PhantomData;

use crate::{runtime::with_current_runtime, storage::ValueId};

pub struct Signal<T: 'static> {
    id: ValueId,
    ty: PhantomData<T>,
}

impl<T: 'static> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for Signal<T> {}

impl<T: Send + 'static> Signal<T> {
    pub fn new(value: T) -> Self {
        Self {
            id: with_current_runtime(|runtime| runtime.storage.create_signal(value)),
            ty: PhantomData,
        }
    }

    pub fn get(&self) -> T
    where
        T: Copy,
    {
        // TODO: Add get-copy method for ValueId to remove useless closure?
        self.with(|val| *val)
        // self.with(T::clone)
    }

    pub fn get_cloned(&self) -> T
    where
        T: Clone,
    {
        self.with(T::clone)
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        with_current_runtime(|runtime| self.id.with(runtime, f))
    }

    pub fn set(&self, new: T) {
        self.update(|val| *val = new)
    }

    pub fn update<U>(&self, f: impl FnOnce(&mut T) -> U) -> U {
        with_current_runtime(|runtime| self.id.update(runtime, f))
    }
}

pub fn create_signal<T: 'static + Send>(value: T) -> Signal<T> {
    Signal::new(value)
}
