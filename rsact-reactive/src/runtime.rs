use crate::{
    callback::CallbackFn,
    computed::ComputedCallback,
    effect::EffectCallback,
    memo::MemoCallback,
    scope::{ScopeData, ScopeHandle, ScopeId},
    storage::{Storage, Value, ValueId, ValueKind, ValueKindTag, ValueState},
};
use alloc::{rc::Rc, vec::Vec};
// Only the `debug-info` mermaid-graph helpers below still use a `BTreeSet`; the
// hot path's pending-effect queue is now a height-bucketed vec (WS9a.1), so this
// import (and the BTree code it pulls in) stays out of release/embedded builds.
#[cfg(feature = "debug-info")]
use alloc::collections::btree_set::BTreeSet;
use core::{
    cell::{Cell, RefCell},
    fmt::Display,
    marker::PhantomData,
    panic::Location,
};
use slotmap::{SecondaryMap, SlotMap};
use tinyvec::TinyVec;

slotmap::new_key_type! {
    pub struct RuntimeId;
}

/// Inline-small vector of value ids used for the dependency-graph edge sets
/// (`sources`/`subscribers`/`owned`). Fan-in and fan-out are almost always
/// tiny, so the common case stays on the stack with no per-node heap
/// allocation; large fan-outs spill to the heap exactly once. Order is not
/// significant — topological order for effect flushing comes from `height`.
type IdVec = TinyVec<[ValueId; 4]>;

/// Remove the first occurrence of `id` from an [`IdVec`] by swap-remove
/// (order-independent).
#[inline]
fn id_vec_remove(v: &mut IdVec, id: ValueId) {
    if let Some(pos) = v.iter().position(|x| *x == id) {
        v.swap_remove(pos);
    }
}

// TODO: Maybe better use Slab instead of SlotMap for efficiency

#[cfg(any(test, feature = "test-utils"))]
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

// TODO: Will we support multi-runtime? If so, ValueId needs to be compound value ID + runtime ID. If no, we need to hide with_new_runtime and new runtime creation from exposed API as it is dangerous.
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
#[cfg(any(test, feature = "test-utils"))]
#[inline(always)]
pub fn with_new_runtime<T>(f: impl FnOnce(&Runtime) -> T) -> T {
    // Restore the previously-current runtime on the way out — even if `f`
    // panics — so a nested `with_new_runtime` (or any temporary runtime) can't
    // brick the current-runtime cell (WS1.2). The old code discarded `prev`,
    // and `leave()` merely clears the current cell, so after the temporary
    // runtime left the current cell was `None` and every subsequent reactive
    // access panicked on `current.unwrap()`.
    struct RuntimeGuard {
        rt: RuntimeId,
        prev: Option<RuntimeId>,
    }
    impl Drop for RuntimeGuard {
        fn drop(&mut self) {
            self.rt.leave();
            CURRENT_RUNTIME.with(|current| current.set(self.prev));
        }
    }

    let rt = create_runtime();
    let prev = CURRENT_RUNTIME.with(|current| {
        let prev = current.get();
        current.set(Some(rt));
        prev
    });
    let _guard = RuntimeGuard { rt, prev };

    with_current_runtime(f)
}

/// Create a new runtime and register it as the current runtime on this thread.
///
/// Returns a [`RuntimeId`] handle. Call [`RuntimeId::leave`] to destroy the
/// runtime and restore the previous one.  Prefer [`with_new_runtime`] for a
/// scoped version that restores the previous runtime automatically.
#[cfg(any(test, feature = "test-utils"))]
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

/// Defers effect flushing until the guard is dropped (or
/// [`DeferEffectsGuard::run`] is called).
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
                // TODO: Not an Option but Requester enum with
                // DeferEffectsGuard?
                rt.run_effects(None, caller);
            }
        })
    }
}

// TODO: ObserverGuard to flatten callbacks into `start_observe` and
// `end_observe` (auto on drop)

// The keyed `observe()` family and its global `static_observers` registry were
// removed in WS2: render identity is now the owned [`crate::probe::Probe`]
// handle (its owner — e.g. `rsact-ui`'s `ElState` — stores it), which makes
// cross-context aliasing impossible by construction and lets the node die with
// its owner. `ahash` left the build with the registry's call-site hashing.

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
    // Restore the previous observer on the way out — even if `f` panics —
    // via an RAII guard, so a panic inside `f` cannot leave the runtime's
    // observer cell stuck at `None` and corrupt all subsequent tracking.
    struct RestoreObserver(Option<ValueId>);
    impl Drop for RestoreObserver {
        fn drop(&mut self) {
            with_current_runtime(|rt| rt.observer.set(self.0));
        }
    }

    let _restore =
        RestoreObserver(with_current_runtime(|rt| rt.observer.take()));
    f()
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
/// Here `signal` is a *source* of `memo`, and `memo` is a *subscriber* of
/// `signal`.
#[derive(Default)]
pub struct Runtime {
    pub(crate) storage: Storage,
    scopes: RefCell<SlotMap<ScopeId, ScopeData>>,
    current_scope: Cell<Option<ScopeId>>,
    /// Values owned by observers.
    owned: RefCell<SecondaryMap<ValueId, IdVec>>,
    /// Current observer
    pub(crate) observer: Cell<Option<ValueId>>,
    /// Signals subscribers.
    pub(crate) subscribers: RefCell<SecondaryMap<ValueId, IdVec>>,
    /// Sources of signal changes. Signals that affect this observer (memo,
    /// effect, etc.).
    pub(crate) sources: RefCell<SecondaryMap<ValueId, IdVec>>,
    /// Effects to run after a value changed (or after a [`DeferEffectsGuard`]
    /// runs/drops if `defer_effects` is enabled). Height-bucketed so draining
    /// ascending is already topological — no per-flush sort — and its buffers
    /// are reused across flushes, so a steady-state write queues an effect with
    /// zero heap allocation (WS9a.1, replacing a per-write `BTreeSet` node +
    /// collected sort `Vec`).
    pub(crate) pending_effects: RefCell<EffectQueue>,
    /// Nesting depth of active `batch()`/`defer_effects()` guards. Effects are
    /// deferred while > 0.
    pub(crate) defer_effects: Cell<u32>,
    /// Reused worklist for the iterative `mark_dirty` transitive walk. Cleared
    /// (not reallocated) each write, so after warm-up it does not allocate.
    mark_stack: RefCell<Vec<ValueId>>,
    /// Per-node "last seen" generation for the `mark_dirty` walk's dedup.
    /// Bumping [`mark_gen`](Runtime::mark_gen) invalidates every entry in O(1)
    /// — no `clear()` (which would be O(capacity)) and no per-write allocation.
    /// `u64` so the generation never wraps in any realistic device lifetime.
    mark_seen: RefCell<SecondaryMap<ValueId, u64>>,
    mark_gen: Cell<u64>,
    /// True while [`run_effects`](Runtime::run_effects) is draining the pending
    /// queue. A signal written *by an effect* during a flush re-enters
    /// `run_effects`; that nested call returns early and lets the outermost
    /// flush loop drain the newly-queued effects. This keeps effect execution
    /// iterative (no per-write recursion → no stack growth on effect cascades)
    /// and turns a dependency cycle into a bounded, logged loop instead of a
    /// stack overflow.
    flushing: Cell<bool>,
}

