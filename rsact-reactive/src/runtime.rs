use crate::{
    callback::CallbackFn,
    computed::ComputedCallback,
    effect::EffectCallback,
    memo::MemoCallback,
    memo_chain::{MemoChainCallback, MemoChainErr},
    scope::{ScopeData, ScopeHandle, ScopeId},
    storage::{
        Storage, Value, ValueDebugInfo, ValueDebugInfoState, ValueId,
        ValueKind, ValueState,
    },
};
use ahash::RandomState;
use alloc::{
    boxed::Box,
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    rc::Rc,
    vec::Vec,
};
use core::{
    cell::{Cell, RefCell},
    fmt::Display,
    hash::Hash,
    marker::PhantomData,
    panic::Location,
};
use log::debug;
use slotmap::{SecondaryMap, SlotMap};

slotmap::new_key_type! {
    pub struct RuntimeId;
}

// TODO: Maybe better use Slab instead of SlotMap for efficiency

impl RuntimeId {
    pub fn leave(&self) {
        let rt = RUNTIMES.with(|rts| {
            rts.borrow_mut()
                .remove(*self)
                .expect("Attempt to leave non-existent runtime")
        });

        CURRENT_RUNTIME.with(|current| {
            if current.get() == Some(*self) {
                current.take();
            }
        });

        drop(rt);
    }
}

/// Run `f` with the **current** runtime and return its result.
///
/// Panics if no runtime is active on the current thread. This is the standard
/// way runtime-internal code accesses the singleton without holding a
/// long-lived borrow.
#[inline(always)]
#[track_caller]
pub fn with_current_runtime<T>(f: impl FnOnce(&Runtime) -> T) -> T {
    RUNTIMES.with(|rts| {
        let rts = rts.borrow();
        let current = CURRENT_RUNTIME.with(|current| current.get());
        let rt = rts.get(current.unwrap()).unwrap();
        f(rt)
    })
}

/// Create a **fresh** runtime, make it current, run `f`, then destroy it
/// and restore the previous runtime.
///
/// Primarily used in tests and benchmarks to get a clean isolated runtime
/// for each run without leaking state between calls.
///
/// ```rust
/// # use rsact_reactive::runtime::with_new_runtime;
/// # use rsact_reactive::prelude::*;
/// with_new_runtime(|_| {
///     let s = create_signal(42u32);
///     assert_eq!(s.get(), 42);
/// });
/// ```
#[inline(always)]
pub fn with_new_runtime<T>(f: impl FnOnce(&Runtime) -> T) -> T {
    let rt = create_runtime();

    CURRENT_RUNTIME.with(|current| {
        let prev = current.get();
        current.set(Some(rt));
        prev
    });

    let result = with_current_runtime(f);
    rt.leave();

    result
}

/// Create a new runtime and register it as the current runtime on this thread.
///
/// Returns a [`RuntimeId`] handle. Call [`RuntimeId::leave`] to destroy the
/// runtime and restore the previous one.  Prefer [`with_new_runtime`] for a
/// scoped version that restores the previous runtime automatically.
#[must_use]
#[inline(always)]
pub fn create_runtime() -> RuntimeId {
    RUNTIMES.with(|rts| rts.borrow_mut().insert(Runtime::new()))
}

crate::thread_local::thread_local_impl! {
    static CURRENT_RUNTIME: Cell<Option<RuntimeId>> = Cell::new(None);

    static RUNTIMES: RefCell<SlotMap<RuntimeId, Runtime>> = {
        let mut runtimes = SlotMap::default();

        #[cfg(feature = "default-runtime")]
        CURRENT_RUNTIME.with(|current| current.set(Some(runtimes.insert(Runtime::new()))));

        RefCell::new(runtimes)
    };
}

/// Defers effect flushing until the guard is dropped (or [`DeferEffectsGuard::run`] is called).
///
/// Obtained from [`defer_effects`]. Equivalent to a single nesting level of
/// [`batch`]: effects are only flushed when the outermost
/// `DeferEffectsGuard` is dropped.
///
/// Dropping the guard is equivalent to calling [`run`](DeferEffectsGuard::run).
#[non_exhaustive]
pub struct DeferEffectsGuard;

impl DeferEffectsGuard {
    pub fn run(self) {
        core::mem::drop(self);
    }
}

/// Obtain a [`DeferEffectsGuard`] that postpones effect flushing until dropped.
///
/// Prefer [`batch`] for the common case — `defer_effects` is the lower-level
/// primitive that `batch` is built on.
pub fn defer_effects() -> DeferEffectsGuard {
    with_current_runtime(|rt| rt.defer_effects())
}

impl Drop for DeferEffectsGuard {
    #[track_caller]
    fn drop(&mut self) {
        let caller = Location::caller();

        with_current_runtime(|rt| {
            let count = rt.defer_effects.get().saturating_sub(1);
            rt.defer_effects.set(count);
            // Only flush when the outermost batch ends
            if count == 0 {
                // TODO: Not an Option but Requester enum with DeferEffectsGuard?
                rt.run_effects(None, caller);
            }
        })
    }
}

// TODO: ObserverGuard to flatten callbacks into `start_observe` and `end_observe` (auto on drop)

/// Like [`observe`] but identifies the call-site by its source location
/// rather than an explicit key.  
/// Annotate the calling function with `#[track_caller]` to ensure each
/// distinct call-site is treated as a separate observer.
#[track_caller]
pub fn observe_by_location<R>(f: impl FnOnce() -> R) -> Option<R> {
    let location = Location::caller();

    with_current_runtime(|rt| {
        rt.use_observe(rt.hasher.hash_one(location), false, location, f)
    })
}

// TODO: Should observes be scoped? Like 1 { 2 {} } should not be the same observers as 2 { 1 {} } in the storage.
/// Run `f` identified by an arbitrary hashable key; re-runs only if reactive
/// dependencies from the previous call changed.
///
/// Returns `Some(result)` when `f` was executed this call, or `None` if
/// nothing changed since the last call with the same `id`.
///
/// Useful for code paths (e.g. render loops) that may be called many times
/// per frame and should only do work when their reactive inputs changed.
#[track_caller]
pub fn observe<H: Hash, R>(id: H, f: impl FnOnce() -> R) -> Option<R> {
    let location = Location::caller();
    with_current_runtime(|rt| {
        rt.use_observe(rt.hasher.hash_one(id), false, location, f)
    })
}

