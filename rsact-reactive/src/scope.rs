use crate::{runtime::with_current_runtime, storage::ValueId};
use alloc::vec::Vec;
use core::fmt::Display;

/// Creates new scope, all reactive values will be dropped on scope drop. Scope dropped automatically when returned ScopeHandle drops.
#[must_use]
#[track_caller]
pub fn new_scope() -> ScopeHandle {
    #[cfg(feature = "debug-info")]
    let caller = core::panic::Location::caller();
    with_current_runtime(|rt| {
        rt.new_scope(
            #[cfg(feature = "debug-info")]
            caller,
        )
    })
}

// TODO: Rename to something tautology like `new_void_scope` or `new_childless_scope`
/// Creates new scope where creation of new reactive values is disallowed and will cause a panic. Useful mostly only for debugging.
#[track_caller]
pub fn new_deny_new_scope() -> ScopeHandle {
    #[cfg(feature = "debug-info")]
    let caller = core::panic::Location::caller();
    with_current_runtime(|rt| {
        rt.new_deny_new_scope(
            #[cfg(feature = "debug-info")]
            caller,
        )
    })
}

slotmap::new_key_type! {
    pub struct ScopeId;
}

pub struct ScopeData {
    // TODO: Use enum, deny_new makes values field useless
    pub(crate) deny_new: bool,
    pub(crate) values: Vec<ValueId>,
    #[cfg(feature = "debug-info")]
    pub(crate) created_at: &'static core::panic::Location<'static>,
}

#[cfg(feature = "debug-info")]
impl Display for ScopeData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Scope at {}", self.created_at)
    }
}

#[cfg(not(feature = "debug-info"))]
impl Display for ScopeData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "")
    }
}

impl ScopeData {
    pub fn new(
        #[cfg(feature = "debug-info")]
        created_at: &'static core::panic::Location<'static>,
    ) -> Self {
        Self {
            deny_new: false,
            values: Default::default(),
            #[cfg(feature = "debug-info")]
            created_at,
        }
    }

    pub fn new_deny_new(
        #[cfg(feature = "debug-info")]
        created_at: &'static core::panic::Location<'static>,
    ) -> Self {
        Self {
            deny_new: true,
            values: Default::default(),
            #[cfg(feature = "debug-info")]
            created_at,
        }
    }
}

pub struct ScopeHandle {
    scope_id: ScopeId,
}

impl ScopeHandle {
    pub(crate) fn new(scope_id: ScopeId) -> Self {
        Self { scope_id }
    }
}

impl Drop for ScopeHandle {
    fn drop(&mut self) {
        with_current_runtime(|rt| {
            rt.drop_scope(self.scope_id);
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{prelude::create_signal, scope::new_scope};

    #[test]
    fn scoping() {
        let parent = create_signal(0);

        let scoped = {
            let _scope = new_scope();
            let scoped = create_signal(0);

            assert!(scoped.is_alive());

            scoped
        };

        assert!(!scoped.is_alive());
        assert!(parent.is_alive());
    }

    #[test]
    fn signal_dropped() {
        let scoped = {
            let _scope = new_scope();
            let signal = create_signal(0);

            assert!(signal.is_alive());

            signal
        };

        assert!(!scoped.is_alive())
    }

    #[test]
    fn extend_guard_lifetime() {
        let (_scope, signal) = {
            let scope = new_scope();
            let signal = create_signal(0);

            assert!(signal.is_alive());

            (scope, signal)
        };

        assert!(signal.is_alive());
    }
}
