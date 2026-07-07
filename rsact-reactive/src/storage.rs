use crate::{callback::AnyCallback, runtime::Runtime};
use alloc::rc::Rc;
use core::{
    any::{Any, type_name},
    cell::RefCell,
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
    // TODO: In an ideal world I would love track methods in ReadSignal to
    // return `true` if the value is the source of change of the current
    // observer. This would be perfect for debugging.
    // TODO: Add `subscribe_with_current_rt` for simplicity
    pub(crate) fn subscribe(&self, rt: &Runtime) {
        rt.subscribe(*self);
    }

    #[track_caller]
    #[inline(always)]
    pub fn with_untracked<T: 'static, U>(
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
        // Clone only the value cell `Rc` (one refcount bump), not the whole
        // `Value` — this runs on every read.
        let cell = rt.storage.value_rc(*self).unwrap();
        let value = match RefCell::try_borrow(&cell) {
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
        // Build the panic message lazily so the success path allocates nothing.
        let value = value.downcast_ref::<T>().unwrap_or_else(|| {
            panic!("Failed to cast value to {}", type_name::<T>())
        });

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
    pub fn update_untracked<T: 'static, U>(
        &self,
        rt: &Runtime,
        f: impl FnOnce(&mut T) -> U,
        _caller: &'static Location<'static>,
    ) -> U {
        // rt.updating.set(rt.updating.get() + 1);

        // let value = self.get_untracked(rt);
        #[cfg(feature = "debug-info")]
        rt.storage.set_debug_info(*self, |debug_info| {
            debug_info.borrowed_mut = Some(_caller);
        });

        // Clone only the value cell `Rc`, not the whole `Value`.
        let cell = rt.storage.value_rc(*self).unwrap();

        let mut value = RefCell::borrow_mut(&cell);

        // Build the panic message lazily so the success path allocates nothing.
        let value = value.downcast_mut::<T>().unwrap_or_else(|| {
            panic!("Failed to mut cast value to {}", type_name::<T>())
        });

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
        use crate::runtime::with_current_runtime;

        with_current_runtime(|rt| {
            rt.storage.set_debug_info(*self, |debug_info| {
                debug_info.name = Some(name)
            });
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ValueDebugInfoState {
    Clean(Option<(ValueId, &'static Location<'static>)>),
    CheckRequested(
        &'static Location<'static>,
        /** requester */ Option<(ValueId, &'static Location<'static>)>,
    ),
    Dirten(
        &'static Location<'static>,
        /** requester */ Option<(ValueId, &'static Location<'static>)>,
    ),
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

#[derive(Clone, Copy, Debug)]
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
        write!(f, "Value")?;
        if let Some(name) = self.name {
            write!(f, " '{name}'")?;
        }
        write!(f, " of type `{}`. ", self.ty)?;
        write!(f, "Created at {}. ", self.created_at)?;
        match self.state {
            ValueDebugInfoState::Clean(requester) => {
                if let Some((_, requester)) = requester {
                    writeln!(f, "Cleaned by {requester}")?;
                } else {
                    writeln!(f, "Clean")?;
                }
            },
            ValueDebugInfoState::CheckRequested(location, requester) => {
                write!(f, "Check requested at {location}")?;
                if let Some((_, requester)) = requester {
                    writeln!(f, " by {requester}")?;
                }
            },
            ValueDebugInfoState::Dirten(location, requester) => {
                write!(f, "Dirten at {location}")?;
                if let Some((_, requester)) = requester {
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
    Stored,
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
    Observer,
}

/// A cheap `Copy` discriminant of [`ValueKind`], used on hot paths that only
/// need to know *which* kind a value is (state machine, effect detection)
/// without cloning the `Rc`-holding [`ValueKind`] out of storage.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ValueKindTag {
    Stored,
    Signal,
    Effect,
    Memo,
    Computed,
    Observer,
}

impl ValueKind {
    #[inline]
    pub(crate) fn tag(&self) -> ValueKindTag {
        match self {
            ValueKind::Stored => ValueKindTag::Stored,
            ValueKind::Signal => ValueKindTag::Signal,
            ValueKind::Effect { .. } => ValueKindTag::Effect,
            ValueKind::Memo { .. } => ValueKindTag::Memo,
            ValueKind::Computed { .. } => ValueKindTag::Computed,
            ValueKind::Observer => ValueKindTag::Observer,
        }
    }
}

impl Display for ValueKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ValueKind::Stored => "stored",
                ValueKind::Effect { .. } => "effect",
                ValueKind::Signal => "signal",
                ValueKind::Memo { .. } => "memo",
                ValueKind::Computed { .. } => "computed",
                ValueKind::Observer => "observer",
            }
        )
    }
}

// Note: Order matters
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
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
pub struct Value {
    pub value: Rc<RefCell<dyn Any>>,
    pub kind: ValueKind,
    pub state: ValueState,
    /// Topological height in the reactive graph (0 = source signal, n+1 =
    /// subscriber of height-n node). Used to run pending effects in
    /// topological order, preventing glitches.
    pub height: u32,
    #[cfg(feature = "debug-info")]
    pub debug: ValueDebugInfo,
}

impl Value {
    fn mark(&mut self, state: ValueState) {
        self.state = state;
    }
}

#[derive(Default)]
pub struct Storage {
    pub(crate) values: RefCell<SlotMap<ValueId, Value>>,
}

impl Storage {
    pub(crate) fn add_value(&self, value: Value) -> ValueId {
        self.values.borrow_mut().insert(value)
    }

    pub(crate) fn get(&self, id: ValueId) -> Option<Value> {
        self.values.borrow().get(id).cloned()
    }

    /// Cheap read of the `Copy` [`ValueState`] field without cloning the whole
    /// [`Value`] (which would clone the value `Rc` and 1-3 callback `Rc`s in
    /// `ValueKind`). Hot path: called for every `state()`/`is()` check.
    pub(crate) fn state_of(&self, id: ValueId) -> Option<ValueState> {
        self.values.borrow().get(id).map(|v| v.state)
    }

    /// Cheap read of the [`ValueKindTag`] discriminant without cloning the
    /// `Rc`-holding [`ValueKind`].
    pub(crate) fn kind_of(&self, id: ValueId) -> Option<ValueKindTag> {
        self.values.borrow().get(id).map(|v| v.kind.tag())
    }

    /// Clone only the inner value cell `Rc` (a single refcount bump) instead of
    /// the entire [`Value`]. Used by read/write access paths that need the cell
    /// but not the kind.
    pub(crate) fn value_rc(&self, id: ValueId) -> Option<Rc<RefCell<dyn Any>>> {
        self.values.borrow().get(id).map(|v| v.value.clone())
    }

    pub(crate) fn get_height(&self, id: ValueId) -> u32 {
        self.values.borrow().get(id).map(|v| v.height).unwrap_or(0)
    }

    pub(crate) fn set_height(&self, id: ValueId, height: u32) {
        let mut values = self.values.borrow_mut();
        if let Some(value) = values.get_mut(id) {
            value.height = height;
        }
    }

    #[cfg(feature = "debug-info")]
    pub(crate) fn debug_info(&self, id: ValueId) -> Option<ValueDebugInfo> {
        self.values.borrow().get(id).map(|value| value.debug)
    }

    pub(crate) fn mark(
        &self,
        id: ValueId,
        state: ValueState,
        _requester: Option<ValueId>,
        _caller: &'static Location<'static>,
    ) {
        let mut values = self.values.borrow_mut();
        // Silently skip marking a value that has already been disposed.
        let Some(value) = values.get_mut(id) else { return };

        value.mark(state);

        #[cfg(feature = "debug-info")]
        {
            let requester = _requester.and_then(|requester| {
                values
                    .get(requester)
                    .map(|value| (requester, value.debug.created_at))
            });

            let Some(value) = values.get_mut(id) else { return };

            value.debug.state = match state {
                ValueState::Clean => ValueDebugInfoState::Clean(requester),
                ValueState::Check => {
                    ValueDebugInfoState::CheckRequested(_caller, requester)
                },
                ValueState::Dirty => {
                    ValueDebugInfoState::Dirten(_caller, requester)
                },
            };
        }
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
