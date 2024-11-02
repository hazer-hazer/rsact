use crate::{
    callback::AnyCallback,
    effect::{EffectCallback, EffectOrder},
    memo::MemoCallback,
    runtime::Runtime,
};
use alloc::{collections::btree_map::BTreeMap, format, rc::Rc, vec::Vec};
use core::{
    any::{type_name, Any, TypeId},
    cell::RefCell,
    fmt::{Debug, Display},
    marker::PhantomData,
    panic::Location,
};
use slotmap::SlotMap;

// TODO: Add typed ValueId's
slotmap::new_key_type! {
    pub struct ValueId;
}

#[derive(Clone, Copy)]
pub enum NotifyError {
    Cycle(ValueDebugInfo),
}
pub type NotifyResult = Result<(), NotifyError>;

impl ValueId {
    // pub(crate) fn get_untracked(&self, rt: &Runtime) -> Rc<RefCell<dyn Any>> {
    //     // let values = &runtime.storage.values.borrow();
    //     // let value = values.get(*self).unwrap().value();

    //     rt.storage.get(*self).unwrap().value
    // }

    // TODO: Add `subscribe_with_current_rt` for simplicity
    pub(crate) fn subscribe(&self, rt: &Runtime) {
        rt.subscribe(*self);
    }

    #[track_caller]
    #[inline(always)]
    pub(crate) fn with_untracked<T: 'static, U>(
        &self,
        rt: &Runtime,
        f: impl FnOnce(&T) -> U,
        caller: &'static Location<'static>,
    ) -> U {
        rt.maybe_update(*self);

        // let value = self.get_untracked(rt);
        rt.storage.set_debug_info(*self, |info| {
            info.borrowed = Some(caller);
        });
        let value = rt.storage.get(*self).unwrap();
        let value = match RefCell::try_borrow(&value.value) {
            Ok(value) => value,
            Err(err) => {
                panic!(
                    "Failed to borrow reactive value: {err}\n{}",
                    rt.debug_info(*self)
                )
            },
        };
        let value = value
            .downcast_ref::<T>()
            .expect(&format!("Failed to cast value to {}", type_name::<T>()));

        let result = f(value);

        rt.storage.set_debug_info(*self, |info| {
            // TODO: Invalid, should reset to previous `borrowed`
            info.borrowed = None;
        });

        result
    }

    #[track_caller]
    pub(crate) fn notify(
        &self,
        rt: &Runtime,
        caller: &'static Location<'static>,
    ) -> NotifyResult {
        // if rt.is_dirty(*self) {
        //     return Err(NotifyError::Cycle(self.debug_info(rt)));
        // }

        rt.mark_dirty(*self, Some(caller));
        rt.run_effects();
        // rt.mark_clean(*self);

        Ok(())
    }

    #[track_caller]
    #[inline(always)]
    pub(crate) fn update_untracked<T: 'static, U>(
        &self,
        rt: &Runtime,
        f: impl FnOnce(&mut T) -> U,
        caller: Option<&'static Location<'static>>,
    ) -> U {
        // rt.updating.set(rt.updating.get() + 1);

        // let value = self.get_untracked(rt);
        rt.storage.set_debug_info(*self, |debug_info| {
            debug_info.borrowed_mut = caller;
        });
        let value = rt.storage.get(*self).unwrap();

        let mut value = RefCell::borrow_mut(&value.value);

        let value = value.downcast_mut::<T>().expect(&format!(
            "Failed to mut cast value to {}",
            type_name::<T>()
        ));

        let result = f(value);

        rt.storage.set_debug_info(*self, |debug_info| {
            // TODO: Reset to previous `borrowed`
            debug_info.borrowed_mut = None;
        });

        // rt.updating.set(rt.updating.get() - 1);

        result
    }
}

#[derive(Clone, Copy, Default)]
pub struct ValueDebugInfo {
    pub creator: Option<&'static Location<'static>>,
    pub dirten: Option<&'static Location<'static>>,
    pub borrowed_mut: Option<&'static Location<'static>>,
    pub borrowed: Option<&'static Location<'static>>,
    pub ty: Option<&'static str>,
    pub observer: Option<&'static Location<'static>>,
    // TODO: Add Value kind
}

impl ValueDebugInfo {
    pub(crate) fn with_observer(
        mut self,
        observer: &'static Location<'static>,
    ) -> Self {
        self.observer = Some(observer);
        self
    }
}