/// The pending-effect queue (WS9a.1): effects awaiting a flush, bucketed by
/// topological height so draining low→high is already glitch-free order with no
/// per-flush sort.
///
/// Replaces a `BTreeSet<ValueId>` that allocated a tree node on the first insert
/// of every write and was then `collect`ed into a fresh `Vec` to sort each
/// flush round (2 allocs / 112 B per write with ≥1 effect). Here:
/// - `pending` holds queued effects indexed by height;
/// - `draining` is swapped with `pending` at the start of each round so effects
///   queued *while effects run* accumulate for the next round — the
///   allocation-free equivalent of the old `.take()`;
/// - `queued` is an O(1) membership set giving the old set-dedup semantics
///   (load-bearing when batched writes mark the same effect repeatedly);
/// - all buffers retain capacity across flushes, so after warm-up a write
///   queues an effect with zero heap allocation. Peak-sized capacity is the
///   deterministic RAM embedded targets want.
///
/// Invariant: `draining` is empty between rounds (each round drains it fully);
/// an id is stamped in `queued` iff it sits in `pending` (a `draining` id is
/// unstamped the moment it is popped, so a cascade can re-queue it).
#[derive(Default)]
pub(crate) struct EffectQueue {
    pending: Vec<Vec<ValueId>>,
    draining: Vec<Vec<ValueId>>,
    queued: SecondaryMap<ValueId, ()>,
    len: usize,
}

impl EffectQueue {
    /// Queue `effect` at its topological `height`. No-op if already pending
    /// (dedup). Allocates only while the buckets/`queued` map are still growing
    /// to their high-water mark; steady-state writes queue for free.
    fn push(&mut self, effect: ValueId, height: u32) {
        if self.queued.insert(effect, ()).is_some() {
            return;
        }
        let h = height as usize;
        if h >= self.pending.len() {
            self.pending.resize_with(h + 1, Vec::new);
        }
        self.pending[h].push(effect);
        self.len += 1;
    }