/// Observe version with `force` option to force execution even if no reactive dependencies changed.
#[track_caller]
pub fn observe_with_force<H: Hash, R>(
    id: H,
    force: bool,
    f: impl FnOnce() -> R,
) -> Option<R> {
    let location = Location::caller();
    with_current_runtime(|rt| {
        rt.use_observe(rt.hasher.hash_one(id), force, location, f)
    })
}

/// Run `f` without registering any reactive reads inside it as dependencies.
///
/// Signal/memo accesses inside `f` return their current value normally but
/// do not subscribe the active observer. Use this to read reactive state for
/// a side-effect without creating a dependency that would re-trigger it.
///
/// ```rust
/// # use rsact_reactive::prelude::*;
/// # use rsact_reactive::runtime::with_new_runtime;
/// # with_new_runtime(|_| {
/// let sig = create_signal(1u32);
/// let val = untrack(|| sig.get()); // read without tracking
/// assert_eq!(val, 1);
/// # });
/// ```
pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    let prev = with_current_runtime(|rt| rt.observer.take());
    let result = f();
    with_current_runtime(|rt| rt.observer.set(prev));
    result
}

/// Group all signal writes inside `f` into a single batch: effects are deferred
/// until `f` returns, then flushed exactly once.  Batches may be nested.
#[track_caller]
pub fn batch<T>(f: impl FnOnce() -> T) -> T {
    let _guard = defer_effects();
    f()
}

// TODO: Debug call-stack. Value get -> value get -> ... -> value get
/// The central reactive runtime that owns the dependency graph.
///
/// A `Runtime` holds all signals, memos, effects, scopes, and the
/// pending-effect queue.  You rarely interact with it directly;
/// free functions like [`with_current_runtime`], [`create_runtime`], and
/// [`batch`] reach into the current thread-local runtime for you.
///
/// The dependency graph is a directed bipartite graph:
/// - *sources* — signals/memos that produce values.
/// - *subscribers* — memos/effects that consume values.
///
/// When a source is written, all of its subscribers are marked dirty and
/// queued as pending effects.  Effects are flushed in topological order at
/// the end of the current batch (or immediately if no batch is active).
///
/// ```text
/// let memo = create_memo(move |_| signal.get())
/// ```
/// Here `signal` is a *source* of `memo`, and `memo` is a *subscriber* of `signal`.
#[derive(Default)]
pub struct Runtime {
    pub(crate) storage: Storage,
    scopes: RefCell<SlotMap<ScopeId, ScopeData>>,
    current_scope: Cell<Option<ScopeId>>,
    /// Values owned by observers.
    owned: RefCell<SecondaryMap<ValueId, BTreeSet<ValueId>>>,
    /// Current observer
    pub(crate) observer: Cell<Option<ValueId>>,
    /// Signals subscribers.
    pub(crate) subscribers: RefCell<SecondaryMap<ValueId, BTreeSet<ValueId>>>,
    /// Sources of signal changes. Signals that affect this observer (memo, effect, etc.).
    pub(crate) sources: RefCell<SecondaryMap<ValueId, BTreeSet<ValueId>>>,
    // TODO: Maybe use Vec or BTreeMap<Vec<>> so values are pre-sorted in topological order, so we don't need to sort them on every update?
    /// Effects to run after value changed or after [`DeferEffectsGuard`] runs/drops if defer_effects is enabled.
    pub(crate) pending_effects: RefCell<BTreeSet<ValueId>>,
    /// Nesting depth of active `batch()`/`defer_effects()` guards. Effects are deferred while > 0.
    pub(crate) defer_effects: Cell<u32>,
    /// Mapping from [`observe`] call location to its value id. Calling the same [`observe`] twice gives the same [`ValueId`]
    static_observers: RefCell<BTreeMap<u64, ValueId>>,
    hasher: RandomState,
}

impl Runtime {
    fn new() -> Self {
        Self {
            storage: Default::default(),
            scopes: Default::default(),
            current_scope: Default::default(),
            owned: Default::default(),
            subscribers: Default::default(),
            sources: Default::default(),
            observer: Default::default(),
            pending_effects: Default::default(),
            defer_effects: Cell::new(0),
            static_observers: Default::default(),
            hasher: RandomState::new(),
        }
    }

    pub fn is_alive(&self, id: ValueId) -> bool {
        self.storage.values.borrow().get(id).is_some()
    }

    #[must_use]
    pub fn new_scope(
        &self,
        #[cfg(feature = "debug-info")] created_at: &'static Location<'static>,
    ) -> ScopeHandle {
        let id = self.scopes.borrow_mut().insert(ScopeData::new(
            #[cfg(feature = "debug-info")]
            created_at,
        ));
        self.current_scope.set(Some(id));

        ScopeHandle::new(id)
    }

    pub fn new_deny_new_scope(
        &self,
        #[cfg(feature = "debug-info")] created_at: &'static Location<'static>,
    ) -> ScopeHandle {
        let id = self.scopes.borrow_mut().insert(ScopeData::new_deny_new(
            #[cfg(feature = "debug-info")]
            created_at,
        ));

        self.current_scope.set(Some(id));

        ScopeHandle::new(id)
    }

    fn add_value<T: 'static, DT: 'static>(
        &self,
        value: T,
        kind: ValueKind,
        initial_state: ValueState,
        _caller: &'static Location<'static>,
    ) -> ValueId {
        let mut scopes = self.scopes.borrow_mut();
        let scope = self
            .current_scope
            .get()
            .map(|current| scopes.get_mut(current))
            .flatten();

        let id = self.storage.add_value(Value {
            value: Rc::new(RefCell::new(value)),
            kind,
            state: initial_state,
            height: 0,
            #[cfg(feature = "debug-info")]
            debug: ValueDebugInfo {
                name: None,
                created_at: _caller,
                state: match initial_state {
                    ValueState::Clean => ValueDebugInfoState::Clean(None),
                    ValueState::Check => {
                        ValueDebugInfoState::CheckRequested(_caller, None)
                    },
                    ValueState::Dirty => {
                        ValueDebugInfoState::Dirten(_caller, None)
                    },
                },
                borrowed_mut: None,
                borrowed: None,
                ty: core::any::type_name::<DT>(),
                observer: None,
            },
        });

        if let Some(scope) = scope {
            if scope.deny_new {
                panic!(
                    "Creating new reactive values is disallowed in special `deny_new` scope. {scope}"
                );
            }

            scope.values.push(id);
        }

        if let Some(observer) = self.observer.get() {
            // Use entry API so the owned set is created on first use,
            // enabling proper owned-value tracking for effects/memos.
            // SecondaryMap::entry() returns Option<Entry<...>>.
            if let Some(entry) = self.owned.borrow_mut().entry(observer) {
                entry.or_insert_with(BTreeSet::new).insert(id);
            }
        }

        id
    }

