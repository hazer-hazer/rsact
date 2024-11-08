use core::panic::Location;

use alloc::vec::Vec;

use crate::{runtime::with_current_runtime, storage::ValueId};

slotmap::new_key_type! {
    pub struct ScopeId;
}

pub struct ScopeData {
    // TODO: Use enum, deny_new makes values useless
    pub(crate) deny_new: bool,
    pub(crate) values: Vec<ValueId>,
    #[cfg(debug_assertions)]
    pub(crate) created_at: &'static Location<'static>,
}

impl ScopeData {
    pub fn new(
        #[cfg(debug_assertions)] created_at: &'static Location<'static>,
    ) -> Self {
        Self {
            deny_new: false,
            values: Default::default(),
            #[cfg(debug_assertions)]
            created_at,
        }
    }

    pub fn new_deny_new(
        #[cfg(debug_assertions)] created_at: &'static Location<'static>,
    ) -> Self {
        Self {
            deny_new: true,
            values: Default::default(),
            #[cfg(debug_assertions)]
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
    use crate::{prelude::create_signal, runtime::new_scope};

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
