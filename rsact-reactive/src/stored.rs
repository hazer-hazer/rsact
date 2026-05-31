use crate::{
    ReactiveValue,
    read::{ReadSignal, impl_read_signal_traits},
    runtime::with_current_runtime,
    storage::ValueId,
    write::WriteSignal,
};
use alloc::boxed::Box;
use core::marker::PhantomData;

pub struct StoredValue<T: 'static> {
    id: ValueId,
    ty: PhantomData<T>,
}

impl<T: 'static> StoredValue<T> {
    #[track_caller]
    pub fn new(value: T) -> Self {
        let caller = core::panic::Location::caller();
        let id = with_current_runtime(|rt| rt.create_stored(value, caller));
        Self { id, ty: PhantomData }
    }
}

impl<T: 'static> Clone for StoredValue<T> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), ty: self.ty.clone() }
    }
}

impl<T: 'static> Copy for StoredValue<T> {}

impl<T: 'static> ReactiveValue for StoredValue<T> {
    type Value = T;

    fn id(&self) -> Option<ValueId> {
        Some(self.id)
    }

    fn is_alive(&self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }

    unsafe fn dispose(self) {
        unsafe { with_current_runtime(|rt| rt.dispose(self.id)) }
    }
}

impl<T: 'static> ReadSignal<T> for StoredValue<T> {
    fn track(&self) {}

    #[track_caller]
    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        let caller = core::panic::Location::caller();
        with_current_runtime(|rt| self.id.with_untracked(rt, f, caller))
    }
}

impl_read_signal_traits!(StoredValue<T>);

impl<T: 'static> WriteSignal<T> for StoredValue<T> {
    fn notify(&self) {}

    #[track_caller]
    fn update_untracked<U>(&mut self, f: impl FnOnce(&mut T) -> U) -> U {
        let caller = core::panic::Location::caller();
        with_current_runtime(|rt| self.id.update_untracked(rt, f, caller))
    }
}