    pub fn create_stored<T: 'static>(
        &self,
        value: T,
        _caller: &'static Location<'static>,
    ) -> ValueId {
        self.add_value::<_, T>(
            value,
            ValueKind::Stored,
            ValueState::Clean,
            _caller,
        )
    }

    pub fn create_inert<T: 'static>(
        &self,
        value: T,
        _caller: &'static Location<'static>,
    ) -> ValueId {
        self.add_value::<_, T>(
            value,
            ValueKind::Stored,
            ValueState::Clean,
            _caller,
        )
    }

    pub fn create_signal<T: 'static>(
        &self,
        value: T,
        _caller: &'static Location<'static>,
    ) -> ValueId {
        self.add_value::<_, T>(
            value,
            ValueKind::Signal,
            ValueState::Clean,
            _caller,
        )
    }

    /// Whether the value identified by `id` participates in reactivity, i.e. it
    /// is not an inert [`ValueKind::Stored`] value. Reactive-on-write wrappers
    /// use this to decide whether reading the value should subscribe the
    /// current observer.
    pub fn is_reactive(&self, id: ValueId) -> bool {
        self.storage
            .values
            .borrow()
            .get(id)
            .map(|value| !matches!(value.kind, ValueKind::Stored))
            .unwrap_or(false)
    }

    /// Upgrade an inert [`ValueKind::Stored`] value into a [`ValueKind::Signal`]
    /// in place, keeping the same [`ValueId`]. This is the reactive-on-write
    /// transition: because reactivity is keyed by `ValueId`, every existing
    /// handle to `id` becomes reactive at once. No-op if `id` is already
    /// reactive or absent.
    pub fn make_reactive(&self, id: ValueId) {
        let mut values = self.storage.values.borrow_mut();
        if let Some(value) = values.get_mut(id) {
            if matches!(value.kind, ValueKind::Stored) {
                value.kind = ValueKind::Signal;
            }
        }
    }

    pub fn create_effect<T, F>(
        &self,
        f: F,
        _caller: &'static Location<'static>,
    ) -> ValueId
    where
        T: 'static,
        F: FnMut(Option<T>) -> T + 'static,
    {
        self.add_value::<_, T>(
            None::<T>,
            ValueKind::Effect {
                f: Rc::new(RefCell::new(EffectCallback { f, ty: PhantomData })),
            },
            ValueState::Dirty,
            _caller,
        )
    }

    pub fn create_memo<T, F, P: 'static>(
        &self,
        f: F,
        _caller: &'static Location<'static>,
    ) -> ValueId
    where
        T: PartialEq + 'static,
        F: CallbackFn<T, P> + 'static,
    {
        self.add_value::<_, T>(
            None::<T>,
            ValueKind::Memo {
                f: Rc::new(RefCell::new(MemoCallback {
                    f,
                    ty: PhantomData,
                    p: PhantomData,
                })),
            },
            ValueState::Dirty,
            _caller,
        )
    }

    pub fn create_computed<T, F, P>(
        &self,
        f: F,
        _caller: &'static Location<'static>,
    ) -> ValueId
    where
        T: 'static,
        F: CallbackFn<T, P>,
        P: 'static,
    {
        self.add_value::<_, T>(
            None::<T>,
            ValueKind::Computed {
                f: Rc::new(RefCell::new(ComputedCallback {
                    f,
                    ty: PhantomData,
                    p: PhantomData,
                })),
            },
            ValueState::Dirty,
            _caller,
        )
    }

    pub fn create_memo_chain<T, F, P>(
        &self,
        f: F,
        _caller: &'static Location<'static>,
    ) -> ValueId
    where
        T: PartialEq + 'static,
        F: CallbackFn<T, P>,
        P: 'static,
    {
        self.add_value::<_, T>(
            None::<T>,
            ValueKind::MemoChain {
                memo: Rc::new(RefCell::new(MemoCallback {
                    f,
                    ty: PhantomData,
                    p: PhantomData,
                })),
                first: Rc::new(RefCell::new(None)),
                last: Rc::new(RefCell::new(None)),
            },
            ValueState::Dirty,
            _caller,
        )
    }

    fn use_observe<R>(
        &self,
        hash: u64,
        force: bool,
        location: &'static Location<'static>,
        f: impl FnOnce() -> R,
    ) -> Option<R> {
        let id = {
            let existing =
                *self.static_observers.borrow_mut().entry(hash).or_insert_with(
                    || {
                        self.add_value::<_, ()>(
                            (),
                            ValueKind::Observer,
                            ValueState::Dirty,
                            location,
                        )
                    },
                );

            // The observer may have been disposed by a parent observer's
            // cleanup (e.g. render_children re-running disposes owned child
            // render_part observers). In that case the stale id still exists
            // in static_observers but is no longer alive, so we must recreate
            // it; otherwise every subsequent observe call silently returns
            // None and the subtree is never redrawn.
            // TODO: This logic is basically wrong. As if parent observer cleanups its owned observers, then we create new dirty observer each time, leading to rerun each time. We need tests for this.
            if !self.is_alive(existing) {
                debug!("Reviving observe");
                let new_id = self.add_value::<_, ()>(
                    (),
                    ValueKind::Observer,
                    ValueState::Dirty,
                    location,
                );
                self.static_observers.borrow_mut().insert(hash, new_id);
                new_id
            } else {
                existing
            }
        };

        self.subscribe(id);
        // TODO: `maybe_update` call can be eliminated when `force=true` and just replaced with marking subscribers as dirty as we don't need to check deps.
        let updated = self.maybe_update(id, Some(id), location);

        if updated || force {
            let result = self.with_observer(id, |rt| {
                // TODO: Cleanup is wrong, we need to delete only values from the previous call, as we might delete nested observer
                // rt.cleanup(id);
                f()
            });

            Some(result)
        } else {
            None
        }
    }

    pub unsafe fn dispose(&self, id: ValueId) {
        // Collect owned children first so the borrow on `owned` is fully
        // released before any recursive dispose() call re-borrows it.
        let owned_children: Vec<ValueId> =
            self.owned.borrow_mut().remove(id).into_iter().flatten().collect();

        // Remove id from the subscriber set of every source it tracked.
        // Without this, disposed effects leave ghost entries that cause
        // mark_dirty to try to mark a dead value, panicking in storage.mark.
        {
            let mut subs = self.subscribers.borrow_mut();
            let sources = self.sources.borrow();
            for source in sources.get(id).into_iter().flatten().copied() {
                if let Some(set) = subs.get_mut(source) {
                    set.remove(&id);
                }
            }
        }

        // Remove id from the source set of every downstream node,
        // so stale sources don't linger in their cleanup lists.
        {
            let mut srcs = self.sources.borrow_mut();
            let subs = self.subscribers.borrow();
            for sub in subs.get(id).into_iter().flatten().copied() {
                if let Some(set) = srcs.get_mut(sub) {
                    set.remove(&id);
                }
            }
        }

        self.sources.borrow_mut().remove(id);
        self.subscribers.borrow_mut().remove(id);
        // TODO: Is it okay to remove from pending_effects?
        self.pending_effects.borrow_mut().remove(&id);
        self.storage
            .values
            .borrow_mut()
            .remove(id)
            .expect("Removing non-existent scope value");

        // Recursively dispose owned children now that all borrows are released.
        for child in owned_children {
            if self.is_alive(child) {
                unsafe { self.dispose(child) };
            }
        }
    }

    pub(crate) fn drop_scope(&self, scope_id: ScopeId) {
        // Release the borrow immediately so dispose() can run without conflicts.
        let scope_data = self.scopes.borrow_mut().remove(scope_id).unwrap();

        // TODO: Children scopes drop

        for id in scope_data.values {
            // Guard against double-dispose: a value may already have been
            // disposed as an owned child of another value in this scope.
            if self.is_alive(id) {
                unsafe { self.dispose(id) };
            }
        }
    }

    pub(crate) fn with_observer<T>(
        &self,
        observer: ValueId,
        f: impl FnOnce(&Self) -> T,
    ) -> T {
        let prev_observer = self.observer.get();

        self.observer.set(Some(observer));

        let result = f(self);

        self.observer.set(prev_observer);

        result
    }

    pub(crate) fn subscribe(&self, id: ValueId) {
        use alloc::borrow::BorrowMut as _;

        if let Some(observer) = self.observer.get() {
            if observer == id {
                panic!(
                    "Recursive subscription. Tried to subscribe observer to itself"
                );
            }

            {
                let mut sources = self.sources.borrow_mut();
                if let Some(sources) = sources.entry(observer) {
                    sources.or_default().borrow_mut().insert(id);
                }
            }

            {
                let mut subs = self.subscribers.borrow_mut();
                if let Some(subs) = subs.entry(id) {
                    subs.or_default().borrow_mut().insert(observer);
                }
            }

            self.update_height(observer);
        }
        // match self.observer.get() {
        //     Observer::None => panic!(
        //         "[BUG] Attempt to subscribe to reactive value updates out of
        // reactive context.",     ),
        //     Observer::Root => {
        //         // TODO: Add source?
        //         self.subscribers.borrow_mut().entry(id).or_default().
        // insert(Observer::Root);     },
        //     Observer::Effect(observer) => {
        //         self.sources
        //             .borrow_mut()
        //             .entry(observer)
        //             .or_default()
        //             .insert(id);

        //         self.subscribers
        //             .borrow_mut()
        //             .entry(id)
        //             .or_default()
        //             .insert(Observer::Effect(observer));
        //     },
        // }
    }

    pub(crate) fn maybe_update(
        &self,
        id: ValueId,
        requester: Option<ValueId>,
        caller: &'static Location<'static>,
    ) -> bool {
        if self.is(id, ValueState::Check) {
            let sources = {
                // TODO: Optimize out cloned sources set. Maybe alloc a Vec instead of using BTreeSet.
                let subs = self.sources.borrow();
                subs.get(id).cloned().into_iter().flatten()
            };
            for source in sources {
                // TODO: Should all sources by updates or we stop at the first change? If we stop at the first, why do even check if value could already be dirty?
                self.maybe_update(source, Some(source), caller);
                if self.is(id, ValueState::Dirty) {
                    // TODO: Cache check and use after break
                    break;
                }
            }
        }

        if self.is(id, ValueState::Dirty) {
            self.update(id, requester, caller);
            return true;
        }

        false
    }

    pub(crate) fn update(
        &self,
        id: ValueId,
        requester: Option<ValueId>,
        caller: &'static Location<'static>,
    ) {
        let value = self.storage.get(id);

        if let Some(value) = value {
            if self.defer_effects.get() > 0
                && matches!(value.kind, ValueKind::Effect { .. })
            {
                self.pending_effects.borrow_mut().insert(id);
                return;
            }

            let changed = match &value.kind {
                ValueKind::Stored => false,
                ValueKind::MemoChain { memo, first, last } => {
                    let value = value.value;

                    self.with_observer(id, move |rt| {
                        rt.cleanup(id);

                        let memo_changed = memo.borrow_mut().run(value.clone());

                        let first_changed = first
                            .borrow_mut()
                            .as_mut()
                            .map(|first| first.run(value.clone()))
                            .unwrap_or(false);
                        let last_changed = last
                            .borrow_mut()
                            .as_mut()
                            .map(|last| last.run(value.clone()))
                            .unwrap_or(false);

                        memo_changed || first_changed || last_changed
                    })
                },
                ValueKind::Computed { f }
                | ValueKind::Memo { f }
                | ValueKind::Effect { f } => {
                    let value = value.value;
                    self.with_observer(id, move |rt| {
                        rt.cleanup(id);

                        f.borrow_mut().run(value)
                    })
                },
                ValueKind::Signal { .. } => true,
                ValueKind::Observer => true,
            };

            if changed {
                if let Some(subs) = self.subscribers.borrow().get(id) {
                    for sub in subs {
                        // TODO: Shouldn't deep deps mark_dirty be used?
                        self.storage.mark(
                            *sub,
                            ValueState::Dirty,
                            requester,
                            caller,
                        );
                    }
                }
            }

            self.mark_clean(id, requester, caller);
        }
    }

    pub(crate) fn mark_clean(
        &self,
        id: ValueId,
        requester: Option<ValueId>,
        caller: &'static Location<'static>,
    ) {
        self.storage.mark(id, ValueState::Clean, requester, caller);
    }

    pub(crate) fn mark_dirty(
        &self,
        id: ValueId,
        requester: Option<ValueId>,
        caller: &'static Location<'static>,
    ) {
        self.mark_node(id, ValueState::Dirty, requester, caller);

        // TODO: Find other way to deal with recursive dependencies than BTreeSet?
        let mut deps = BTreeSet::new();
        Self::get_deep_deps(&self.subscribers.borrow(), &mut deps, id);
        for dep in deps {
            self.mark_node(dep, ValueState::Check, requester, caller);
        }
    }

    fn get_deep_deps(
        subscribers: &SecondaryMap<ValueId, BTreeSet<ValueId>>,
        deps: &mut BTreeSet<ValueId>,
        id: ValueId,
    ) {
        if let Some(subs) = subscribers.get(id) {
            for sub in subs {
                if !deps.contains(sub) {
                    deps.insert(*sub);
                    Self::get_deep_deps(subscribers, deps, *sub);
                }
            }
        }
    }

    fn mark_node(
        &self,
        id: ValueId,
        state: ValueState,
        requester: Option<ValueId>,
        caller: &'static Location<'static>,
    ) {
        if state > self.state(id) {
            self.storage.mark(id, state, requester, caller);
        }

        if let Some(node) = self.storage.get(id) {
            if let (ValueKind::Effect { .. }, true) =
                (node.kind, self.observer.get() != Some(id))
            {
                let mut pending_effects =
                    RefCell::borrow_mut(&self.pending_effects);
                pending_effects.insert(id);
            }
        }
    }

    // #[track_caller]
    // pub(crate) fn is_dirty(&self, id: ValueId) -> bool {
    //     self.storage.get(id).state == ValueState::Dirty
    // }

    // TODO: Explicitly return Option<ValueState> for disposed values.
    pub(crate) fn state(&self, id: ValueId) -> ValueState {
        self.storage
            .get(id)
            .map(|value| value.state)
            .unwrap_or(ValueState::Clean)
    }

    pub(crate) fn is(&self, id: ValueId, state: ValueState) -> bool {
        self.state(id) == state
    }

    #[cfg(feature = "debug-info")]
    pub fn debug_info(&self, id: ValueId) -> crate::storage::ValueDebugInfo {
        let debug_info = self.storage.debug_info(id).unwrap();

        // TODO: This is wrong, should not return current observer but subscribers list
        // if let Some(crate::storage::ValueDebugInfo {
        //     created_at: observer,
        //     ..
        // }) = self
        //     .observer
        //     .get()
        //     .map(|observer| self.storage.debug_info(observer))
        //     .flatten()
        // {
        //     debug_info.with_observer(observer)
        // } else {
        debug_info
        // }
    }

    #[cfg(feature = "debug-info")]
    pub fn observer_debug_info(&self) -> Option<ValueDebugInfo> {
        self.observer
            .get()
            .and_then(|observer| self.storage.debug_info(observer))
    }

    /// Generate mermaid graph containing all values in runtime.
    /// Be careful, this might be very expensive, use it only for debug purposes.
    #[cfg(feature = "debug-info")]
    pub fn global_mermaid_graph(
        &self,
        max_depth: usize,
    ) -> alloc::string::String {
        use alloc::{format, string::String};

        let mut visited = BTreeSet::new();
        let graph = { self.storage.values.borrow().keys().collect::<Vec<_>>() }
            .iter()
            .fold(String::new(), |graph, &id| {
                format!(
                    "{graph}\n{}",
                    self.mermaid_subgraph(id, 0, max_depth, &mut visited).1
                )
            });

        format!("graph TD\n{graph}")
    }

    /// Generate mermaid graph around the value.
    /// The center value node has a red border
    #[cfg(feature = "debug-info")]
    pub fn mermaid_graph(
        &self,
        id: ValueId,
        max_depth: usize,
    ) -> alloc::string::String {
        use alloc::format;

        let mut visited = BTreeSet::new();
        let (center_name, center_subgraph) =
            self.mermaid_subgraph(id, 0, max_depth, &mut visited);

        format!("graph TD\n{center_subgraph}\nstyle {center_name} stroke:#f55")
    }

    #[cfg(feature = "debug-info")]
    fn mermaid_subgraph(
        &self,
        id: ValueId,
        depth: usize,
        max_depth: usize,
        visited: &mut BTreeSet<ValueId>,
    ) -> (alloc::string::String, alloc::string::String) {
        use alloc::{
            format,
            string::{String, ToString},
        };

        let (name, decl, debug_info) = {
            let value = self.storage.get(id);
            if let Some(value) = value {
                let name = format!("{}{id}", value.kind);
                let (lp, rp, print_ty) = match &value.kind {
                    ValueKind::Stored => ("[", "]", true),
                    ValueKind::Signal => ("(", ")", true),
                    ValueKind::Effect { .. } => ("[[", "]]", true),
                    ValueKind::Memo { .. } => ("([", "])", true),
                    ValueKind::Computed { .. } => ("((", "))", true),
                    ValueKind::MemoChain { .. } => ("(((", ")))", true),
                    ValueKind::Observer => ("{", "}", false),
                };

                if visited.contains(&id) {
                    return (name, Default::default());
                }

                (
                    name.clone(),
                    format!(
                        "{name}{lp}\"{} {}{} ({})\"{rp}",
                        if let Some(name) = value.debug.name {
                            format!(" \'{name}\'")
                        } else {
                            "".to_string()
                        },
                        value.kind,
                        if print_ty {
                            format!(": {}", value.debug.ty)
                        } else {
                            "".to_string()
                        },
                        value.state.to_string()
                    ),
                    value.debug,
                )
            } else {
                // TODO: Better name than NULL
                return ("NULL".into(), format!(">NULL]"));
            }
        };

        visited.insert(id);

        if depth == max_depth {
            return (name, decl);
        }

        let state_change =
            if let ValueDebugInfoState::CheckRequested(
                _,
                Some((requester_id, _)),
            )
            | ValueDebugInfoState::Dirten(_, Some((requester_id, _)))
            | ValueDebugInfoState::Clean(Some((requester_id, _))) =
                debug_info.state
            {
                let (req_name, req_graph) = self.mermaid_subgraph(
                    requester_id,
                    depth + 1,
                    max_depth,
                    visited,
                );
                let arrow = if requester_id == id { "--" } else { "===" };
                format!(
                    "{req_graph}\n{req_name} {arrow}> |{}|{name}",
                    debug_info.state
                )
            } else {
                String::new()
            };

        let subs = {
            self.subscribers
                .borrow()
                .get(id)
                .cloned()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        };
        let subs = subs.into_iter().fold(String::new(), |subs, sub| {
            let (sub_name, sub_graph) =
                self.mermaid_subgraph(sub, depth + 1, max_depth, visited);
            format!("{subs}\n{sub_name} ===o |sub|{name}\n{sub_graph}")
        });

        let sources = {
            self.sources
                .borrow()
                .get(id)
                .cloned()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        };
        let sources =
            sources.into_iter().fold(String::new(), |sources, source| {
                let (source_name, source_graph) = self.mermaid_subgraph(
                    source,
                    depth + 1,
                    max_depth,
                    visited,
                );

                format!("{sources}\n{source_name} ===o |source|{name}\n{source_graph}")
            });

        (name, format!("{decl}\n{subs}\n{sources}\n{state_change}\n"))
    }

    fn cleanup(&self, id: ValueId) {
        {
            let sources = self.sources.borrow();
            if let Some(srcs) = sources.get(id) {
                // Remove `id` from the subscriber set of every source it previously tracked.
                let mut subs = self.subscribers.borrow_mut();
                for source in srcs {
                    if let Some(set) = subs.get_mut(*source) {
                        set.remove(&id);
                    }
                }
            }
        }

        // Clear the source list so heights are recomputed on next re-subscription.
        if let Some(srcs) = self.sources.borrow_mut().get_mut(id) {
            srcs.clear();
        }

        // FIXME: I am deleting the values created in this observer, but they could be leaked outside.
        // Collect and clear owned list before calling dispose to avoid a double
        // borrow of `owned` (dispose() also calls owned.borrow_mut()).
        let owned_snapshot: Vec<ValueId> = self
            .owned
            .borrow_mut()
            .get_mut(id)
            .map(|owned| {
                let v: Vec<_> = owned.iter().copied().collect();
                owned.clear();
                v
            })
            .unwrap_or_default();

        for child in owned_snapshot {
            if self.is_alive(child) {
                unsafe { self.dispose(child) };
            }
        }
    }

    pub(crate) fn defer_effects(&self) -> DeferEffectsGuard {
        self.defer_effects.set(self.defer_effects.get() + 1);
        DeferEffectsGuard
    }

    pub(crate) fn run_effects(
        &self,
        requester: Option<ValueId>,
        caller: &'static Location<'static>,
    ) {
        if self.defer_effects.get() > 0 {
            return;
        }

        // Loop until stable: running effects may write signals that queue more effects.
        loop {
            let pending = self.pending_effects.take();
            if pending.is_empty() {
                break;
            }

            // Sort by topological height so effects closer to source signals run first,
            // preventing glitches (an observer never sees a stale intermediate value).
            let mut sorted: Vec<ValueId> = pending.into_iter().collect();
            sorted.sort_unstable_by_key(|&id| self.storage.get_height(id));

            for effect in sorted {
                self.maybe_update(effect, requester, caller);
            }
        }
    }

    /// Recompute the topological height of `id` from its current sources and update storage.
    /// Height = max(height of sources) + 1.  Signals start at 0 (no sources).
    /// Called after every new subscription so pending effects are always sorted correctly.
    fn update_height(&self, id: ValueId) {
        let new_height = {
            let sources = self.sources.borrow();
            sources
                .get(id)
                .map(|srcs| {
                    srcs.iter()
                        .map(|s| self.storage.get_height(*s))
                        .max()
                        .map(|h| h + 1)
                        .unwrap_or(0)
                })
                .unwrap_or(0)
        };

        let old_height = self.storage.get_height(id);
        if new_height != old_height {
            self.storage.set_height(id, new_height);
        }
    }

    pub(crate) fn set_memo_chain<T: PartialEq + 'static>(
        &self,
        id: ValueId,
        is_first: bool,
        f: impl FnMut(&T) -> T + 'static,
    ) -> Result<(), MemoChainErr> {
        let kind = self.storage.get(id).unwrap().kind;
        match kind {
            ValueKind::MemoChain { memo: _, first, last } => {
                let mut func = if is_first {
                    first.borrow_mut()
                } else {
                    last.borrow_mut()
                };

                let redefined = func.replace(Box::new(MemoChainCallback {
                    f,
                    ty: PhantomData,
                }));

                // TODO: Location?
                if let Some(_) = redefined {
                    Err(if is_first {
                        MemoChainErr::FirstRedefined
                    } else {
                        MemoChainErr::LastRedefined
                    })
                } else {
                    Ok(())
                }
            },
            _ => panic!(
                "Cannot set memo {} chain for non-memo-chain value {kind}",
                if is_first { "first" } else { "last" }
            ),
        }
    }

    // pub(crate) fn add_memo_chain<T: PartialEq + 'static>(
    //     &self,
    //     id: ValueId,
    //     order: EffectOrder,
    //     map: impl Fn(&T) -> T + 'static,
    // ) {
    //     let kind = self.storage.get(id).unwrap().kind;
    //     match kind {
    //         ValueKind::MemoChain { memo: _, fs } => {
    //             fs.borrow_mut()
    //                 .entry(order)
    //                 .or_default()
    //                 .push(Rc::new(RefCell::new(MemoChainCallback::new(map))));
    //         },
    //         _ => panic!("Cannot add memo chain to {}", kind),
    //     }
    // }
}

