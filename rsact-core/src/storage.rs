use alloc::{
    boxed::Box, collections::btree_map::BTreeMap, format, rc::Rc, vec::Vec,
};
use core::{
    any::{type_name, Any},
    cell::{Ref, RefCell},
    fmt::Debug,
    marker::PhantomData,
};
use slotmap::SlotMap;

use crate::{
    callback::AnyCallback,
    effect::EffectCallback,
    operator::{AnyOperator, Operation, OperatorState},
    runtime::{Observer, Runtime},
};

slotmap::new_key_type! {
    pub struct ValueId;
}

impl ValueId {
    pub(crate) fn get_untracked(
        &self,
        runtime: &Runtime,
    ) -> Rc<RefCell<dyn Any>> {
        let values = &runtime.storage.values.borrow();
        let value = values.get(*self).unwrap().value();

        value
    }

    pub(crate) fn subscribe(&self, rt: &Runtime) {
        rt.subscribe(*self);
    }

    #[inline(always)]
    pub(crate) fn with_untracked<T: 'static, U>(
        &self,
        rt: &Runtime,
        f: impl FnOnce(&T) -> U,
    ) -> U {
        let value = self.get_untracked(rt);
        let value =
            RefCell::try_borrow(&value).expect("Failed to borrow value");
        let value = value
            .downcast_ref::<T>()
            .expect(&format!("Failed to cast value to {}", type_name::<T>()));

        f(value)
    }

    pub(crate) fn notify(&self, rt: &Runtime) {
        rt.mark_dirty(*self);
        rt.run_effects();
    }

    #[inline(always)]
    pub(crate) fn update_untracked<T: 'static, U>(
        &self,
        rt: &Runtime,
        f: impl FnOnce(&mut T) -> U,
    ) -> U {
        let result = {
            let value = self.get_untracked(rt);
            let mut value = RefCell::borrow_mut(&value);

            let value = value.downcast_mut::<T>().expect(&format!(
                "Failed to mut cast value to {}",
                type_name::<T>()
            ));
            f(value)
        };

        result
    }
}

#[derive(Clone)]
pub enum ValueKind {
    Signal,
    Effect {
        f: Rc<dyn AnyCallback>,
    },
    // Computed { f: Rc<dyn AnyCallback> },
    Operator {
        // TODO: Really clone? Store separately
        scheduled: BTreeMap<Observer, Vec<Rc<dyn Any>>>,
        operator: Rc<dyn AnyOperator>,
    },
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

    pub fn create_effect<T, F>(&self, f: F) -> ValueId
    where
        T: 'static,
        F: Fn(Option<T>) -> T + 'static,
    {
        self.values.borrow_mut().insert(StoredValue {
            value: Rc::new(RefCell::new(None::<T>)),
            kind: ValueKind::Effect {
                f: Rc::new(EffectCallback { f, ty: PhantomData }),
            },
            state: ValueState::Dirty,
        })
    }

    pub fn create_operator<T, F, O>(&self, f: F) -> ValueId
    where
        T: Default + 'static,
        F: for<'a, 'b> Fn(&'a O, &'b mut T) + 'static,
        O: Operation + 'static,
    {
        self.values.borrow_mut().insert(StoredValue {
            value: Rc::new(RefCell::new(<T as Default>::default())),
            kind: ValueKind::Operator {
                scheduled: Default::default(),
                operator: Rc::new(OperatorState {
                    ty: PhantomData,
                    op: PhantomData,
                    f,
                }),
            },
            state: ValueState::Clean,
        })
    }

    // pub fn create_computed<T: 'static, F>(&self, f: impl Fn() -> T + 'static)
    // -> ValueId {     self.values.borrow_mut().insert(StoredValue {
    //         value: Rc::new(RefCell::new(None::<T>)),
    //         kind: ValueKind::Computed { f:  },
    //         state: ValueState::Clean,
    //     })
    // }

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
    // fn store<T: Sync + Send + 'static>(&mut self, value: T) -> ValueLocation
    // {     let location = ValueLocation::new();

    //     assert!(self
    //         .values
    //         .borrow_mut()
    //         .insert(location, Box::new(value))
    //         .is_none());

    //     location
    // }

    // fn read<'a, T: 'static>(&'a self, location: ValueLocation) -> &dyn Fn()
    // -> &'a T {     &move || {
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
