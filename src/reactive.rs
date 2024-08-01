use core::{
    borrow::{Borrow, BorrowMut},
    cell::{Ref, RefCell},
    fmt::Debug,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::Deref,
};

use alloc::{boxed::Box, collections::btree_map::BTreeMap, format};
use spin::MutexGuard;

use crate::runtime::{Runtime, RUNTIME};

// Reactive is the owner of inner value of type T, but this value is passed to runtime. It would be easier to move ownership of this value to Runtime, but it would require heterogeneous collection to store all the different values.
#[derive(Clone, Copy, Debug)]
pub struct Reactive<T> {
    location: ValueLocation,
    marker: PhantomData<T>,
}

impl<T: Sync + Send + 'static> Reactive<T> {
    #[track_caller]
    pub fn new(value: T) -> Self {
        Self {
            location: RUNTIME.lock().storage.store(value),
            marker: PhantomData,
        }
    }
}

impl Runtime {
    #[track_caller]
    fn read<T: 'static>(&self, location: ValueLocation) -> &T {
        let val = self.storage.values.get(&location).unwrap();

        val.downcast_ref().expect(&format!(
            "[BUG] Expected reactive of type {}, got [?]",
            core::any::type_name::<T>()
        ))
    }
}

impl<T: 'static> Deref for Reactive<T> {
    type Target = T;

    #[track_caller]
    fn deref(&self) -> &Self::Target {
        // RUNTIME.lock().read::<T>(self.location)
        todo!()
    }
}

#[derive(Clone, Copy)]
struct ValuePointer<'a> {
    location: ValueLocation,
    storage: &'a Storage,
}

// impl ValuePointer {
//     #[track_caller]
//     fn new() -> Self {
//         Self {
//             location: ValueLocation::new(),
//             storage: todo!(),
//         }
//     }
// }

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct ValueLocation(core::panic::Location<'static>);

impl ValueLocation {
    #[track_caller]
    fn new() -> Self {
        Self(*core::panic::Location::caller())
    }
}

#[derive(Default)]
pub struct Storage {
    values: BTreeMap<ValueLocation, Box<dyn core::any::Any + Sync + Send + 'static>>,
}

impl Storage {
    #[track_caller]
    fn store<T: Sync + Send + 'static>(&mut self, value: T) -> ValueLocation {
        let location = ValueLocation::new();

        assert!(self
            .values
            .borrow_mut()
            .insert(location, Box::new(value))
            .is_none());

        location
    }

    // fn read<'a, T: 'static>(&'a self, location: ValueLocation) -> &dyn Fn() -> &'a T {
    //     &move || {
    //         self.values[&location]
    //             .borrow()
    //             .inner
    //             .downcast_ref()
    //             .unwrap()
    //     }
    // }

    // fn read<T: 'static>(&self, location: ValueLocation) -> &T {
    //     self.values
    //         .get(&location)
    //         .unwrap()
    //         .borrow()
    //         .inner
    //         .downcast_ref()
    //         .unwrap()

    //     // Ref::filter_map(r, |value| value.inner.downcast_ref()).unwrap()
    // }
}

#[derive(Debug)]
struct StoredValue {
    inner: Box<dyn core::any::Any + Sync + Send + 'static>,
}

impl StoredValue {
    fn new<T: Sync + Send + 'static>(inner: T) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }

    fn read<T: 'static>(&self) -> &T {
        self.inner.as_ref().downcast_ref().unwrap()
    }
}
