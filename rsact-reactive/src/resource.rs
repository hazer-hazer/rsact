use crate::{
    ReactiveValue, async_rt::{AsyncNotify, AsyncState}, effect::create_effect, read::ReadSignal, signal::{Signal, create_signal}, write::WriteSignal
};
use alloc::{boxed::Box, rc::Rc};
use core::{
    cell::{Cell, RefCell},
    future::{Future, poll_fn},
    pin::Pin,
};

/// A reactive handle to an asynchronously-computed value.
///
/// `Resource<T>` wraps a `Signal<AsyncState<T>>` and participates in the
/// reactive graph just like any other signal: any effect or memo that reads it
/// will re-run when the async state changes.
///
/// Create a resource with [`create_resource`].
pub struct Resource<T: 'static> {
    signal: Signal<AsyncState<T>>,
}

impl<T: Clone + 'static> Resource<T> {
    /// Returns the inner value if the resource is [`AsyncState::Ready`], else `None`.
    pub fn ready(&self) -> Option<T> {
        self.signal.with(|s| s.ready().cloned())
    }

    /// Returns `true` if a fetch is currently in progress.
    pub fn is_loading(&self) -> bool {
        self.signal.with(|s| s.is_loading())
    }

    /// Returns `true` if no fetch has started yet.
    pub fn is_uninitialized(&self) -> bool {
        self.signal.with(|s| s.is_uninitialized())
    }
}

impl<T: 'static> ReactiveValue for Resource<T> {
    type Value = T;

    fn id(&self) -> Option<crate::storage::ValueId> {
        Some(self.signal.id())
    }

    fn is_alive(&self) -> bool {
        self.signal.is_alive()
    }

    unsafe fn dispose(self) {
        self.signal.dispose();
    }
}

impl<T: 'static> ReadSignal<AsyncState<T>> for Resource<T> {
    fn track(&self) {
        self.signal.track();
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&AsyncState<T>) -> U) -> U {
        self.signal.with_untracked(f)
    }
}

