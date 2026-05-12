use core::{
    cell::{Cell, RefCell},
    task::{Context, Poll, Waker},
};

/// The lifecycle state of an asynchronously-computed value.
///
/// Transitions: `Uninitialized` → `Loading` → `Ready(T)`.
/// When the reactive source changes, the state goes back to `Loading` while
/// the new fetch is in progress.
#[derive(Clone, PartialEq)]
pub enum AsyncState<T> {
    /// No fetch has been started yet (before the first reactive source read).
    Uninitialized,
    /// A fetch is currently in progress.
    Loading,
    /// The most recent fetch completed successfully.
    Ready(T),
}

impl<T> AsyncState<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Self::Ready(v) => Some(v),
            _ => None,
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    pub fn is_uninitialized(&self) -> bool {
        matches!(self, Self::Uninitialized)
    }
}

/// Waker-based bridge between the synchronous reactive graph and an async driver future.
///
/// The reactive system calls [`notify`] when a source changes; the async driver
/// calls [`poll_wait`] to suspend until the next notification.
///
/// This type is executor-agnostic: it uses the standard `core::task::Waker`
/// mechanism, so any conforming executor (Embassy, tokio, smol, manual polling)
/// can drive the associated future.
pub struct AsyncNotify {
    waker: RefCell<Option<Waker>>,
    pending: Cell<bool>,
}

impl AsyncNotify {
    /// Creates a new `AsyncNotify`.
    ///
    /// Starts with `pending = true` so the driver's first poll immediately
    /// proceeds to fetch the initial value rather than suspending.
    pub fn new() -> Self {
        Self {
            waker: RefCell::new(None),
            // Start pending so driver runs immediately on first poll
            pending: Cell::new(true),
        }
    }

    /// Called by the sync reactive system when a source signal changes.
    ///
    /// Marks this notifier as pending and wakes the driver future if it is
    /// currently suspended.
    pub fn notify(&self) {
        self.pending.set(true);
        if let Some(waker) = self.waker.borrow().as_ref() {
            waker.wake_by_ref();
        }
    }

    /// Called inside the driver future's `poll` to wait for the next notification.
    ///
    /// Returns `Poll::Ready(())` if a notification is pending (clearing it);
    /// otherwise registers the waker and returns `Poll::Pending`.
    pub fn poll_wait(&self, cx: &mut Context<'_>) -> Poll<()> {
        // Always update the waker — the executor may provide a new one each poll.
        *self.waker.borrow_mut() = Some(cx.waker().clone());
        if self.pending.get() {
            self.pending.set(false);
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