impl core::fmt::Display for ValueDebugInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(ty) = self.ty {
            write!(f, "of type {}\n", ty)?;
        }
        if let Some(creator) = self.creator {
            write!(f, "created at {}\n", creator)?;
        }
        if let Some(dirten) = self.dirten {
            write!(f, "dirten at {}\n", dirten)?;
        }
        if let Some(borrowed) = self.borrowed {
            write!(f, "Borrowed at {}\n", borrowed)?;
        }
        if let Some(borrowed_mut) = self.borrowed_mut {
            write!(f, "Borrowed Mutably at {}\n", borrowed_mut)?;
        }
        if let Some(observer) = self.observer {
            write!(f, "Observed at {}\n", observer)?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub enum ValueKind {
    Signal,
    Effect {
        f: Rc<dyn AnyCallback>,
    },
    Memo {
        f: Rc<dyn AnyCallback>,
    },
    MemoChain {
        initial: Rc<dyn AnyCallback>,
        // TODO: Optimize, don't use BtreeMap but fixed structure with each
        // EffectOrder
        fs: Rc<RefCell<BTreeMap<EffectOrder, Vec<Rc<dyn AnyCallback>>>>>,
    },
}

impl Display for ValueKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ValueKind::Effect { .. } => "effect",
                ValueKind::Signal => "signal",
                ValueKind::Memo { .. } => "memo",
                ValueKind::MemoChain { .. } => "memo chain",
            }
        )
    }
}

// Note: Order matters
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValueState {
    Clean,
    Check,
    Dirty,
}

#[derive(Clone)]
pub struct StoredValue {
    pub value: Rc<RefCell<dyn Any>>,
    pub kind: ValueKind,
    pub state: ValueState,
    pub debug: ValueDebugInfo,
}

impl StoredValue {
    // pub(crate) fn value(&self) -> Rc<RefCell<dyn Any>> {
    //     self.value.clone()
    // }

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
            kind: ValueKind::Signal,
            state: ValueState::Clean,
            debug: ValueDebugInfo {
                creator: Some(caller),
                dirten: None,
                borrowed: None,
                borrowed_mut: None,
                ty: Some(type_name::<T>()),
                observer: None,
            },
        })
    }

    #[track_caller]
    pub fn create_effect<T, F>(
        &self,
        f: F,
        caller: &'static Location<'static>,
    ) -> ValueId
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
            debug: ValueDebugInfo {
                creator: Some(caller),
                dirten: None,
                borrowed: None,
                borrowed_mut: None,
                ty: Some(type_name::<F>()),
                observer: None,
            },
        })
    }

    #[track_caller]
    pub fn create_memo<T, F>(
        &self,
        f: F,
        caller: &'static Location<'static>,
    ) -> ValueId
    where
        T: PartialEq + 'static,
        F: Fn(Option<&T>) -> T + 'static,
    {
        self.values.borrow_mut().insert(StoredValue {
            value: Rc::new(RefCell::new(None::<T>)),
            kind: ValueKind::Memo {
                f: Rc::new(MemoCallback { f, ty: PhantomData }),
            },
            state: ValueState::Dirty,
            debug: ValueDebugInfo {
                creator: Some(caller),
                dirten: None,
                borrowed: None,
                borrowed_mut: None,
                ty: Some(type_name::<F>()),
                observer: None,
            },
        })
    }

    pub fn create_memo_chain<T, F>(
        &self,
        f: F,
        caller: &'static Location<'static>,
    ) -> ValueId
    where
        T: PartialEq + 'static,
        F: Fn(Option<&T>) -> T + 'static,
    {
        self.values.borrow_mut().insert(StoredValue {
            value: Rc::new(RefCell::new(None::<T>)),
            kind: ValueKind::MemoChain {
                initial: Rc::new(MemoCallback { f, ty: PhantomData }),
                fs: Rc::new(RefCell::new(BTreeMap::new())),
            },
            state: ValueState::Dirty,
            debug: ValueDebugInfo {
                creator: Some(caller),
                dirten: None,
                borrowed: None,
                borrowed_mut: None,
                ty: Some(type_name::<F>()),
                observer: None,
            },
        })
    }

    pub(crate) fn get(&self, id: ValueId) -> Option<StoredValue> {
        // self.values.borrow().get(id).unwrap().clone()
        self.values.borrow().get(id).cloned()
    }

    pub(crate) fn debug_info(&self, id: ValueId) -> Option<ValueDebugInfo> {
        self.values.borrow().get(id).cloned().map(|value| value.debug)
    }

    pub(crate) fn mark(
        &self,
        id: ValueId,
        state: ValueState,
        caller: Option<&'static Location<'static>>,
    ) {
        self.set_debug_info(id, |debug_info| match state {
            ValueState::Clean => debug_info.dirten = None,
            ValueState::Check | ValueState::Dirty => debug_info.dirten = caller,
        });

        let mut values = self.values.borrow_mut();
        let value = values.get_mut(id).unwrap();
        value.mark(state);
    }

    pub(crate) fn set_debug_info(
        &self,
        id: ValueId,
        f: impl FnOnce(&mut ValueDebugInfo),
    ) {
        let mut values = self.values.borrow_mut();
        let value = values.get_mut(id).unwrap();

        f(&mut value.debug)
    }
}
