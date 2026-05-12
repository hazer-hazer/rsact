use crate::{
    ReactiveValue, prelude::create_signal, read::ReadSignal, signal::Signal,
    write::WriteSignal,
};

/// Create a new [`Trigger`] in the current runtime scope.
///
/// Sugar for [`Trigger::new`].
#[track_caller]
pub fn create_trigger() -> Trigger {
    Trigger::new()
}

/// A unit-valued reactive cell used purely to manually invalidate subscribers.
///
/// `Trigger` wraps a `Signal<()>` and exposes only `track` and `notify`.
/// It is ideal for signalling that *something* changed without carrying any
/// value — for example, invalidating a cache or requesting a UI redraw.
///
/// # Example
///
/// ```rust
/// # use rsact_reactive::prelude::*;
/// # use rsact_reactive::runtime::with_new_runtime;
/// # with_new_runtime(|_| {
/// let trigger = create_trigger();
/// let mut run_count = create_signal(0u32);
///
/// let t = trigger;
/// create_effect(move |_| {
///     t.track();
///     run_count.update_untracked(|c| *c += 1);
/// });
///
/// assert_eq!(run_count.get_untracked(), 1);
/// trigger.notify();
/// assert_eq!(run_count.get_untracked(), 2);
/// # });
/// ```
#[derive(Clone, Copy)]
pub struct Trigger {
    inner: Signal<()>,
}

impl ReactiveValue for Trigger {
    type Value = ();

    fn id(&self) -> Option<crate::storage::ValueId> {
        Some(self.inner.id())
    }

    fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }

    unsafe fn dispose(self) {
        self.inner.dispose();
    }
}

impl Trigger {
    #[track_caller]
    pub fn new() -> Self {
        Self { inner: create_signal(()) }
    }

    pub fn is_alive(self) -> bool {
        self.inner.is_alive()
    }

    #[track_caller]
    pub fn track(&self) {
        self.inner.track();
    }

    #[track_caller]
    pub fn notify(&self) {
        self.inner.notify();
    }
}

impl ReadSignal<()> for Trigger {
    fn track(&self) {
        self.track();
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&()) -> U) -> U {
        f(&())
    }
}

impl WriteSignal<()> for Trigger {
    fn notify(&self) {
        self.notify();
    }

    fn update_untracked<U>(&mut self, f: impl FnOnce(&mut ()) -> U) -> U {
        f(&mut ())
    }
}