pub fn current_runtime_profile() -> Profile {
    with_current_runtime(|rt| {
        let (stored, signals, effects, memos, computed, memo_chains) =
            rt.storage.values.borrow().values().fold(
                (0, 0, 0, 0, 0, 0),
                |(
                    mut stored,
                    mut signals,
                    mut effects,
                    mut memos,
                    mut computed,
                    mut memo_chains,
                ),
                 value| {
                    match &value.kind {
                        ValueKind::Stored => stored += 1,
                        ValueKind::Signal => signals += 1,
                        ValueKind::Effect { .. } => effects += 1,
                        ValueKind::Memo { .. } => memos += 1,
                        ValueKind::Computed { .. } => computed += 1,
                        ValueKind::MemoChain { .. } => memo_chains += 1,
                        ValueKind::Observer { .. } => {},
                    }

                    (stored, signals, effects, memos, computed, memo_chains)
                },
            );

        let subscribers_bindings =
            rt.subscribers.borrow().values().map(|subs| subs.len()).sum();
        let sources_bindings =
            rt.sources.borrow().values().map(|sources| sources.len()).sum();

        #[cfg(feature = "debug-info")]
        let top_by_subs = rt
            .subscribers
            .borrow()
            .iter()
            .map(|(id, subs)| (id, subs.len()))
            .max_by_key(|(_, subs)| *subs)
            .map(|(top_by_subs, subs)| {
                let created_at = rt
                    .storage
                    .values
                    .borrow()
                    .get(top_by_subs)
                    .unwrap()
                    .debug
                    .created_at;

                (created_at, subs)
            });

        #[cfg(feature = "debug-info")]
        let top_by_sources = rt
            .sources
            .borrow()
            .iter()
            .map(|(id, sources)| (id, sources.len()))
            .max_by_key(|(_, sources)| *sources)
            .map(|(top_by_sources, sources)| {
                let created_at = rt
                    .storage
                    .values
                    .borrow()
                    .get(top_by_sources)
                    .unwrap()
                    .debug
                    .created_at;
                (created_at, sources)
            });

        Profile {
            stored,
            signals,
            effects,
            memos,
            computed,
            memo_chains,
            subscribers: rt.subscribers.borrow().len(),
            subscribers_bindings,
            sources: rt.sources.borrow().len(),
            sources_bindings,
            pending_effects: rt.pending_effects.borrow().len(),
            #[cfg(feature = "debug-info")]
            top_by_subs,
            #[cfg(feature = "debug-info")]
            top_by_sources,
        }
    })
}

