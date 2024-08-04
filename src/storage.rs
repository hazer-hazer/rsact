use core::{
    any::{type_name, Any},
    cell::RefCell,
    fmt::Debug,
    marker::PhantomData,
};

use alloc::{format, rc::Rc};
use slotmap::SlotMap;

use crate::{
    effect::{AnyCallback, EffectCallback},
    runtime::Runtime,
};

slotmap::new_key_type! {
    pub struct ValueId;
}

impl ValueId {
    pub fn get_untracked(&self, runtime: &Runtime) -> Rc<RefCell<dyn Any>> {
        let values = &runtime.storage.values.borrow();
        let value = values.get(*self).unwrap().value();

        value
    }

    #[inline(always)]
    pub fn with<T: 'static, U>(&self, runtime: &Runtime, f: impl FnOnce(&T) -> U) -> U {
        runtime.subscribe(*self);

        let value = self.get_untracked(runtime);
        let value = RefCell::try_borrow(&value).expect("Failed to borrow value");
        let value = value
            .downcast_ref::<T>()
            .expect(&format!("Failed to cast value to {}", type_name::<T>()));

        f(value)
    }

    #[inline(always)]
    pub fn update<T: 'static, U>(&self, rt: &Runtime, f: impl FnOnce(&mut T) -> U) -> U {
        let result = {
            let value = self.get_untracked(rt);
            let mut value = RefCell::borrow_mut(&value);

            let value = value
                .downcast_mut::<T>()
                .expect(&format!("Failed to mut cast value to {}", type_name::<T>()));
            f(value)
        };

        rt.mark_dirty(*self);
        rt.run_effects();

        result
    }
}

#[derive(Clone)]
pub enum ValueKind {
    Signal,
    Effect { f: Rc<dyn AnyCallback> },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ValueState {
    Clean,
    Dirty,
}

#[derive(Clone)]
pub struct StoredValue {
    pub value: Rc<RefCell<dyn Any>>,
    pub kind: ValueKind,
    pub state: ValueState,
}

impl StoredValue {
    pub(crate) fn value(&self) -> Rc<RefCell<dyn Any>> {
        self.value.clone()
    }

    fn mark(&mut self, state: ValueState) {
        self.state = state;
    }
}

#[derive(Default)]
pub struct Storage {
    values: RefCell<SlotMap<ValueId, StoredValue>>,
}

impl Storage {
    pub fn create_signal<T: 'static>(&self, value: T) -> ValueId {
        self.values.borrow_mut().insert(StoredValue {
            value: Rc::new(RefCell::new(value)),
            kind: ValueKind::Signal,
            state: ValueState::Clean,
        })
    }

    pub fn create_effect<T: 'static, F>(&self, f: impl Fn(Option<T>) -> T + 'static) -> ValueId {
        self.values.borrow_mut().insert(StoredValue {
            value: Rc::new(RefCell::new(None::<T>)),
            kind: ValueKind::Effect {
                f: Rc::new(EffectCallback { f, ty: PhantomData }),
            },
            state: ValueState::Dirty,
        })
    }

    pub(crate) fn get(&self, id: ValueId) -> StoredValue {
        self.values.borrow().get(id).unwrap().clone()
    }

    pub(crate) fn mark(&self, id: ValueId, state: ValueState) {
        let mut values = self.values.borrow_mut();
        let value = values.get_mut(id).unwrap();
        value.mark(state)
    }

    // pub fn get(&self, id: ValueId) -> &StoredValue {
    //     self.values.get(&id).unwrap()
    // }

    // #[track_caller]
    // fn store<T: Sync + Send + 'static>(&mut self, value: T) -> ValueLocation {
    //     let location = ValueLocation::new();

    //     assert!(self
    //         .values
    //         .borrow_mut()
    //         .insert(location, Box::new(value))
    //         .is_none());

    //     location
    // }

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
