use crate::{callback::AnyCallback, effect::EffectOrder, runtime::Runtime};
use alloc::{collections::btree_map::BTreeMap, format, rc::Rc, vec::Vec};
use core::{
    any::{type_name, Any},
    cell::RefCell,
    fmt::{Debug, Display},
    panic::Location,
};
use slotmap::SlotMap;

// TODO: Add typed ValueId's
slotmap::new_key_type! {
    pub struct ValueId;
}

#[derive(Clone, Copy)]
pub enum NotifyError {
    #[allow(unused)]
    // TODO: ?
    Cycle(ValueDebugInfo),
}
pub type NotifyResult = Result<(), NotifyError>;

impl ValueId {
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
        _caller: &'static Location<'static>,
    ) -> U {
        rt.maybe_update(*self);

        // let value = self.get_untracked(rt);
        #[cfg(debug_assertions)]
        rt.storage.set_debug_info(*self, |info| {
            info.borrowed = Some(_caller);
        });
        let value = rt.storage.get(*self).unwrap();
        let value = match RefCell::try_borrow(&value.value) {
            Ok(value) => value,
            Err(err) => {
                #[cfg(debug_assertions)]
                panic!(
                    "Failed to borrow reactive value: {err}\n{}",
                    rt.debug_info(*self)
                );
                #[cfg(not(debug_assertions))]
                panic!("Failed to borrow reactive value: {err}");
            },
        };
        let value = value
            .downcast_ref::<T>()
            .expect(&format!("Failed to cast value to {}", type_name::<T>()));

        let result = f(value);

        #[cfg(debug_assertions)]
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
        _caller: Option<&'static Location<'static>>,
    ) -> U {
        // rt.updating.set(rt.updating.get() + 1);

        // let value = self.get_untracked(rt);
        #[cfg(debug_assertions)]
        rt.storage.set_debug_info(*self, |debug_info| {
            debug_info.borrowed_mut = _caller;
        });

        let value = rt.storage.get(*self).unwrap();

        let mut value = RefCell::borrow_mut(&value.value);

        let value = value.downcast_mut::<T>().expect(&format!(
            "Failed to mut cast value to {}",
            type_name::<T>()
        ));

        let result = f(value);

        #[cfg(debug_assertions)]
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
    pub created_at: Option<&'static Location<'static>>,
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
        if let Some(creator) = self.created_at {
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
        f: Rc<RefCell<dyn AnyCallback>>,
    },
    Memo {
        f: Rc<RefCell<dyn AnyCallback>>,
    },
    MemoChain {
        initial: Rc<RefCell<dyn AnyCallback>>,
        // TODO: Optimize, don't use BtreeMap but fixed structure with each
        // EffectOrder
        fs: Rc<
            RefCell<BTreeMap<EffectOrder, Vec<Rc<RefCell<dyn AnyCallback>>>>>,
        >,
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
    #[cfg(debug_assertions)]
    pub debug: ValueDebugInfo,
}

impl StoredValue {
    fn mark(&mut self, state: ValueState) {
        self.state = state;
    }
}

#[derive(Default)]
pub struct Storage {
    pub(crate) values: RefCell<SlotMap<ValueId, StoredValue>>,
}

impl Storage {
    pub(crate) fn add_value(&self, value: StoredValue) -> ValueId {
        self.values.borrow_mut().insert(value)
    }

    pub(crate) fn get(&self, id: ValueId) -> Option<StoredValue> {
        self.values.borrow().get(id).cloned()
    }

    #[cfg(debug_assertions)]
    pub(crate) fn debug_info(&self, id: ValueId) -> Option<ValueDebugInfo> {
        self.values.borrow().get(id).map(|value| value.debug)
    }

    pub(crate) fn mark(
        &self,
        id: ValueId,
        state: ValueState,
        _caller: Option<&'static Location<'static>>,
    ) {
        #[cfg(debug_assertions)]
        self.set_debug_info(id, |debug_info| match state {
            ValueState::Clean => debug_info.dirten = None,
            ValueState::Check | ValueState::Dirty => {
                debug_info.dirten = _caller
            },
        });

        let mut values = self.values.borrow_mut();
        let value = values.get_mut(id).unwrap();
        value.mark(state);
    }

    #[cfg(debug_assertions)]
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