#[derive(Clone, Copy)]
pub struct Profile {
    stored: usize,
    signals: usize,
    effects: usize,
    memos: usize,
    computed: usize,
    memo_chains: usize,
    subscribers: usize,
    subscribers_bindings: usize,
    sources: usize,
    sources_bindings: usize,
    pending_effects: usize,
    #[cfg(feature = "debug-info")]
    top_by_subs: Option<(&'static Location<'static>, usize)>,
    #[cfg(feature = "debug-info")]
    top_by_sources: Option<(&'static Location<'static>, usize)>,
}

impl Display for Profile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(
            f,
            "{} values:",
            self.stored
                + self.signals
                + self.effects
                + self.memos
                + self.memo_chains
        )?;
        writeln!(f, "  {} stored", self.stored)?;
        writeln!(f, "  {} signals", self.signals)?;
        writeln!(f, "  {} effects", self.effects)?;
        writeln!(f, "  {} memos", self.memos)?;
        writeln!(f, "  {} computed", self.computed)?;
        writeln!(f, "  {} memo chains", self.memo_chains)?;
        writeln!(
            f,
            "{} subscribers ({} bindings), {} sources ({} bindings), {} pending effects",
            self.subscribers,
            self.subscribers_bindings,
            self.sources,
            self.sources_bindings,
            self.pending_effects
        )?;

        writeln!(f, "top values:")?;

        #[cfg(feature = "debug-info")]
        if let Some((top_by_subs, count)) = self.top_by_subs {
            writeln!(f, " by subscribers: {top_by_subs} ({count})")?;
        }

        #[cfg(feature = "debug-info")]
        if let Some((top_by_sources, count)) = self.top_by_sources {
            writeln!(f, " by sources: {top_by_sources} ({count})")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CURRENT_RUNTIME;
    use crate::{
        ReactiveValue as _,
        effect::create_effect,
        memo::create_memo,
        read::ReadSignal,
        runtime::{
            RUNTIMES, observe_by_location, with_current_runtime,
            with_new_runtime,
        },
        scope::new_scope,
        signal::create_signal,
        trigger::{Trigger, create_trigger},
        write::WriteSignal,
    };
    use alloc::rc::Rc;
    use core::cell::Cell;

    #[test]
    fn primary_runtime() {
        assert!(
            RUNTIMES.with(|rts| rts.borrow().contains_key(
                CURRENT_RUNTIME.with(|current| current.get().unwrap())
            )),
            "First insertion into RUNTIMES does not have key of RuntimeId::default()"
        );
    }

    #[test]
    fn check_observe() {
        let mut signal = create_signal(123);
        let runs_count = Rc::new(Cell::new(0));

        let runs = runs_count.clone();
        let run = move || {
            observe_by_location(|| {
                signal.get();
                runs.set(runs.get() + 1);
            })
        };

        assert_eq!(run(), Some(()));

        signal.set(2);

        assert_eq!(run(), Some(()));
        assert_eq!(run(), None);
        assert_eq!(run(), None);
        assert_eq!(run(), None);
        assert_eq!(run(), None);

        assert_eq!(runs_count.get(), 2);
    }

    #[test]
    fn observe_works_with_memos() {
        let mut calls = create_signal(0);
        let mut a = create_signal(0);
        let a_is_even = create_memo(move || a.get() % 2 == 0);

        // Run observe only for even `a` values
        let mut run = move || {
            observe_by_location(|| {
                calls.update_untracked(|calls| *calls += 1);
                a_is_even.get();
            })
        };

        assert_eq!(run(), Some(()), "observe didn't runs first time");
        assert_eq!(a_is_even.get(), true);
        assert_eq!(calls.get(), 1);
        assert_eq!(run(), None);

        a.set(3);
        assert_eq!(run(), Some(()), "observe didn't run on value change");
        assert_eq!(a_is_even.get(), false);
        assert_eq!(calls.get(), 2);
        assert_eq!(run(), None);

        // `a` is still odd, so observe shouldn't rerun
        a.set(5);
        assert_eq!(run(), None, "observe rerun on unchanged memo");
        assert_eq!(a_is_even.get(), false);
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn recursive_observe() {
        let mut signal = create_signal(123);

        let mut run = move || {
            observe_by_location(|| {
                signal.get();
                signal.set(69);
            })
        };

        assert_eq!(run(), Some(()));
        signal.set(0);
        assert_eq!(run(), Some(()));
        assert_eq!(run(), None);
    }

    #[test]
    fn nested_observe() {
        let mut signal = create_signal(123);

        let run = move || {
            observe_by_location(move || {
                observe_by_location(|| {
                    observe_by_location(|| {
                        signal.get();
                        signal.set(69);
                    });
                });
            })
        };

        assert_eq!(run(), Some(()));
        signal.set(0);
        assert_eq!(run(), Some(()));
        assert_eq!(run(), None);
    }

    /// dispose() must remove the effect from the source signal's subscriber
    /// set so that subsequent signal writes don't try to notify a dead effect.
    #[test]
    fn dispose_cleans_up_subscribers_in_source() {
        with_new_runtime(|rt| {
            let mut sig = create_signal(0i32);
            {
                let _scope = new_scope();
                let s = sig;
                create_effect(move |_: Option<()>| {
                    s.get();
                });
            }
            // Effect disposed. The signal's subscribers map must be empty —
            // confirmed by checking that no subscribers remain.
            let sig_id = sig.id().unwrap();
            let subs_count = rt
                .subscribers
                .borrow()
                .get(sig_id)
                .map(|s| s.len())
                .unwrap_or(0);
            assert_eq!(subs_count, 0, "ghost subscriber left after dispose");
        });
    }

    /// dispose() must not add the effect to pending_effects after it is gone.
    #[test]
    fn dispose_removes_from_pending_effects() {
        with_new_runtime(|rt| {
            let mut sig = create_signal(0i32);
            let _scope = {
                let scope = new_scope();
                let s = sig;
                create_effect(move |_: Option<()>| {
                    s.get();
                });
                scope
                // keep scope alive so we can drop it explicitly below
            };
            drop(_scope); // dispose the effect

            // No dead ids should linger in pending_effects.
            sig.set(1); // triggers mark_dirty, which would queue effects
            let pending = rt.pending_effects.borrow();
            for &id in pending.iter() {
                assert!(
                    rt.is_alive(id),
                    "dead ValueId {id:?} in pending_effects after dispose"
                );
            }
        });
    }

    /// After an effect re-runs, stale subscriptions from its previous run
    /// should be removed from the source signals' subscriber sets (cleanup).
    #[test]
    fn cleanup_removes_stale_subscriptions() {
        with_new_runtime(|rt| {
            let mut condition = create_signal(true);
            let mut a = create_signal(1i32);
            let mut b = create_signal(2i32);
            let reads = Rc::new(Cell::new(0u32));

            let reads_eff = reads.clone();
            create_effect(move |_: Option<()>| {
                reads_eff.set(reads_eff.get() + 1);
                if condition.get() {
                    a.get();
                } else {
                    b.get();
                }
            });

            assert_eq!(reads.get(), 1);

            // Switch condition: effect now reads b instead of a.
            condition.set(false);
            assert_eq!(reads.get(), 2);

            // `a` should have no subscribers now (cleanup removed the stale sub).
            let a_id = a.id().unwrap();
            let a_subs =
                rt.subscribers.borrow().get(a_id).map(|s| s.len()).unwrap_or(0);
            assert_eq!(
                a_subs, 0,
                "stale subscription to `a` not removed after cleanup"
            );

            // Writing `a` must not trigger the effect (it no longer depends on it).
            let before = reads.get();
            a.set(99);
            assert_eq!(
                reads.get(),
                before,
                "effect re-ran on stale dependency `a`"
            );
        });
    }

    /// When a parent observer re-runs its closure, `cleanup` disposes all
    /// reactive values that were *owned* by it (created while it was the active
    /// observer). Child `observe` calls are identified by a stable hash stored
    /// in `static_observers`. After cleanup the stored `ValueId` is dead, but
    /// the hash entry remains. Without an aliveness check `use_observe` would
    /// silently reuse the dead id, skip re-running the child closure.
    #[test]
    fn observe_recreates_disposed_child_observer() {
        let mut trigger = create_trigger();
        // Tracks how many times the *inner* observe ran.
        let inner_runs = Rc::new(Cell::new(0u32));

        let run = |trigger: &mut Trigger, inner_runs: Rc<Cell<u32>>| {
            // Outer observe – acts like `render_children`.
            observe_by_location(move || {
                trigger.track();
                // Inner observe – acts like a child `render_part`. The id is
                // derived from the call-site location, which is stable across
                // repeated invocations of the outer closure.
                let r = inner_runs.clone();
                observe_by_location(move || {
                    r.set(r.get() + 1);
                });
            })
        };

        // First call: both outer and inner run.
        assert_eq!(run(&mut trigger, inner_runs.clone()), Some(()));
        assert_eq!(inner_runs.get(), 1);

        // No change – neither outer nor inner should re-run.
        assert_eq!(run(&mut trigger, inner_runs.clone()), None);
        assert_eq!(inner_runs.get(), 1);

        // Notify trigger: outer is dirtied, its cleanup disposes the inner
        // observer. `use_observe` must recreate the inner observer so the
        // closure runs again.
        trigger.notify();
        assert_eq!(run(&mut trigger, inner_runs.clone()), Some(()));
        assert_eq!(
            inner_runs.get(),
            2,
            "inner observe did not re-run after parent cleanup disposed it"
        );

        // Stable again after re-run.
        assert_eq!(run(&mut trigger, inner_runs.clone()), None);
        assert_eq!(inner_runs.get(), 2);
    }
}
