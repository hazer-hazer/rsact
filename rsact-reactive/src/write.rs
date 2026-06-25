use crate::ReactiveValue;
use core::ops::ControlFlow;

/// Return value from a [`WriteSignal::update_if`] closure that controls
/// whether subscribers are notified after the update.
///
/// Implemented for [`core::ops::ControlFlow`]:
/// - `ControlFlow::Break(_)` → subscribers are notified (update happened).
/// - `ControlFlow::Continue(_)` → subscribers are **not** notified.
///
/// Useful when walking tree-structured data and only some branches changed.
pub trait UpdateNotification {
    fn is_updated(&self) -> bool;
}

// Maybe better only add this to ControlFlow without `UpdateNotification` trait
impl<B, C> UpdateNotification for ControlFlow<B, C> {
    fn is_updated(&self) -> bool {
        matches!(self, ControlFlow::Break(_))
    }
}

// TODO: Add `change` method, like `set` but notifies only if value is changed.
// Open question is if `change` should track get of current value to compare
// with new one or do it silently

// TODO: Can use Reactive::Value instead of requiring `T`
/// Write access to a reactive value.
///
/// The two required methods are [`WriteSignal::notify`] and
/// [`WriteSignal::update_untracked`]. All other methods are default
/// implementations built on top of those two.
///
/// | Method | Tracks read? | Notifies subscribers? |
/// |---|---|---|
/// | `update_untracked` | no | no |
/// | `set_untracked` | no | no |
/// | `update` | no | yes |
/// | `set` | no | yes |
/// | `update_if` | no | only if `UpdateNotification::is_updated` |
///
/// Implemented by: [`crate::signal::Signal`], [`crate::maybe::MaybeSignal`],
/// [`crate::trigger::Trigger`].
pub trait WriteSignal<T> {
    // TODO: Must be mutable access
    /// Notify subscribers that signal is updated
    fn notify(&self);

    /// Update signal value without notifying subscribers. In pair with
    /// [`WriteSignal::notify`] they form [`WriteSignal::update`] function.
    fn update_untracked<U>(&mut self, f: impl FnOnce(&mut T) -> U) -> U;

    /// Update [`WriteSignal`] but notify subscribers only if updater `f`
    /// returns value which denotes effective update. [`UpdateNotification`]
    /// is implemented for example for [`core::ops::ControlFlow`] where
    /// subscribers are notified in case of [`core::ops::ControlFlow::Break`].
    /// Useful in tree-structured data walking with reactivity.
    #[track_caller]
    fn update_if<U: UpdateNotification>(
        &mut self,
        f: impl FnOnce(&mut T) -> U,
    ) -> U {
        let result = self.update_untracked(f);
        if result.is_updated() {
            self.notify();
        }
        result
    }

    /// Update signal and notify subscribers. If you just want to assign a new
    /// value to the signal, use [`WriteSignal::set`]
    #[track_caller]
    fn update<U>(&mut self, f: impl FnOnce(&mut T) -> U) -> U {
        let result = self.update_untracked(f);
        self.notify();

        result
    }

    /// Update signal by assigning new value. If you need to map the value or to
    /// update a particular part of signal (for example structure field), use
    /// [`WriteSignal::update`]
    #[track_caller]
    fn set(&mut self, new: T) {
        self.update(|value| *value = new)
    }

    /// Same as [`WriteSignal::set`] but does not notify subscribers, see
    /// [`WriteSignal::update_untracked`]/[`WriteSignal::update`]
    #[track_caller]
    fn set_untracked(&mut self, new: T) {
        self.update_untracked(|value| *value = new)
    }
}

/// Bind a signal to a reactive source so the signal stays in sync.
///
/// [`SignalSetter::setter`] wires up a persistent effect: every time `source`
/// changes, `set_map` is called with `(&mut T, &I::Value)` and the signal is
/// updated. For inert sources a one-shot update is performed instead.
///
/// [`SignalSetter::set_from`] is the common-case convenience wrapper for `T =
/// I::Value` with a plain clone mapping.
///
/// # Example
///
/// ```rust
/// # use rsact_reactive::prelude::*;
/// # use rsact_reactive::runtime::with_new_runtime;
/// # use rsact_reactive::write::{WriteSignal, SignalSetter};
/// # with_new_runtime(|_| {
/// let source = create_signal(42u32);
/// let mut target = create_signal(0u32);
/// target.set_from(source.map(|v| *v)); // target mirrors source
/// assert_eq!(target.get(), 42);
/// # });
/// ```
pub trait SignalSetter<T: 'static, I: ReactiveValue> {
    fn setter(
        &mut self,
        source: I,
        set_map: impl FnMut(&mut T, &I::Value) + 'static,
    );

    fn set_from(&mut self, source: I)
    where
        T: Clone,
        I: ReactiveValue<Value = T>,
        Self: Sized + 'static,
    {
        self.setter(source, |this, new| *this = new.clone());
    }
}

// TODO: *Assign ops implementations
