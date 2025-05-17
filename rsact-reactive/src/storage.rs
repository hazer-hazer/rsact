use crate::{
    callback::AnyCallback,
    runtime::{Runtime, with_current_runtime},
};
use alloc::{boxed::Box, format, rc::Rc};
use core::{
    any::{Any, type_name},
    cell::{Cell, RefCell},
    fmt::{Debug, Display},
    panic::Location,
};
use slotmap::{Key, SlotMap};

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

impl Display for ValueId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.data().as_ffi())
    }
}

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
        caller: &'static Location<'static>,
    ) -> U {
        rt.maybe_update(*self, Some(*self), caller);

        // let value = self.get_untracked(rt);
        #[cfg(feature = "debug-info")]
        rt.storage.set_debug_info(*self, |info| {
            info.borrowed = Some(caller);
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

    pub fn notify(
        &self,
        rt: &Runtime,
        caller: &'static Location<'static>,
    ) -> NotifyResult {
        // if rt.is_dirty(*self) {
        //     return Err(NotifyError::Cycle(self.debug_info(rt)));
        // }

        rt.mark_dirty(*self, Some(*self), caller);
        rt.run_effects(Some(*self), caller);
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

    #[cfg(feature = "debug-info")]
    pub fn debug_info(&self) -> ValueDebugInfo {
        use crate::runtime::with_current_runtime;

        with_current_runtime(|rt| rt.debug_info(*self))
    }

    #[cfg(feature = "debug-info")]
    pub fn mermaid_graph(&self, max_depth: usize) -> alloc::string::String {
        use crate::runtime::with_current_runtime;

        with_current_runtime(|rt| rt.mermaid_graph(*self, max_depth))
    }

    #[cfg(feature = "debug-info")]
    pub fn set_name(&self, name: &'static str) {
        with_current_runtime(|rt| {
            rt.storage.set_debug_info(*self, |debug_info| {
                debug_info.name = Some(name)
            });
        })
    }

    #[track_caller]
    pub fn dirten(&self) {
        let caller = Location::caller();
        with_current_runtime(|rt| {
            rt.mark_dirty(*self, Some(*self), caller);
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ValueDebugInfoState {
    Clean(Option<ValueId>),
    CheckRequested(
        &'static Location<'static>,
        /** requester */ Option<ValueId>,
    ),
    Dirten(&'static Location<'static>, /** requester */ Option<ValueId>),
}

impl Display for ValueDebugInfoState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ValueDebugInfoState::Clean(..) => "clean",
                ValueDebugInfoState::CheckRequested(..) => "check",
                ValueDebugInfoState::Dirten(..) => "dirten",
            }
        )
    }
}

#[derive(Clone, Copy)]
pub struct ValueDebugInfo {
    pub name: Option<&'static str>,
    pub created_at: &'static Location<'static>,
    pub state: ValueDebugInfoState,
    pub borrowed_mut: Option<&'static Location<'static>>,
    pub borrowed: Option<&'static Location<'static>>,
    pub ty: &'static str,
    pub observer: Option<&'static Location<'static>>,
    // TODO: Add Value kind
}

impl ValueDebugInfo {
    #[allow(unused)]
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
        write!(f, "Value");
        if let Some(name) = self.name {
            write!(f, " '{name}'")?;
        }
        write!(f, " of type `{}`. ", self.ty)?;
        write!(f, "Created at {}. ", self.created_at)?;
        match self.state {
            ValueDebugInfoState::Clean(requester) => {
                if let Some(requester) = requester {
                    writeln!(f, "Cleaned by {requester}")?;
                } else {
                    writeln!(f, "Clean")?;
                }
            },
            ValueDebugInfoState::CheckRequested(location, requester) => {
                write!(f, "Check requested at {location}")?;
                if let Some(requester) = requester {
                    writeln!(f, " by {requester}")?;
                }
            },
            ValueDebugInfoState::Dirten(location, requester) => {
                write!(f, "Dirten at {location}")?;
                if let Some(requester) = requester {
                    writeln!(f, " by {requester}")?;
                }
            },
        }
        if let Some(borrowed) = self.borrowed {
            write!(f, "Borrowed at {}. ", borrowed)?;
        }
        if let Some(borrowed_mut) = self.borrowed_mut {
            write!(f, "Borrowed Mutably at {}. ", borrowed_mut)?;
        }
        if let Some(observer) = self.observer {
            write!(f, "Observed at {}. ", observer)?;
        }
        Ok(())
    }
}

// pub struct ValueDebugInfoTree {
//     info: ValueDebugInfo,
//     subs: alloc::vec::Vec<ValueId>,
//     sources: alloc::vec::Vec<ValueId>,
// }

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
    Observer,
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
                ValueKind::MemoChain { .. } => "memoChain",
                ValueKind::Computed { .. } => "computed",
                ValueKind::Observer => "observer",
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

impl Display for ValueState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ValueState::Clean => "clean",
                ValueState::Check => "check",
                ValueState::Dirty => "dirty",
            }
        )
    }
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
        requester: Option<ValueId>,
        caller: &'static Location<'static>,
    ) {
        #[cfg(feature = "debug-info")]
        self.set_debug_info(id, |debug_info| match state {
            ValueState::Clean => {
                debug_info.state = ValueDebugInfoState::Clean(requester);
            },
            ValueState::Check => {
                debug_info.state =
                    ValueDebugInfoState::CheckRequested(caller, requester);
            },
            ValueState::Dirty => {
                debug_info.state =
                    ValueDebugInfoState::Dirten(caller, requester)
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
