use crate::{
    callback::AnyCallback,
    effect::{EffectCallback, EffectOrder},
    memo::MemoCallback,
    runtime::Runtime,
};
use alloc::{
    boxed::Box, collections::btree_map::BTreeMap, format, rc::Rc, vec::Vec,
};
use core::{
    any::{type_name, Any},
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
    // fn debug_info(&self, rt: &Runtime) -> ValueDebugInfo {
    //     match rt.storage.get(*self).map(|value| &value.kind) {
    //         Some(&ValueKind::Signal(debug_info)) => debug_info,
    //         _ => ValueDebugInfo::none(),
    //     }
    // }

    // pub(crate) fn get_untracked(&self, rt: &Runtime) -> Rc<RefCell<dyn Any>>
    // {     // let values = &runtime.storage.values.borrow();
    //     let value = rt.storage.values.get(*self).unwrap().value();

    //     value
    // }

    // TODO: Add `subscribe_with_current_rt` for simplicity
    pub(crate) fn subscribe(&self, rt: &mut Runtime) {
        rt.subscribe(*self);
    }

    #[inline(always)]
    pub(crate) fn with_untracked<T: 'static, U>(
        &self,
        rt: &mut Runtime,
        f: impl FnOnce(&T) -> U,
    ) -> U {
        rt.maybe_update(*self);

        // let value = self.get_untracked(rt);
        // let value = match RefCell::try_borrow(&value) {
        //     Ok(value) => value,
        //     Err(err) => {
        //         panic!(
        //             "Failed to borrow reactive value: {err} {}",
        //             self.debug_info(rt)
        //         )
        //     },
        // };
        let value =
            rt.storage.get(*self).unwrap().value.downcast_ref::<T>().expect(
                &format!("Failed to cast value to {}", type_name::<T>()),
            );

        f(value)
    }

    #[track_caller]
    pub(crate) fn notify(
        &self,
        rt: &mut Runtime,
        caller: &'static Location<'static>,
    ) -> NotifyResult {
        // if rt.is_dirty(*self) {
        //     return Err(NotifyError::Cycle(self.debug_info(rt)));
        // }

        rt.mark_dir(*self, Some(caller));
        rt.run_effects();
        // rt.mark_clean(*self);

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn update_untracked<T: 'static, U>(
        &self,
        rt: &mut Runtime,
        f: impl FnOnce(&mut T) -> U,
    ) -> U {
        let result = {
            // let value = self.get_untracked(rt);
            // let mut value = RefCell::borrow_mut(&value);

            let value = rt
                .storage
                .get_mut(*self)
                .unwrap()
                .value
                .downcast_mut::<T>()
                .expect(&format!(
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

// #[derive(Clone)]
pub enum ValueKind {
    Signal(ValueDebugInfo),
    Effect {
        f: Box<dyn AnyCallback + Send>,
    },
    Memo {
        f: Box<dyn AnyCallback + Send>,
    },
    MemoChain {
        initial: Box<dyn AnyCallback + Send>,
        // TODO: Optimize, don't use BtreeMap but fixed structure with each
        // EffectOrder
        fs: Box<BTreeMap<EffectOrder, Vec<Box<dyn AnyCallback + Send>>>>,
    },
}

impl Display for ValueKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ValueKind::Effect { .. } => "effect",
                ValueKind::Signal(_) => "signal",
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

// #[derive(Clone)]
pub struct StoredValue {
    pub value: Box<dyn Any + Send>,
    pub kind: ValueKind,
    pub state: ValueState,
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
    values: SlotMap<ValueId, StoredValue>,
}

impl Storage {
    #[track_caller]
    pub fn create_signal<T: Send + 'static>(
        &mut self,
        value: T,
        caller: &'static Location<'static>,
    ) -> ValueId {
        self.values.insert(StoredValue {
            value: Box::new(value),
            kind: ValueKind::Signal(ValueDebugInfo {
                creator: Some(caller),
                dirten: None,
            }),
            state: ValueState::Clean,
        })
    }

    #[track_caller]
    pub fn create_effect<T, F>(&mut self, f: F) -> ValueId
    where
        T: Send + 'static,
        F: Fn(Option<T>) -> T + Send + 'static,
    {
        self.values.insert(StoredValue {
            value: Box::new(None::<T>),
            kind: ValueKind::Effect {
                f: Box::new(EffectCallback { f, ty: PhantomData }),
            },
            // Note: Check this, might need to be Dirty
            state: ValueState::Dirty,
        })
    }

    #[track_caller]
    pub fn create_memo<T, F>(&mut self, f: F) -> ValueId
    where
        T: Send + PartialEq + 'static,
        F: Fn(Option<&T>) -> T + Send + 'static,
    {
        self.values.insert(StoredValue {
            value: Box::new(None::<T>),
            kind: ValueKind::Memo {
                f: Box::new(MemoCallback { f, ty: PhantomData }),
            },
            state: ValueState::Dirty,
        })
    }

    pub fn create_memo_chain<T, F>(&mut self, f: F) -> ValueId
    where
        T: Send + PartialEq + 'static,
        F: Fn(Option<&T>) -> T + Send + 'static,
    {
        self.values.insert(StoredValue {
            value: Box::new(None::<T>),
            kind: ValueKind::MemoChain {
                initial: Box::new(MemoCallback { f, ty: PhantomData }),
                fs: Box::new(BTreeMap::new()),
            },
            state: ValueState::Dirty,
        })
    }

    pub(crate) fn get(&self, id: ValueId) -> Option<&StoredValue> {
        // self.values.borrow().get(id).unwrap().clone()
        self.values.get(id)
    }

    pub(crate) fn get_mut(&mut self, id: ValueId) -> Option<&mut StoredValue> {
        self.values.get_mut(id)
    }

    pub(crate) fn mark(
        &mut self,
        id: ValueId,
        state: ValueState,
        caller: Option<&'static Location<'static>>,
    ) {
        // let mut values = self.values.borrow_mut();
        let value = self.values.get_mut(id).unwrap();
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
}
