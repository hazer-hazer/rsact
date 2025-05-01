use crate::{callback::AnyCallback, runtime::Runtime};
use alloc::{boxed::Box, format, rc::Rc};
use core::{
    any::{Any, type_name},
    cell::RefCell,
    fmt::{Debug, Display},
    panic::Location,
};
use slotmap::SlotMap;

// TODO: Add typed ValueId's (per Memo, Signal, etc.)
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
        #[cfg(feature = "debug-info")]
        rt.storage.set_debug_info(*self, |info| {
            info.borrowed = Some(_caller);
        });
        let value = rt.storage.get(*self).unwrap();
        let value = match RefCell::try_borrow(&value.value) {
            Ok(value) => value,
            Err(err) => {
                #[cfg(feature = "debug-info")]
                panic!(
                    "Failed to borrow reactive value: {err}\n{}",
                    rt.debug_info(*self)
                );
                #[cfg(not(feature = "debug-info"))]
                panic!("Failed to borrow reactive value: {err}");
            },
        };
        let value = value
            .downcast_ref::<T>()
            .expect(&format!("Failed to cast value to {}", type_name::<T>()));

        let result = f(value);

        #[cfg(feature = "debug-info")]
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
        #[cfg(feature = "debug-info")]
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

        #[cfg(feature = "debug-info")]
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
    Computed {
        f: Rc<RefCell<dyn AnyCallback>>,
    },
    MemoChain {
        memo: Rc<RefCell<dyn AnyCallback>>,
        first: Rc<RefCell<Option<Box<dyn AnyCallback>>>>,
        last: Rc<RefCell<Option<Box<dyn AnyCallback>>>>,
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
                ValueKind::Computed { .. } => "computed",
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
    #[cfg(feature = "debug-info")]
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

    #[cfg(feature = "debug-info")]
    pub(crate) fn debug_info(&self, id: ValueId) -> Option<ValueDebugInfo> {
        self.values.borrow().get(id).map(|value| value.debug)
    }

    pub(crate) fn mark(
        &self,
        id: ValueId,
        state: ValueState,
        _caller: Option<&'static Location<'static>>,
    ) {
        #[cfg(feature = "debug-info")]
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

    #[cfg(feature = "debug-info")]
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
