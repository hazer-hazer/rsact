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

// TODO: Rename to something tautology like `new_void_scope` or `new_childless_scope` or `new_inert_scope`
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

/// A RAII guard that owns a reactive scope.
///
/// When `ScopeHandle` is dropped all reactive values (signals, memos, effects)
/// that were created while this scope was active are disposed. Scopes can be
/// nested: child scopes are dropped before their parent.
///
/// Obtain a handle from [`new_scope`] or [`new_deny_new_scope`].
#[must_use]
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
    use crate::{
        effect::create_effect, prelude::create_signal, read::ReadSignal,
        runtime::with_new_runtime, scope::new_scope, write::WriteSignal,
    };
    use alloc::rc::Rc;
    use core::cell::Cell;

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

    /// After a scope containing an effect is dropped, writing to a signal that
    /// the effect subscribed to must not panic (ghost subscriptions removed).
    #[test]
    fn dispose_removes_ghost_subscriptions() {
        with_new_runtime(|_| {
            let mut sig = create_signal(0i32);
            {
                let _scope = new_scope();
                let s = sig;
                create_effect(move |_: Option<()>| {
                    s.get();
                });
            }
            // Scope dropped → effect disposed.
            // Writing the signal must not panic even though the effect previously
            // subscribed to it. (ghost entry would cause a mark on a dead ValueId)
            sig.set(1);
            sig.set(2);
        });
    }

    /// After a scope is dropped, effects inside it must no longer run when
    /// their source signals change.
    #[test]
    fn dispose_stops_effect_from_running() {
        with_new_runtime(|_| {
            let run_count = Rc::new(Cell::new(0u32));
            let mut sig = create_signal(0i32);

            {
                let _scope = new_scope();
                let count = run_count.clone();
                let s = sig;
                create_effect(move |_: Option<()>| {
                    s.get();
                    count.set(count.get() + 1);
                });
            }

            let after_scope = run_count.get(); // ran once on creation

            // Signal write after scope drop must not re-run the disposed effect.
            sig.set(99);
            assert_eq!(
                run_count.get(),
                after_scope,
                "disposed effect still ran"
            );
        });
    }

    /// Values created *inside* an effect body (owned values) should be
    /// disposed together with the effect when its scope is dropped.
    #[test]
    fn dispose_cascades_to_owned_values() {
        use crate::signal::Signal;
        use alloc::rc::Rc;
        use core::cell::RefCell;

        with_new_runtime(|_| {
            // Signal<T> is Copy, so we share via Rc<RefCell<>> to capture a
            // reference across the closure boundary.
            let inner_sig: Rc<RefCell<Option<Signal<i32>>>> =
                Rc::new(RefCell::new(None));
            let captured = inner_sig.clone();

            {
                let _scope = new_scope();
                create_effect(move |_: Option<()>| {
                    // Create a signal inside the effect body — it becomes an
                    // owned child of this effect.
                    *captured.borrow_mut() = Some(create_signal(42i32));
                });
            }

            // Scope dropped → effect disposed → owned inner signal disposed.
            let guard = inner_sig.borrow();
            let still_alive = guard.as_ref().map_or(false, |s| s.is_alive());
            assert!(
                !still_alive,
                "owned inner signal still alive after scope drop"
            );
        });
    }

    /// Dropping a scope that contains both an effect and a signal should not
    /// double-dispose the signal if it is also an owned child of the effect.
    #[test]
    fn no_double_dispose_when_scope_and_owned_overlap() {
        with_new_runtime(|_| {
            // Create a scope so the signal is tracked by the scope AND can
            // be an owned child of the effect inside the same scope.
            let scope = new_scope();
            let mut outer = create_signal(0i32);
            let inner_ref = Rc::new(Cell::new(false));
            let flag = inner_ref.clone();
            create_effect(move |_: Option<()>| {
                outer.get();
                // Create an inner signal as an owned child of this effect.
                let _inner = create_signal(0i32);
                flag.set(true);
            });
            // Dropping scope here disposes both the effect and the signal.
            // The inner signal is already disposed by the effect's cleanup;
            // the scope must not panic when it sees it is already gone.
            drop(scope);
            assert!(inner_ref.get());
        });
    }
}