    /// Remove `effect` from the queue if pending (dispose path). Rare, so a
    /// linear bucket scan is fine. A `draining` (mid-flush) effect is left in
    /// place — it is already unstamped and drains as a harmless no-op once its
    /// storage entry is gone.
    fn remove(&mut self, effect: ValueId) {
        if self.queued.remove(effect).is_none() {
            return;
        }
        for bucket in &mut self.pending {
            if let Some(pos) = bucket.iter().position(|&id| id == effect) {
                bucket.swap_remove(pos);
                self.len -= 1;
                return;
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Effects currently queued for the next flush (used by `Profile`).
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Begin a flush round: move the accumulated `pending` into `draining` to be
    /// run, leaving `pending` empty for effects queued during the round. O(1),
    /// no allocation (`draining` is empty here per the type invariant).
    fn start_round(&mut self) {
        core::mem::swap(&mut self.pending, &mut self.draining);
        self.len = 0;
    }

    /// Pop the next effect of this round's snapshot, lowest height first, and
    /// unstamp it so a cascade may re-queue it for a later round. `cursor` is
    /// the caller's monotonic ascending-height cursor; returns `None` when the
    /// snapshot is exhausted (all `draining` buckets empty).
    fn pop_next(&mut self, cursor: &mut usize) -> Option<ValueId> {
        while *cursor < self.draining.len() {
            if let Some(id) = self.draining[*cursor].pop() {
                self.queued.remove(id);
                return Some(id);
            }
            *cursor += 1;
        }
        None
    }

    /// Discard everything (cycle-abort path): clear both buffers and the dedup
    /// set, retaining capacity.
    fn clear(&mut self) {
        for bucket in &mut self.pending {
            bucket.clear();
        }
        for bucket in &mut self.draining {
            bucket.clear();
        }
        self.queued.clear();
        self.len = 0;
    }

    /// Ids currently pending — for tests asserting no dead ids linger.
    #[cfg(test)]
    fn queued_ids(&self) -> impl Iterator<Item = ValueId> + '_ {
        self.queued.keys()
    }
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
            mark_stack: Default::default(),
            mark_seen: Default::default(),
            mark_gen: Cell::new(0),
            flushing: Cell::new(false),
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
        // Record the current scope as this scope's parent so `drop_scope` can
        // restore it (WS1.1) — the parent pointer *is* the scope stack.
        let parent = self.current_scope.get();
        let id = self.scopes.borrow_mut().insert(ScopeData::new(
            parent,
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
        let parent = self.current_scope.get();
        let id = self.scopes.borrow_mut().insert(ScopeData::new_deny_new(
            parent,
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
            debug: crate::storage::ValueDebugInfo {
                name: None,
                created_at: _caller,
                state: match initial_state {
                    ValueState::Clean => {
                        crate::storage::ValueDebugInfoState::Clean(None)
                    },
                    ValueState::Check => {
                        crate::storage::ValueDebugInfoState::CheckRequested(
                            _caller, None,
                        )
                    },
                    ValueState::Dirty => {
                        crate::storage::ValueDebugInfoState::Dirten(
                            _caller, None,
                        )
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
                entry.or_default().push(id);
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

    /// Upgrade an inert [`ValueKind::Stored`] value into a
    /// [`ValueKind::Signal`] in place, keeping the same [`ValueId`]. This
    /// is the reactive-on-write transition: because reactivity is keyed by
    /// `ValueId`, every existing handle to `id` becomes reactive at once.
    /// No-op if `id` is already reactive or absent.
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

    /// Create a fresh [`ValueKind::Probe`] node, born `Dirty` so its first
    /// poll runs. Identity *is* the returned id — no registry, no hashing
    /// (WS2). The core stays UI-vocabulary-free; the owner (e.g. `rsact-ui`'s
    /// `ElState`) stores the handle.
    pub(crate) fn create_probe(
        &self,
        caller: &'static Location<'static>,
    ) -> ValueId {
        self.add_value::<_, ()>((), ValueKind::Probe, ValueState::Dirty, caller)
    }

    /// The identity-free core of a polled reaction (formerly the body of
    /// [`use_observe`]). Runs `f` (tracked) iff a dependency changed since the
    /// last poll, or `force`. Step order is deliberate and unchanged from the
    /// keyed path:
    ///
    /// 1. `is_alive` — a disposed probe returns an honest `None` (no revive:
    ///    the handle *is* the identity).
    /// 2. `subscribe` — record the parent-observer edge so a child re-render
    ///    dirties its page (invariant: child dirty ⇒ page dirty).
    /// 3. `maybe_update` — freshen dependencies; `Dirty` ⇒ this probe changed.
    /// 4. if changed‖force: `with_observer(f)` then `mark_clean` — the clean is
    ///    placed *after* the closure runs so a parent's dependency-walk cannot
    ///    consume this probe's dirtiness first (the checkbox-redraw fix).
    pub(crate) fn run_probe<R>(
        &self,
        id: ValueId,
        force: bool,
        location: &'static Location<'static>,
        f: impl FnOnce() -> R,
    ) -> Option<R> {
        if !self.is_alive(id) {
            return None;
        }

        self.subscribe(id);
        let updated = self.maybe_update(id, Some(id), location);

        if updated || force {
            // Detach the previous poll's dependency edges *before* re-running,
            // so `f` re-tracks a fresh source set (a conditionally-dropped
            // dependency is unsubscribed). This is the edge-only half of the
            // old `cleanup` — it deliberately does NOT dispose owned children,
            // which is what nuked nested probes on a parent re-run (WS2). Memos
            // and effects already pay this per recompute; probes now do too.
            self.clear_sources(id);
            let result = self.with_observer(id, |_rt| f());
            self.mark_clean(id, Some(id), location);
            Some(result)
        } else {
            None
        }
    }

    pub unsafe fn dispose(&self, id: ValueId) {
        // Collect owned children first so the borrow on `owned` is fully
        // released before any recursive dispose() call re-borrows it.
        let owned_children: Vec<ValueId> = self
            .owned
            .borrow_mut()
            .remove(id)
            .into_iter()
            .flatten()
            .collect();

        // Remove id from the subscriber set of every source it tracked.
        // Without this, disposed effects leave ghost entries that cause
        // mark_dirty to try to mark a dead value, panicking in storage.mark.
        {
            let mut subs = self.subscribers.borrow_mut();
            let sources = self.sources.borrow();
            for source in sources.get(id).into_iter().flatten().copied() {
                if let Some(set) = subs.get_mut(source) {
                    id_vec_remove(set, id);
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
                    id_vec_remove(set, id);
                }
            }
        }

        self.sources.borrow_mut().remove(id);
        self.subscribers.borrow_mut().remove(id);
        // TODO: Is it okay to remove from pending_effects?
        self.pending_effects.borrow_mut().remove(id);
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
        // Release the borrow immediately so dispose() can run without
        // conflicts.
        let scope_data = self.scopes.borrow_mut().remove(scope_id).unwrap();

        // Restore `current_scope` to this scope's parent — but *only* if this
        // scope is still the current one. Page scopes are held across frames
        // and dropped non-lexically (out of LIFO order); in that case the
        // current scope belongs to unrelated live work and must be left
        // untouched (WS1.1).
        if self.current_scope.get() == Some(scope_id) {
            self.current_scope.set(scope_data.parent);
        }

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
        // Restore the previous observer on the way out — even if `f` panics
        // (e.g. a memo/effect closure panics and is caught upstream) — so the
        // observer cell can't be left pointing at a disposed node.
        struct ObserverGuard<'a> {
            cell: &'a Cell<Option<ValueId>>,
            prev: Option<ValueId>,
        }
        impl Drop for ObserverGuard<'_> {
            fn drop(&mut self) {
                self.cell.set(self.prev);
            }
        }

        let _guard =
            ObserverGuard { cell: &self.observer, prev: self.observer.get() };
        self.observer.set(Some(observer));

        f(self)
    }

    pub(crate) fn subscribe(&self, id: ValueId) {
        if let Some(observer) = self.observer.get() {
            if observer == id {
                panic!(
                    "Recursive subscription. Tried to subscribe observer to itself"
                );
            }

            // Fast path: re-reading the same source within one observer run is
            // common (e.g. a widget reading a signal several times per render).
            // If the edge already exists, skip both inserts *and* the height
            // recompute (which rescans all sources) — nothing changed.
            if self
                .sources
                .borrow()
                .get(observer)
                .is_some_and(|srcs| srcs.contains(&id))
            {
                return;
            }

            {
                let mut sources = self.sources.borrow_mut();
                if let Some(sources) = sources.entry(observer) {
                    sources.or_default().push(id);
                }
            }

            {
                let mut subs = self.subscribers.borrow_mut();
                if let Some(subs) = subs.entry(id) {
                    subs.or_default().push(observer);
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
            // Snapshot source ids into a stack-inline buffer instead of cloning
            // the whole `BTreeSet` — fan-in is almost always tiny, so this
            // avoids a heap allocation on every Check-state pull. A local
            // (not shared runtime scratch) so it stays recursion-safe.
            let sources: tinyvec::TinyVec<[ValueId; 8]> = {
                let subs = self.sources.borrow();
                subs.get(id)
                    .map(|s| s.iter().copied().collect())
                    .unwrap_or_default()
            };
            for source in sources {
                // TODO: Should all sources by updates or we stop at the first
                // change? If we stop at the first, why do even check if value
                // could already be dirty?
                self.maybe_update(source, Some(source), caller);
                if self.is(id, ValueState::Dirty) {
                    // TODO: Cache check and use after break
                    break;
                }
            }

            // The source walk completed without any source turning us Dirty:
            // every source was freshened (maybe_update'd) and none reported a
            // change, so this node is genuinely unchanged (a memo cut).
            // Downgrade Check -> Clean so the *next* read is an O(1) state check
            // instead of re-walking the whole source subtree every time
            // (WS1.5b — the "check residue"). Only ever Check -> Clean here,
            // never Dirty -> Clean: a changed source would have marked us Dirty
            // (breaking the loop above), so reaching here still Check means
            // clean. This is also correct for Observers — a real dependency
            // change leaves them Dirty (re-run in use_observe), and an unchanged
            // one would not re-render anyway.
            if self.is(id, ValueState::Check) {
                self.mark_clean(id, requester, caller);
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
        // Read the cheap `Copy` tag first; only the callback kinds below need
        // to clone the `Rc`-holding kind out of storage, and Signal/Stored/
        // Observer avoid touching the value cell entirely.
        let Some(tag) = self.storage.kind_of(id) else { return };

        if self.defer_effects.get() > 0 && tag == ValueKindTag::Effect {
            let height = self.storage.get_height(id);
            self.pending_effects.borrow_mut().push(id, height);
            return;
        }

        // A `Probe`'s work (its closure) runs in `run_probe`, not here. If we
        // marked it clean here (during a *parent* observer's `maybe_update`
        // dependency-walk), its dirtiness would be consumed before the nested
        // `poll` re-checks it, so the nested closure would be skipped. Probes
        // are marked clean in `run_probe` after their closure actually runs.
        let is_probe = tag == ValueKindTag::Probe;

        let changed = match tag {
            ValueKindTag::Stored => false,
            ValueKindTag::Signal | ValueKindTag::Probe => true,
            ValueKindTag::Memo
            | ValueKindTag::Computed
            | ValueKindTag::Effect => {
                // Clone only the callback `Rc` + the value cell. Borrow is
                // dropped before running the callback (which re-enters storage).
                let borrowed = {
                    let values = self.storage.values.borrow();
                    match values.get(id) {
                        Some(Value {
                            kind:
                                ValueKind::Memo { f }
                                | ValueKind::Computed { f }
                                | ValueKind::Effect { f },
                            value,
                            ..
                        }) => Some((f.clone(), value.clone())),
                        _ => None,
                    }
                };
                let Some((f, value)) = borrowed else { return };

                let ran = self.with_observer(id, move |rt| {
                    // Guard against re-entrant recompute: if this node's callback
                    // is already running higher on the stack, a dependency cycle
                    // has re-entered it. Skip (logged) instead of panicking on
                    // the double borrow_mut, so a cycle degrades to a no-op.
                    // `None` signals "skipped, never recomputed" so the state is
                    // left untouched below.
                    let Ok(mut callback) = f.try_borrow_mut() else {
                        log::error!(
                            "skipping re-entrant reactive update (dependency \
                             cycle) at {caller}"
                        );
                        return None;
                    };

                    rt.cleanup(id);
                    Some(callback.run(value))
                });

                match ran {
                    Some(changed) => changed,
                    // Re-entrant skip: the node was never recomputed, so its
                    // stale value must NOT be marked Clean (that would present
                    // it as fresh). Leave its state untouched — it stays Dirty
                    // and is retried on the next independent pull; a true cycle
                    // is bounded by run_effects' round cap (WS1.3b).
                    None => return,
                }
            },
        };

        if changed {
            if let Some(subs) = self.subscribers.borrow().get(id) {
                for sub in subs.iter() {
                    // The commit path uses a bare `storage.mark` (state byte
                    // only), NOT `mark_node`. `mark_node` would also enqueue
                    // effect subscribers into `pending_effects`, but the
                    // write-time push (`mark_dirty`) already queued every
                    // transitively-reachable effect — the push-queues-effects
                    // invariant pinned by the WS1.3a tests. Re-enqueuing here
                    // doubled effect-rerun allocations (measured 2/112 -> 4/224 B
                    // in benches/allocations.rs): a redundant BTreeSet insert
                    // into the just-drained queue plus an extra flush round, for
                    // zero benefit today. Making the pull self-sufficient is only
                    // needed once WS5 makes marking lazier, and must be done there
                    // WITHOUT this re-enqueue cost (WS1.3b).
                    self.storage.mark(
                        *sub,
                        ValueState::Dirty,
                        requester,
                        caller,
                    );
                }
            }
        }

        if !is_probe {
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

        // Walk the transitive subscriber closure iteratively (heap worklist,
        // not the call stack — so a deep chain can't overflow the stack, which
        // matters on embedded's tiny stacks) and mark each node `Check` exactly
        // once. Dedup uses a per-node generation stamp: bumping `mark_gen`
        // invalidates all prior stamps in O(1), so there is no per-write
        // allocation and no O(capacity) `clear`.
        let generation = self.mark_gen.get().wrapping_add(1);
        self.mark_gen.set(generation);

        // Reuse the per-runtime scratch buffers. `mark_dirty` is never entered
        // re-entrantly (its walk only calls `mark_node`, which never writes a
        // signal), but if that ever changes, fall back to fresh locals so the
        // walk stays correct instead of panicking on the scratch borrow.
        match (
            self.mark_stack.try_borrow_mut(),
            self.mark_seen.try_borrow_mut(),
        ) {
            (Ok(mut stack), Ok(mut seen)) => {
                stack.clear();
                self.mark_check_closure(
                    id, generation, requester, caller, &mut stack, &mut seen,
                );
            },
            _ => {
                log::warn!(
                    "mark_dirty re-entered; falling back to fresh stack/seen"
                );
                let mut stack = Vec::new();
                let mut seen = SecondaryMap::new();
                self.mark_check_closure(
                    id, generation, requester, caller, &mut stack, &mut seen,
                );
            },
        }
    }

    /// Iterative transitive-subscriber walk used by [`mark_dirty`]: marks every
    /// node reachable from `root`'s subscribers `Check`, each exactly once
    /// (deduped by `generation` stamp in `seen`). `stack` is the worklist.
    fn mark_check_closure(
        &self,
        root: ValueId,
        generation: u64,
        requester: Option<ValueId>,
        caller: &'static Location<'static>,
        stack: &mut Vec<ValueId>,
        seen: &mut SecondaryMap<ValueId, u64>,
    ) {
        // Hold the subscribers borrow for the whole walk — `mark_node` touches
        // storage and pending_effects, never the subscribers map.
        let subscribers = self.subscribers.borrow();

        if let Some(direct) = subscribers.get(root) {
            for &sub in direct.iter() {
                if seen.get(sub).copied() != Some(generation) {
                    seen.insert(sub, generation);
                    stack.push(sub);
                }
            }
        }

        while let Some(node) = stack.pop() {
            self.mark_node(node, ValueState::Check, requester, caller);
            if let Some(children) = subscribers.get(node) {
                for &child in children.iter() {
                    if seen.get(child).copied() != Some(generation) {
                        seen.insert(child, generation);
                        stack.push(child);
                    }
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

        if self.storage.kind_of(id) == Some(ValueKindTag::Effect)
            && self.observer.get() != Some(id)
        {
            let height = self.storage.get_height(id);
            RefCell::borrow_mut(&self.pending_effects).push(id, height);
        }
    }

    // TODO: Explicitly return Option<ValueState> for disposed values.
    pub(crate) fn state(&self, id: ValueId) -> ValueState {
        // Cheap Copy-field read; must not clone the whole `Value` (this is on
        // the hot path of every `is()`/`maybe_update` check).
        self.storage.state_of(id).unwrap_or(ValueState::Clean)
    }

    pub(crate) fn is(&self, id: ValueId, state: ValueState) -> bool {
        self.state(id) == state
    }

    #[cfg(feature = "debug-info")]
    pub fn debug_info(&self, id: ValueId) -> crate::storage::ValueDebugInfo {
        let debug_info = self.storage.debug_info(id).unwrap();

        // TODO: This is wrong, should not return current observer but
        // subscribers list if let Some(crate::storage::ValueDebugInfo {
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
    pub fn observer_debug_info(
        &self,
    ) -> Option<crate::storage::ValueDebugInfo> {
        self.observer
            .get()
            .and_then(|observer| self.storage.debug_info(observer))
    }

    /// Generate mermaid graph containing all values in runtime.
    /// Be careful, this might be very expensive, use it only for debug
    /// purposes.
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
                    ValueKind::Probe => ("{", "}", false),
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
            if let crate::storage::ValueDebugInfoState::CheckRequested(
                _,
                Some((requester_id, _)),
            )
            | crate::storage::ValueDebugInfoState::Dirten(
                _,
                Some((requester_id, _)),
            )
            | crate::storage::ValueDebugInfoState::Clean(Some((
                requester_id,
                _,
            ))) = debug_info.state
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

    /// Detach `id` from its current dependency edges: remove `id` from the
    /// subscriber set of every source it tracked, then clear its own source
    /// list (so heights are recomputed on the next re-subscription).
    ///
    /// This is the *first* of [`cleanup`](Self::cleanup)'s two jobs, split out
    /// (WS2) so the per-execution path can re-track dependencies **without**
    /// the second job (owned-value disposal). Memos/effects already pay this
    /// cost on every recompute; probes ([`run_probe`](Self::run_probe)) call it
    /// per executed poll so a conditionally-dropped dependency is unsubscribed
    /// — but must NOT dispose owned children, which is what nuked nested render
    /// probes when the full `cleanup` ran on a parent re-run.
    pub(crate) fn clear_sources(&self, id: ValueId) {
        {
            let sources = self.sources.borrow();
            if let Some(srcs) = sources.get(id) {
                // Remove `id` from the subscriber set of every source it
                // previously tracked.
                let mut subs = self.subscribers.borrow_mut();
                for source in srcs.iter() {
                    if let Some(set) = subs.get_mut(*source) {
                        id_vec_remove(set, id);
                    }
                }
            }
        }

        // Clear the source list so heights are recomputed on next
        // re-subscription.
        if let Some(srcs) = self.sources.borrow_mut().get_mut(id) {
            srcs.clear();
        }
    }

    fn cleanup(&self, id: ValueId) {
        // Job 1: detach dependency edges.
        self.clear_sources(id);

        // Job 2: dispose owned children.
        // FIXME: I am deleting the values created in this observer, but they
        // could be leaked outside. Collect and clear owned list before
        // calling dispose to avoid a double borrow of `owned`
        // (dispose() also calls owned.borrow_mut()).
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

        // Non-re-entrant: a signal written by an effect during this flush
        // re-enters here; that nested call returns and the outermost loop below
        // drains the newly-queued effects. Keeps effect execution iterative and
        // makes the round cap below an effective cycle guard.
        if self.flushing.get() {
            return;
        }
        self.flushing.set(true);
        struct FlushGuard<'a>(&'a Cell<bool>);
        impl Drop for FlushGuard<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let _flush_guard = FlushGuard(&self.flushing);

        // Loop until stable: running effects may write signals that queue more
        // effects. A legitimate cascade settles in at most (graph height) rounds
        // — realistically a handful. If it does not settle after this many
        // rounds it is almost certainly a dependency cycle (e.g. two effects
        // mutually writing each other's sources); log and break rather than
        // hang or overflow the stack (`CLAUDE.md`: log errors, do not panic).
        const MAX_FLUSH_ROUNDS: u32 = 10_000;
        let mut rounds = 0u32;
        loop {
            if self.pending_effects.borrow().is_empty() {
                break;
            }

            rounds += 1;
            if rounds > MAX_FLUSH_ROUNDS {
                let mut queue = self.pending_effects.borrow_mut();
                log::error!(
                    "reactive effect flush did not settle after {MAX_FLUSH_ROUNDS} rounds \
                     (likely a dependency cycle); aborting flush at {caller} with {} effect(s) \
                     still pending",
                    queue.len()
                );
                // Discard the stuck queue so the cycle can't re-trigger every
                // flush (WS9a.1 decision: clear-and-log over leave-wedged).
                queue.clear();
                break;
            }

            // Snapshot this round's effects into `draining`; effects queued
            // while they run accumulate in `pending` for the next round (the
            // allocation-free equivalent of the old `.take()`).
            self.pending_effects.borrow_mut().start_round();

            // Drain lowest topological height first — buckets are already in
            // height order, so no sort. Draining source-adjacent effects first
            // keeps observers from seeing a stale intermediate value; deeper
            // glitch-freedom comes from `maybe_update` pulling each effect's
            // sources on demand. The borrow is released around every
            // `maybe_update` (which re-enters the queue via `push`, and may
            // `remove` on dispose), so re-borrowing per pop keeps that safe.
            let mut cursor = 0usize;
            loop {
                let next =
                    self.pending_effects.borrow_mut().pop_next(&mut cursor);
                match next {
                    Some(effect) => {
                        self.maybe_update(effect, requester, caller);
                    },
                    None => break,
                }
            }
        }
    }

    /// Recompute the topological height of `id` from its current sources and
    /// update storage. Height = max(height of sources) + 1.  Signals start
    /// at 0 (no sources). Called after every new subscription so pending
    /// effects are always sorted correctly.
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
}

pub fn current_runtime_profile() -> Profile {
    with_current_runtime(|rt| {
        let (stored, signals, effects, memos, computed, observers) =
            rt.storage.values.borrow().values().fold(
                (0, 0, 0, 0, 0, 0),
                |(
                    mut stored,
                    mut signals,
                    mut effects,
                    mut memos,
                    mut computed,
                    mut observers,
                ),
                 value| {
                    match &value.kind {
                        ValueKind::Stored => stored += 1,
                        ValueKind::Signal => signals += 1,
                        ValueKind::Effect { .. } => effects += 1,
                        ValueKind::Memo { .. } => memos += 1,
                        ValueKind::Computed { .. } => computed += 1,
                        // Metrics field is still named `observers` for
                        // snapshot-schema stability; it now counts `Probe`
                        // nodes (WS2 rename). TODO: rename field to `probes`
                        // in a dedicated metrics pass.
                        ValueKind::Probe => observers += 1,
                    }

                    (stored, signals, effects, memos, computed, observers)
                },
            );

        let subscribers_bindings = rt
            .subscribers
            .borrow()
            .values()
            .map(|subs| subs.len())
            .sum();
        let sources_bindings = rt
            .sources
            .borrow()
            .values()
            .map(|sources| sources.len())
            .sum();

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
            observers,
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

/// A snapshot of the reactive runtime's node population and edge counts.
///
/// Fields are public so external tooling (the `metrics-probe` snapshot tool)
/// can serialize them; the values are a read-only sample and hold no runtime
/// borrow. `observers` counts polled [`crate::probe::Probe`] nodes
/// (`ValueKind::Probe`), which the pre-metrics profile ignored.
#[derive(Clone, Copy)]
pub struct Profile {
    pub stored: usize,
    pub signals: usize,
    pub effects: usize,
    pub memos: usize,
    pub computed: usize,
    pub observers: usize,
    pub subscribers: usize,
    pub subscribers_bindings: usize,
    pub sources: usize,
    pub sources_bindings: usize,
    pub pending_effects: usize,
    #[cfg(feature = "debug-info")]
    top_by_subs: Option<(&'static Location<'static>, usize)>,
    #[cfg(feature = "debug-info")]
    top_by_sources: Option<(&'static Location<'static>, usize)>,
}

impl Profile {
    /// Total live node count, summed across every [`crate::storage::ValueKind`]
    /// (the single source of the node-sum formula — used by `Display` here and
    /// by external tooling like metrics-probe).
    pub fn total(&self) -> usize {
        self.stored
            + self.signals
            + self.effects
            + self.memos
            + self.computed
            + self.observers
    }
}

impl Display for Profile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "{} values:", self.total())?;
        writeln!(f, "  {} stored", self.stored)?;
        writeln!(f, "  {} signals", self.signals)?;
        writeln!(f, "  {} effects", self.effects)?;
        writeln!(f, "  {} memos", self.memos)?;
        writeln!(f, "  {} computed", self.computed)?;
        writeln!(f, "  {} observers", self.observers)?;
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
        memo::{Memo, create_memo},
        read::ReadSignal,
        runtime::{
            RUNTIMES, batch, untrack, with_current_runtime, with_new_runtime,
        },
        scope::new_scope,
        signal::create_signal,
        storage::ValueState,
        write::WriteSignal,
    };
    use alloc::rc::Rc;
    use alloc::vec::Vec;
    use core::cell::Cell;

    /// Test helper: the current runtime's [`ValueState`] for a handle.
    fn state_of(id: crate::storage::ValueId) -> ValueState {
        with_current_runtime(|rt| rt.state(id))
    }

    #[test]
    fn primary_runtime() {
        assert!(
            RUNTIMES.with(|rts| rts.borrow().contains_key(
                CURRENT_RUNTIME.with(|current| current.get().unwrap())
            )),
            "First insertion into RUNTIMES does not have key of RuntimeId::default()"
        );
    }

    /// `with_new_runtime` must restore the previously-current runtime when the
    /// temporary one leaves. The buggy version discarded `prev` and `leave()`
    /// merely cleared the current cell, so any reactive access after the call
    /// panicked on a `None` current runtime. Regression test for WS1.2.
    #[test]
    fn with_new_runtime_restores_previous() {
        // `outer` lives in the previously-current (default) runtime.
        let mut outer = create_signal(1i32);

        with_new_runtime(|_| {
            let inner = create_signal(2i32);
            assert_eq!(inner.get(), 2);
        });

        // The previous runtime must be current again — `outer` still works.
        assert_eq!(outer.get(), 1);
        outer.set(5);
        assert_eq!(outer.get(), 5);
    }

    // --- WS1.8: try_* APIs + contextful panics ------------------------------

    /// The `try_*` read/write APIs return `Some` for a live handle and `None`
    /// (logged, no panic) for a disposed one.
    #[test]
    fn try_apis_return_none_for_disposed_handle() {
        with_new_runtime(|_| {
            let mut sig = create_signal(5i32);

            // Live handle: try_* behave like their panicking siblings.
            assert_eq!(sig.try_get(), Some(5));
            assert_eq!(sig.try_with(|v| *v * 2), Some(10));
            assert_eq!(sig.try_get_cloned(), Some(5));
            assert_eq!(sig.try_update(|v| *v += 1), Some(()));
            assert_eq!(sig.get(), 6);
            assert_eq!(sig.try_set(7), Some(()));
            assert_eq!(sig.get(), 7);

            // Disposing a Copy handle kills the shared node.
            unsafe { sig.dispose() };
            assert!(!sig.is_alive());

            // Every try_* now yields None instead of panicking.
            assert_eq!(sig.try_get(), None);
            assert_eq!(sig.try_with(|v| *v), None);
            assert_eq!(sig.try_get_cloned(), None);
            assert_eq!(sig.try_update(|v| *v += 1), None);
            assert_eq!(sig.try_set(9), None);
        });
    }

    /// The panicking APIs still panic on a disposed handle, but with a
    /// contextful message (not a bare unwrap).
    #[test]
    #[should_panic(expected = "reactive value")]
    fn disposed_handle_get_panics_with_context() {
        with_new_runtime(|_| {
            let sig = create_signal(5i32);
            unsafe { sig.dispose() };
            let _ = sig.get();
        });
    }

    // --- WS1.3a: push-queues-effects invariant ------------------------------
    //
    // update()'s commit path marks a recomputed node's subscribers Dirty with a
    // bare storage.mark, which flips the state byte but does NOT enqueue an
    // effect-subscriber into pending_effects (only mark_node does). An effect
    // downstream of a memo therefore fires only because the write-time
    // mark_dirty push already queued every transitively-reachable effect. These
    // tests pin that behaviour so WS1.3b can make the pull self-sufficient
    // (commit path -> mark_node) without changing observable results.

    /// signal -> memo -> effect: the memo recomputes lazily *while the effect is
    /// pulling it* inside run_effects; the effect must still re-run with the new
    /// memo value.
    #[test]
    fn effect_downstream_of_memo_fires_on_source_change() {
        let mut src = create_signal(1i32);
        let doubled = create_memo(move || src.get() * 2);

        let seen = Rc::new(Cell::new(0i32));
        let runs = Rc::new(Cell::new(0u32));
        let seen_c = seen.clone();
        let runs_c = runs.clone();
        create_effect(move |_: Option<()>| {
            seen_c.set(doubled.get());
            runs_c.set(runs_c.get() + 1);
        });

        assert_eq!(runs.get(), 1, "effect should run once on creation");
        assert_eq!(seen.get(), 2);

        src.set(5);
        assert_eq!(
            runs.get(),
            2,
            "effect downstream of a memo did not re-run on source change"
        );
        assert_eq!(seen.get(), 10);
    }

    /// signal -> memo -> effect where a write leaves the memo *value* unchanged
    /// (memo cut): even though the push re-queues the effect, run_effects'
    /// maybe_update re-checks and finds it not actually Dirty, so the body must
    /// NOT run again.
    #[test]
    fn effect_downstream_of_memo_not_rerun_on_memo_cut() {
        let mut src = create_signal(4i32);
        // Memo output depends only on the sign, so many src values map to 1.
        let is_positive = create_memo(move || src.get() > 0);

        let runs = Rc::new(Cell::new(0u32));
        let runs_c = runs.clone();
        create_effect(move |_: Option<()>| {
            is_positive.get();
            runs_c.set(runs_c.get() + 1);
        });

        assert_eq!(runs.get(), 1);

        // Different source value, identical memo output -> effect must not fire.
        src.set(9);
        assert_eq!(
            runs.get(),
            1,
            "effect re-ran despite the memo value being unchanged (memo cut broken)"
        );

        // A genuine memo change still propagates.
        src.set(-3);
        assert_eq!(
            runs.get(),
            2,
            "effect did not re-run when the memo value actually changed"
        );
    }

    /// signal -> memo -> memo -> effect: two lazy layers recompute during the
    /// effect's pull. The effect must fire exactly once per real change,
    /// proving the push queued it across both memo layers.
    #[test]
    fn effect_fires_through_two_memo_layers() {
        let mut src = create_signal(1i32);
        let plus_one = create_memo(move || src.get() + 1);
        let times_ten = create_memo(move || plus_one.get() * 10);

        let seen = Rc::new(Cell::new(0i32));
        let runs = Rc::new(Cell::new(0u32));
        let seen_c = seen.clone();
        let runs_c = runs.clone();
        create_effect(move |_: Option<()>| {
            seen_c.set(times_ten.get());
            runs_c.set(runs_c.get() + 1);
        });

        assert_eq!(runs.get(), 1);
        assert_eq!(seen.get(), 20); // (1+1)*10

        src.set(3);
        assert_eq!(runs.get(), 2);
        assert_eq!(seen.get(), 40); // (3+1)*10
    }

    // --- WS9a.1: effect-queue (height-bucketed) semantics -------------------
    //
    // The `pending_effects` structure must, regardless of its internals:
    //   (a) dedup — an effect marked N times before a flush runs once;
    //   (b) fan out — every effect subscribed to a written signal fires;
    //   (c) cascade — an effect that writes a signal re-runs its dependents,
    //       settling in bounded rounds;
    //   (d) keep glitch-free ordering across memo layers of differing height.
    // These lock behavior across the BTreeSet -> height-bucketed-vec swap.

    /// (a) One signal, one effect, 100 batched writes: the effect is marked on
    /// every write but the queue must dedup so it runs exactly once at flush.
    #[test]
    fn effect_queue_dedups_batched_writes() {
        with_new_runtime(|_| {
            let mut src = create_signal(0i32);
            let runs = Rc::new(Cell::new(0u32));
            let seen = Rc::new(Cell::new(0i32));
            let runs_c = runs.clone();
            let seen_c = seen.clone();
            create_effect(move |_: Option<()>| {
                seen_c.set(src.get());
                runs_c.set(runs_c.get() + 1);
            });
            assert_eq!(runs.get(), 1, "effect runs once on creation");

            batch(|| {
                for k in 1..=100i32 {
                    src.set(k);
                }
            });

            assert_eq!(
                runs.get(),
                2,
                "batched writes must coalesce to a single effect run"
            );
            assert_eq!(seen.get(), 100, "effect must see the last batched value");
        });
    }

    /// (b) Many effects on one signal all fire exactly once per write.
    #[test]
    fn effect_queue_fans_out_to_all_subscribers() {
        with_new_runtime(|_| {
            let mut src = create_signal(0i32);
            let runs = Rc::new(Cell::new(0u32));
            for _ in 0..16 {
                let runs_c = runs.clone();
                create_effect(move |_: Option<()>| {
                    src.get();
                    runs_c.set(runs_c.get() + 1);
                });
            }
            assert_eq!(runs.get(), 16, "each effect runs once on creation");

            src.set(1);
            assert_eq!(
                runs.get(),
                32,
                "every subscriber effect must fire once on the write"
            );
        });
    }

    /// (c) Cascade: an effect writes a downstream signal, re-running a second
    /// effect. Must settle (bounded rounds), and the observer effect must end on
    /// the final value.
    #[test]
    fn effect_queue_cascade_settles() {
        with_new_runtime(|_| {
            let mut a = create_signal(0i32);
            let b = create_signal(0i32);

            // Effect 1: mirror a -> b (writes a signal from inside an effect).
            let mut b_writer = b;
            create_effect(move |_: Option<()>| {
                let v = a.get();
                b_writer.set(v * 2);
            });

            // Effect 2: observe b.
            let seen = Rc::new(Cell::new(-1i32));
            let runs = Rc::new(Cell::new(0u32));
            let seen_c = seen.clone();
            let runs_c = runs.clone();
            create_effect(move |_: Option<()>| {
                seen_c.set(b.get());
                runs_c.set(runs_c.get() + 1);
            });

            assert_eq!(seen.get(), 0, "observer sees initial cascaded b");

            a.set(21);
            assert_eq!(
                seen.get(),
                42,
                "cascade a->b->observer must settle on the final value"
            );
            assert!(
                runs.get() >= 2,
                "observer must re-run after the cascade updated b"
            );
        });
    }

    /// (d) signal -> two memos of different height -> one effect reading both:
    /// the effect must fire once with a consistent (non-glitched) view even
    /// though its sources sit at different topological heights (bucket order).
    #[test]
    fn effect_queue_no_glitch_across_heights() {
        with_new_runtime(|_| {
            let mut src = create_signal(1i32);
            let shallow = create_memo(move || src.get() + 1); // height 2
            let deep = create_memo(move || shallow.get() * 10); // height 3

            let seen = Rc::new(Cell::new((0i32, 0i32)));
            let runs = Rc::new(Cell::new(0u32));
            let seen_c = seen.clone();
            let runs_c = runs.clone();
            create_effect(move |_: Option<()>| {
                // Reads both a shallow and a deep source; a glitch would show
                // `deep` computed from a stale `shallow`.
                seen_c.set((src.get(), deep.get()));
                runs_c.set(runs_c.get() + 1);
            });
            assert_eq!(seen.get(), (1, 20));
            assert_eq!(runs.get(), 1);

            src.set(5);
            assert_eq!(
                seen.get(),
                (5, 60),
                "effect saw a glitched (stale-shallow) view of the deep memo"
            );
            assert_eq!(runs.get(), 2, "effect must fire exactly once per change");
        });
    }

    // --- WS1.5a: check-residue correctness suite ----------------------------
    //
    // When a Check node's *completed* source walk finds nothing Dirty (a memo
    // cut — a source recomputed to an equal value), the node is genuinely
    // unchanged and should be downgraded Check -> Clean so the next read is an
    // O(1) state check rather than re-walking every source. Pre-1.5b it stayed
    // Check forever (residue). These tests prove the state contract (they RED
    // before 1.5b) and that correctness is preserved.

    /// After a memo cut, the downstream node is Clean (not Check residue).
    #[test]
    fn check_residue_downgraded_to_clean_after_memo_cut() {
        with_new_runtime(|_| {
            let mut src = create_signal(4i32);
            let is_pos = create_memo(move || src.get() > 0);
            let downstream = create_memo(move || is_pos.get() as i32);

            // Prime: everything computes Clean.
            assert_eq!(downstream.get(), 1);

            // Different source value, identical memo output -> cut.
            src.set(9);
            assert_eq!(downstream.get(), 1);

            assert_eq!(
                state_of(downstream.id().unwrap()),
                ValueState::Clean,
                "downstream memo left in Check after a memo cut (residue)"
            );
            assert_eq!(
                state_of(is_pos.id().unwrap()),
                ValueState::Clean,
                "cut memo itself left non-Clean"
            );
        });
    }

    /// Diamond: src feeds two memos that both feed a sink. A cut at src leaves
    /// the whole diamond Clean.
    #[test]
    fn diamond_no_check_residue_after_cut() {
        with_new_runtime(|_| {
            let mut src = create_signal(2i32);
            let left = create_memo(move || src.get() > 0);
            let right = create_memo(move || src.get() >= 0);
            let sink = create_memo(move || (left.get(), right.get()));

            assert_eq!(sink.get(), (true, true));

            // src 2 -> 5: both branches unchanged -> sink cut.
            src.set(5);
            assert_eq!(sink.get(), (true, true));

            assert_eq!(
                state_of(sink.id().unwrap()),
                ValueState::Clean,
                "diamond sink left in Check after a cut"
            );
        });
    }

    /// A genuine change after a cut still re-dirties and recomputes downstream.
    #[test]
    fn real_change_after_cut_still_recomputes() {
        with_new_runtime(|_| {
            let mut src = create_signal(4i32);
            let is_pos = create_memo(move || src.get() > 0);
            let downstream = create_memo(move || is_pos.get() as i32);

            assert_eq!(downstream.get(), 1);
            src.set(9); // cut
            assert_eq!(downstream.get(), 1);
            assert_eq!(state_of(downstream.id().unwrap()), ValueState::Clean);

            // Now a real change flips the memo.
            src.set(-2);
            assert_eq!(
                downstream.get(),
                0,
                "genuine change after a cut did not propagate"
            );
        });
    }

    /// Dynamic dependencies: the memo's source set changes across runs. After a
    /// cut through an old (now-untracked) source, a change through the *new*
    /// source still propagates.
    #[test]
    fn dynamic_deps_cut_then_change_through_new_source() {
        with_new_runtime(|_| {
            let mut cond = create_signal(true);
            let mut a = create_signal(1i32);
            let mut b = create_signal(100i32);
            let m =
                create_memo(move || if cond.get() { a.get() } else { b.get() });

            assert_eq!(m.get(), 1); // tracks cond + a

            // b is not a dependency now -> cut, m stays Clean.
            b.set(200);
            assert_eq!(m.get(), 1);
            assert_eq!(state_of(m.id().unwrap()), ValueState::Clean);

            // Switch branch: m re-tracks cond + b.
            cond.set(false);
            assert_eq!(m.get(), 200);

            // a is no longer a dependency -> cut.
            a.set(5);
            assert_eq!(m.get(), 200);
            assert_eq!(state_of(m.id().unwrap()), ValueState::Clean);

            // Change through the new source propagates.
            b.set(300);
            assert_eq!(
                m.get(),
                300,
                "change through the new source was dropped"
            );
        });
    }

    /// Property/fuzz: a fixed-shape memo DAG over signals must always match a
    /// recompute-everything oracle under a stream of pseudo-random writes
    /// (correctness is preserved regardless of Check/Clean bookkeeping).
    #[test]
    fn random_graph_matches_recompute_oracle() {
        with_new_runtime(|_| {
            let mut s = [
                create_signal(0i64),
                create_signal(0i64),
                create_signal(0i64),
                create_signal(0i64),
            ];
            let (s0, s1, s2, s3) = (s[0], s[1], s[2], s[3]);

            // DAG: a diamond over s1 plus a tail on s3.
            let m_a = create_memo(move || s0.get() + s1.get());
            let m_b = create_memo(move || s1.get() * 2 - s2.get());
            let m_c = create_memo(move || m_a.get() + m_b.get());
            let m_d = create_memo(move || m_c.get() + s3.get());

            let oracle = |v: &[i64; 4]| {
                let a = v[0] + v[1];
                let b = v[1] * 2 - v[2];
                let c = a + b;
                c + v[3]
            };

            // Deterministic LCG (Date/rand are unavailable / not a dep).
            let mut rng: u64 = 0x9E3779B97F4A7C15;
            let mut next = || {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                rng
            };

            let mut vals = [0i64; 4];
            for _ in 0..200 {
                let idx = (next() >> 33) as usize % 4;
                let val = ((next() >> 24) % 41) as i64 - 20; // -20..=20
                vals[idx] = val;
                s[idx].set(val);

                assert_eq!(
                    m_d.get(),
                    oracle(&vals),
                    "memo DAG diverged from the recompute oracle at vals={vals:?}"
                );
            }
        });
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
            for id in pending.queued_ids() {
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

            // `a` should have no subscribers now (cleanup removed the stale
            // sub).
            let a_id = a.id().unwrap();
            let a_subs = rt
                .subscribers
                .borrow()
                .get(a_id)
                .map(|s| s.len())
                .unwrap_or(0);
            assert_eq!(
                a_subs, 0,
                "stale subscription to `a` not removed after cleanup"
            );

            // Writing `a` must not trigger the effect (it no longer depends on
            // it).
            let before = reads.get();
            a.set(99);
            assert_eq!(
                reads.get(),
                before,
                "effect re-ran on stale dependency `a`"
            );
        });
    }

    // ---- Propagation-core corner cases (guard the mark_dirty rewrite) ----

    /// A diamond (s -> a, s -> b, {a,b} -> d) must recompute the apex `d`
    /// exactly once per source change. This guards against the mark_dirty
    /// dedup dropping or double-visiting a shared descendant.
    #[test]
    fn diamond_marks_each_node_once() {
        with_new_runtime(|_| {
            let mut s = create_signal(0i32);
            let a = create_memo(move || s.get() + 1);
            let b = create_memo(move || s.get() + 2);
            let d_runs = Rc::new(Cell::new(0u32));
            let dr = d_runs.clone();
            let d = create_memo(move || {
                dr.set(dr.get() + 1);
                a.get() + b.get()
            });

            assert_eq!(d.get(), 3);
            assert_eq!(d_runs.get(), 1);

            s.set(10);
            assert_eq!(d.get(), 23);
            assert_eq!(
                d_runs.get(),
                2,
                "diamond apex must recompute exactly once per source change"
            );
        });
    }

    /// One signal fanning out to N memos: each recomputes exactly once after a
    /// single source change (no missed and no duplicate marking).
    #[test]
    fn wide_fanout_recomputes_each_once() {
        with_new_runtime(|_| {
            let mut s = create_signal(0i32);
            let runs = Rc::new(Cell::new(0u32));
            let memos: Vec<_> = (0..64)
                .map(|k| {
                    let r = runs.clone();
                    create_memo(move || {
                        r.set(r.get() + 1);
                        s.get() + k
                    })
                })
                .collect();

            for m in &memos {
                m.get();
            }
            assert_eq!(runs.get(), 64);

            s.set(1);
            for m in &memos {
                m.get();
            }
            assert_eq!(
                runs.get(),
                128,
                "each memo must recompute exactly once after one source change"
            );
        });
    }

    /// An effect that writes another signal triggers a nested notify ->
    /// mark_dirty *during* the outer effect flush. The downstream effect must
    /// still see the new value (exercises re-entrant mark_dirty; the reused
    /// scratch buffers must not be corrupted by nesting).
    #[test]
    fn reentrant_write_during_effect_propagates() {
        with_new_runtime(|_| {
            let mut a = create_signal(0i32);
            let b = create_signal(0i32);

            // Effect reading `a` writes `b = a * 2`.
            let mut eb = b;
            create_effect(move |_: Option<()>| {
                let v = a.get();
                eb.set(v * 2);
            });

            // Effect reading `b` records the last value seen.
            let seen = Rc::new(Cell::new(-1i32));
            let sc = seen.clone();
            create_effect(move |_: Option<()>| {
                sc.set(b.get());
            });

            assert_eq!(seen.get(), 0);

            a.set(5);
            assert_eq!(
                seen.get(),
                10,
                "nested write during an effect flush must propagate downstream"
            );
        });
    }

    /// A deep linear dependency chain propagates correctly. The `mark_dirty`
    /// *push* walk is now iterative (heap worklist), so marking is O(1)-stack at
    /// any depth. NOTE: the *pull* phase (`maybe_update` → `update` → callback →
    /// `.get()` → `maybe_update`) is still recursive and stack-heavy (~2KB per
    /// chain level), so a deep chain still overflows on `.get()` — iterativizing
    /// the pull phase is a tracked follow-up. Depth here is kept small so the
    /// recursive pull stays within the (2MB) test-thread stack while still
    /// exercising deep marking + multi-level settling.
    #[test]
    fn deep_chain_propagates() {
        with_new_runtime(|_| {
            const DEPTH: usize = 100;
            let mut s = create_signal(0i32);
            let mut prev: Memo<i32> = create_memo(move || s.get());
            for _ in 0..DEPTH {
                let p = prev;
                prev = create_memo(move || p.get() + 1);
            }
            let leaf = prev;

            assert_eq!(leaf.get(), DEPTH as i32);
            // This write drives the iterative mark_dirty walk across the chain.
            s.set(1000);
            assert_eq!(leaf.get(), 1000 + DEPTH as i32);
        });
    }

    // ---- Phase 3A: robustness ----

    /// Two effects that mutually write each other's source form a dependency
    /// cycle. Before the flush guard this recursed / looped forever; now
    /// `run_effects` is non-re-entrant and the round cap breaks the cycle, so
    /// the write must simply return instead of hanging or overflowing.
    #[test]
    fn effect_cycle_degrades_without_hanging() {
        with_new_runtime(|_| {
            let mut a = create_signal(0i32);
            let b = create_signal(0i32);

            // A: reads a, writes b := a + 1 (always changes b).
            {
                let ra = a;
                let mut wb = b;
                create_effect(move |_: Option<()>| {
                    let v = ra.get();
                    wb.set(v + 1);
                });
            }
            // B: reads b, writes a := b + 1 (always changes a) -> mutual cycle.
            {
                let rb = b;
                let mut wa = a;
                create_effect(move |_: Option<()>| {
                    let v = rb.get();
                    wa.set(v + 1);
                });
            }

            // Kick the cycle. Reaching the next line at all is the assertion:
            // the flush terminated (cap hit + logged) rather than hanging.
            a.set(1);
            assert!(a.get() > 0);
        });
    }

    /// A panic inside `untrack`'s closure must still restore the previous
    /// observer (RAII guard), so an effect that catches such a panic keeps
    /// tracking its dependencies afterwards.
    #[cfg(feature = "std")]
    #[test]
    fn untrack_panic_does_not_corrupt_observer() {
        with_new_runtime(|_| {
            let mut trigger = create_signal(0i32);
            let runs = Rc::new(Cell::new(0u32));
            let r = runs.clone();

            create_effect(move |_: Option<()>| {
                r.set(r.get() + 1);
                // On the first run, panic inside untrack (caught). The observer
                // must be restored to THIS effect so the following `get()` still
                // subscribes it.
                if r.get() == 1 {
                    let _ = std::panic::catch_unwind(
                        std::panic::AssertUnwindSafe(|| {
                            untrack(|| panic!("boom"));
                        }),
                    );
                }
                trigger.get();
            });

            assert_eq!(runs.get(), 1);
            trigger.set(1);
            assert_eq!(
                runs.get(),
                2,
                "effect stopped tracking after a panic inside untrack \
                 (observer not restored)"
            );
        });
    }
}