/// Creates a reactive [`Resource`] whose value is produced by an async fetch.
///
/// Returns `(resource, driver)`:
///
/// - **`resource`** is a reactive handle with a current [`AsyncState<T>`].
///   It transitions `Uninitialized → Loading → Ready(T)` as the fetch
///   progresses, and back to `Loading` whenever `source_fn` returns a new
///   value. Any reactive context (effect, memo) that reads `resource` will
///   re-run when the state changes.
///
/// - **`driver`** is a `Future<Output = ()>` that must be driven by the
///   caller's executor. The driver runs forever, re-fetching whenever the
///   reactive source changes.
///
/// # Executor integration
///
/// The driver is a plain, executor-agnostic `Future`. How you run it depends
/// entirely on your environment:
///
/// ```rust,ignore
/// // Embassy — wrap in a task (driver is !Send, which is fine on single-core)
/// #[embassy_executor::task]
/// async fn run_my_resource(driver: impl Future<Output = ()>) { driver.await; }
/// spawner.spawn(run_my_resource(driver));
///
/// // Bare-metal main loop with embassy-futures
/// pin_mut!(driver);
/// loop {
///     embassy_futures::poll_once(driver.as_mut());
///     // ... other work
/// }
/// ```
///
/// # Cancellation
///
/// When `source_fn` returns a new value, the current in-flight fetch is
/// dropped immediately (Rust's cooperative cancellation). A generation counter
/// prevents any stale result that was already computing from being committed.
pub fn create_resource<T, S, SF, F, Fut>(
    source_fn: SF,
    fetcher: F,
) -> (Resource<T>, Pin<Box<dyn Future<Output = ()> + 'static>>)
where
    T: 'static,
    S: Clone + 'static,
    SF: Fn() -> S + 'static,
    F: Fn(S) -> Fut + 'static,
    Fut: Future<Output = T> + 'static,
{
    let mut signal = create_signal(AsyncState::Uninitialized);
    let notify = Rc::new(AsyncNotify::new());
    let current_source: Rc<RefCell<Option<S>>> = Rc::new(RefCell::new(None));
    let generation: Rc<Cell<u32>> = Rc::new(Cell::new(0));

    // Sync reactive effect: tracks source_fn(), stores the result, and wakes
    // the driver every time the source changes. This runs once immediately on
    // creation, which sets `notify` to pending so the driver fires its first
    // fetch as soon as it is first polled.
    let notify_eff = notify.clone();
    let source_eff = current_source.clone();
    let gen_eff = generation.clone();

    create_effect(move |_: Option<()>| {
        let src = source_fn();
        *source_eff.borrow_mut() = Some(src);
        gen_eff.set(gen_eff.get().wrapping_add(1));
        notify_eff.notify();
    });

    // Async driver: loops forever, suspending between source changes and
    // running the user-supplied async fetcher each time.
    let driver: Pin<Box<dyn Future<Output = ()> + 'static>> =
        Box::pin(async move {
            loop {
                // Suspend until the reactive effect notifies us of a change.
                poll_fn(|cx| notify.poll_wait(cx)).await;

                // Exit gracefully if the resource signal was disposed (e.g.
                // its owning scope was dropped) while the driver was suspended.
                // Without this guard, calling signal.set() on a dead ValueId
                // would panic.
                if !signal.is_alive() {
                    break;
                }

                let src = match current_source.borrow().clone() {
                    Some(s) => s,
                    None => continue,
                };

                let current_gen = generation.get();
                signal.set(AsyncState::Loading);

                let value = fetcher(src).await;

                // Guard against stale results: if the source changed while we
                // were fetching, the generation counter will have advanced.
                // Also re-check liveness: a side-effect triggered by the
                // Loading transition above could have disposed the signal.
                if generation.get() == current_gen && signal.is_alive() {
                    signal.set(AsyncState::Ready(value));
                }
            }
        });

    (Resource { signal }, driver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use alloc::rc::Rc;
    use core::{
        cell::Cell,
        future::Future,
        pin::pin,
        task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    };

    // Minimal no-op waker for manually polling futures in tests.
    fn noop_waker() -> Waker {
        const VTABLE: RawWakerVTable = RawWakerVTable::new(
            |p| RawWaker::new(p, &VTABLE),
            |_| {},
            |_| {},
            |_| {},
        );
        unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
    }

    /// Poll a future to completion, up to `limit` iterations.
    /// Panics if the future does not resolve within the limit.
    fn drive<F: Future>(
        mut fut: core::pin::Pin<&mut F>,
        limit: usize,
    ) -> F::Output {
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);
        for _ in 0..limit {
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(v) => return v,
                Poll::Pending => {},
            }
        }
        panic!("future did not complete within {limit} polls");
    }

    #[test]
    fn resource_initial_fetch() {
        with_new_runtime(|_| {
            let (resource, driver) =
                create_resource(|| 42u32, |n| async move { n * 2 });

            // Before driving: Uninitialized
            assert!(resource.is_uninitialized());

            let mut driver = pin!(driver);
            let waker = noop_waker();
            let mut cx = Context::from_waker(&waker);

            // First poll: effect has already notified, so driver proceeds past
            // the `poll_fn` immediately, sets Loading, then awaits the fetcher.
            // The fetcher (`async { n*2 }`) is ready immediately, so the whole
            // thing resolves in one pass.
            let _ = driver.as_mut().poll(&mut cx);

            assert_eq!(resource.ready(), Some(84u32));
        });
    }

    #[test]
    fn resource_refetches_on_source_change() {
        with_new_runtime(|_| {
            let mut source = create_signal(1u32);

            let (resource, driver) = create_resource(
                move || source.get(),
                |n| async move { n * 10 },
            );

            let mut driver = pin!(driver);
            let waker = noop_waker();
            let mut cx = Context::from_waker(&waker);

            // First poll — fetches for source = 1
            let _ = driver.as_mut().poll(&mut cx);
            assert_eq!(resource.ready(), Some(10u32));

            // Change source
            source.set(2);

            // Notify was called synchronously by the effect; poll again.
            let _ = driver.as_mut().poll(&mut cx);
            assert_eq!(resource.ready(), Some(20u32));
        });
    }

    #[test]
    fn resource_stale_result_discarded() {
        // When the source changes twice in quick succession, only the result
        // matching the latest generation should be committed.
        with_new_runtime(|_| {
            let mut source = create_signal(1u32);
            let committed = Rc::new(Cell::new(0u32));

            let committed_clone = committed.clone();
            let (resource, driver) = create_resource(
                move || source.get(),
                move |n| {
                    let c = committed_clone.clone();
                    async move {
                        c.set(c.get() + 1);
                        n * 10
                    }
                },
            );

            let mut driver = pin!(driver);
            let waker = noop_waker();
            let mut cx = Context::from_waker(&waker);

            // First fetch
            let _ = driver.as_mut().poll(&mut cx);
            assert_eq!(resource.ready(), Some(10u32));

            // Rapidly change source twice before next poll
            source.set(2);
            source.set(3);

            // Poll once — only the latest source value (3) should be used.
            // Generation is now 3 (initial + 2 changes); the driver reads
            // generation=3, fetches 3*10=30, and gen still matches → commit.
            let _ = driver.as_mut().poll(&mut cx);
            assert_eq!(resource.ready(), Some(30u32));
        });
    }
}
