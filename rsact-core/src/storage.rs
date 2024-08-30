use alloc::{
    boxed::Box, collections::btree_map::BTreeMap, format, rc::Rc, vec::Vec,
};
use core::{
    any::{type_name, Any},
    cell::{Ref, RefCell},
    fmt::Debug,
    marker::PhantomData,
    panic::Location,
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

#[derive(Clone, Copy)]
pub enum NotifyError {
    Cycle(ValueDebugInfo),
}
pub type NotifyResult = Result<(), NotifyError>;

impl ValueId {
    fn debug_info(&self, rt: &Runtime) -> ValueDebugInfo {
        match rt.storage.get(*self).kind {
            ValueKind::Signal(debug_info) => debug_info,
            _ => ValueDebugInfo::none(),
        }
    }

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
        let value = match RefCell::try_borrow(&value) {
            Ok(value) => value,
            Err(err) => {
                panic!(
                    "Failed to borrow reactive value: {err} {}",
                    self.debug_info(rt)
                )
            },
        };
        let value = value
            .downcast_ref::<T>()
            .expect(&format!("Failed to cast value to {}", type_name::<T>()));

        f(value)
    }

    #[track_caller]
    pub(crate) fn notify(
        &self,
        rt: &Runtime,
        caller: &'static Location<'static>,
    ) -> NotifyResult {
        if rt.is_dirty(*self) {
            return Err(NotifyError::Cycle(self.debug_info(rt)));
        }

        rt.mark_dirty(*self, Some(caller));
        rt.run_effects();
        rt.mark_clean(*self);

        Ok(())
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

#[derive(Clone, Copy)]
pub struct ValueDebugInfo {
    creator: Option<&'static Location<'static>>,
    dirten: Option<&'static Location<'static>>,
}

impl ValueDebugInfo {
    pub fn none() -> Self {
        Self { creator: None, dirten: None }
    }
}

impl core::fmt::Display for ValueDebugInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(creator) = self.creator {
            write!(f, "Created at {}\n", creator)?;
        }
        if let Some(dirten) = self.dirten {
            write!(f, "Dirten at {}\n", dirten)?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub enum ValueKind {
    Signal(ValueDebugInfo),
    Effect {
        f: Rc<dyn AnyCallback>,
    },
    Memo {
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
    #[track_caller]
    pub fn create_signal<T: 'static>(
        &self,
        value: T,
        caller: &'static Location<'static>,
    ) -> ValueId {
        self.values.borrow_mut().insert(StoredValue {
            value: Rc::new(RefCell::new(value)),
            kind: ValueKind::Signal(ValueDebugInfo {
                creator: Some(caller),
                dirten: None,
            }),
            state: ValueState::Clean,
        })
    }

    #[track_caller]
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
            // Note: Check this, might need to be Dirty
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

    pub(crate) fn mark(
        &self,
        id: ValueId,
        state: ValueState,
        caller: Option<&'static Location<'static>>,
    ) {
        let mut values = self.values.borrow_mut();
        let value = values.get_mut(id).unwrap();
        value.mark(state);

        match (&mut value.kind, state, caller) {
            (
                ValueKind::Signal(ValueDebugInfo { creator: _, dirten }),
                ValueState::Dirty,
                Some(caller),
            ) => {
                dirten.replace(caller);
            },
            _ => {},
        }
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
