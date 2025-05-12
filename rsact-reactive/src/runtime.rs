use crate::{
    callback::CallbackFn,
    computed::ComputedCallback,
    effect::EffectCallback,
    memo::MemoCallback,
    memo_chain::{MemoChainCallback, MemoChainErr},
    scope::{ScopeData, ScopeHandle, ScopeId},
    storage::{
        Storage, StoredValue, ValueDebugInfo, ValueDebugInfoState, ValueId,
        ValueKind, ValueState,
    },
};
use alloc::{
    boxed::Box,
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    rc::Rc,
    vec::Vec,
};
use core::{
    cell::{Cell, RefCell},
    fmt::Display,
    marker::PhantomData,
    panic::Location,
};
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

#[non_exhaustive]
pub struct DeferEffectsGuard;

impl DeferEffectsGuard {
    pub fn run(self) {
        core::mem::drop(self);
    }
}

pub fn defer_effects() -> DeferEffectsGuard {
    with_current_runtime(|rt| rt.defer_effects())
}

impl Drop for DeferEffectsGuard {
    #[track_caller]
    fn drop(&mut self) {
        let caller = Location::caller();

        with_current_runtime(|rt| {
            rt.defer_effects.set(false);
            // TODO: Not an Option but Requester enum with DeferEffectsGuard?
            rt.run_effects(None, caller);
        })
    }
}

// TODO: ObserverGuard to flatten callbacks into `start_observe` and `end_observe` (auto on drop)

/// This call is identified by location in code and reruns only if reactive values from previous call are changed.
#[track_caller]
pub fn observe(f: impl FnOnce()) -> bool {
    let location = Location::caller();
    with_current_runtime(|rt| rt.use_observer(location, f, ()).0)
}

/// This call is identified by location in code and reruns only if reactive values from previous call are changed.
#[track_caller]
pub fn observe_or_default<R>(default: R, f: impl FnOnce() -> R) -> R {
    let location = Location::caller();
    with_current_runtime(|rt| rt.use_observer(location, f, default).1)
}

// TODO: Debug call-stack. Value get -> value get -> ... -> value get
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
    /// Sources of signal changes. Signals that change this signal.
    pub(crate) sources: RefCell<SecondaryMap<ValueId, BTreeSet<ValueId>>>,
    /// Effects to run after value changed or after [`DeferEffectsGuard`] runs/drops if defer_effects is enabled.
    pub(crate) pending_effects: RefCell<BTreeSet<ValueId>>,
    pub(crate) defer_effects: Cell<bool>,
    /// Mapping from [`observe`] call location to its value id. Calling the same [`observe`] twice gives the same [`ValueId`]
    static_observers: RefCell<BTreeMap<&'static Location<'static>, ValueId>>,
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
            defer_effects: Cell::new(false),
            static_observers: Default::default(),
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

        let id = self.storage.add_value(StoredValue {
            value: Rc::new(RefCell::new(value)),
            kind,
            state: initial_state,
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
            let mut owned = self.owned.borrow_mut();
            if let Some(owned) = owned.get_mut(observer) {
                owned.insert(id);
            }
        }

        id
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

    fn use_observer<R>(
        &self,
        location: &'static Location<'static>,
        f: impl FnOnce() -> R,
        default: R,
    ) -> (bool, R) {
        let id =
            *self.static_observers.borrow_mut().entry(location).or_insert_with(
                || {
                    self.add_value::<_, bool>(
                        true,
                        ValueKind::Observer,
                        ValueState::Dirty,
                        location,
                    )
                },
            );

        self.subscribe(id);
        self.maybe_update(id, Some(id), location);

        let dirty = {
            let dirty = self.storage.get(id).unwrap();
            let dirty = dirty.value.borrow_mut();
            *dirty.downcast_ref::<bool>().unwrap()
        };

        if dirty {
            let result = self.with_observer(id, |rt| {
                rt.cleanup(id);
                f()
            });
            self.mark_clean(id, Some(id), location);

            {
                let dirty = self.storage.get(id).unwrap();
                let mut dirty = dirty.value.borrow_mut();
                let dirty = dirty.downcast_mut::<bool>().unwrap();
                *dirty = false;
            }

            (true, result)
        } else {
            (false, default)
        }
    }

    pub fn dispose(&self, id: ValueId) {
        let mut values = self.storage.values.borrow_mut();
        let mut sources = self.sources.borrow_mut();
        let mut subscribers = self.subscribers.borrow_mut();
        let mut pending_effects = self.pending_effects.borrow_mut();

        sources.remove(id);
        subscribers.remove(id);
        // TODO: Is it okay to remove from pending_effects?
        pending_effects.remove(&id);
        values.remove(id).expect("Removing non-existent scope value");

        self.owned.borrow_mut().get(id).map(|owned| {
            owned.iter().copied().for_each(|owned| self.dispose(owned));
        });
    }

    pub(crate) fn drop_scope(&self, scope_id: ScopeId) {
        let mut scopes = self.scopes.borrow_mut();
        let scope_data = scopes.remove(scope_id).unwrap();

        // TODO: Children scopes drop

        scope_data.values.iter().copied().for_each(|id| {
            self.dispose(id);
        });
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

            let mut sources = self.sources.borrow_mut();
            if let Some(sources) = sources.entry(observer) {
                sources.or_default().borrow_mut().insert(id);
            }

            let mut subs = self.subscribers.borrow_mut();
            if let Some(subs) = subs.entry(id) {
                subs.or_default().borrow_mut().insert(observer);
            }
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
    ) {
        if self.is(id, ValueState::Check) {
            let sources = {
                // TODO: Optimize out cloned sources set. Maybe alloc a Vec instead of using BTreeSet.
                let subs = self.sources.borrow();
                subs.get(id).cloned().into_iter().flatten()
            };
            for source in sources {
                self.maybe_update(source, requester, caller);
                if self.is(id, ValueState::Dirty) {
                    // TODO: Cache check and use after break
                    break;
                }
            }
        }

        if self.is(id, ValueState::Dirty) {
            self.update(id, requester, caller);
        }

        // // TODO: Isn't marked clean twice?
        // self.mark_clean(id, requester, caller);
    }

    pub(crate) fn update(
        &self,
        id: ValueId,
        requester: Option<ValueId>,
        caller: &'static Location<'static>,
    ) {
        let value = self.storage.get(id);

        if let Some(value) = value {
            if self.defer_effects.get()
                && matches!(value.kind, ValueKind::Effect { .. })
            {
                self.pending_effects.borrow_mut().insert(id);
                return;
            }

            let changed = match value.kind {
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
                ValueKind::Observer => {
                    let mut value = value.value.borrow_mut();
                    let value = value.downcast_mut::<bool>().unwrap();
                    let changed = !*value;
                    *value = true;
                    changed
                },
            };

            if changed {
                if let Some(subs) = self.subscribers.borrow().get(id) {
                    for sub in subs {
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

        let mut deps = Vec::new();
        Self::get_deep_deps(&self.subscribers.borrow(), &mut deps, id);
        for dep in deps {
            self.mark_node(dep, ValueState::Check, requester, caller);
        }
    }

    fn get_deep_deps(
        subscribers: &SecondaryMap<ValueId, BTreeSet<ValueId>>,
        deps: &mut Vec<ValueId>,
        id: ValueId,
    ) {
        if let Some(subs) = subscribers.get(id) {
            for sub in subs {
                Self::get_deep_deps(subscribers, deps, *sub);
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
                        if let ValueKind::Observer = &value.kind {
                            if *value
                                .value
                                .borrow()
                                .downcast_ref::<bool>()
                                .unwrap()
                            {
                                "dirty"
                            } else {
                                "clean"
                            }
                            .to_string()
                        } else {
                            value.state.to_string()
                        }
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
            if let ValueDebugInfoState::CheckRequested(_, Some(requester))
            | ValueDebugInfoState::Dirten(_, Some(requester))
            | ValueDebugInfoState::Clean(Some(requester)) = debug_info.state
            {
                let (req_name, req_graph) = self.mermaid_subgraph(
                    requester,
                    depth + 1,
                    max_depth,
                    visited,
                );
                let arrow = if requester == id { "--" } else { "===" };
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
        let sources = self.sources.borrow();

        // TODO: Is it better not to cleanup the subs but only changes? Store new temporary subs and remove only not used in new run
        if let Some(sources) = sources.get(id) {
            let mut subs = self.subscribers.borrow_mut();
            for source in sources.iter() {
                if let Some(sources) = subs.get_mut(*source) {
                    sources.remove(&id);
                }
            }
        }

        // FIXME: I am deleting the values created in this observer, but they could be leaked outside.
        if let Some(owned) = self.owned.borrow_mut().get_mut(id) {
            owned.iter().copied().for_each(|owned| {
                // if let Some(value) = self.storage.get(owned) {
                //     if let ValueKind::Observer = value.kind {
                //         return;
                //     }
                // }
                self.dispose(owned);
            });
            owned.clear();
        }
    }

    pub(crate) fn defer_effects(&self) -> DeferEffectsGuard {
        // TODO: Panic if already true?
        self.defer_effects.set(true);
        DeferEffectsGuard
    }

    pub(crate) fn run_effects(
        &self,
        requester: Option<ValueId>,
        caller: &'static Location<'static>,
    ) {
        if !self.defer_effects.get() {
            self.pending_effects.take().iter().copied().for_each(|effect| {
                self.maybe_update(effect, requester, caller);
            });
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
        let (signals, effects, memos, computed, memo_chains) =
            rt.storage.values.borrow().values().fold(
                (0, 0, 0, 0, 0),
                |(
                    mut signals,
                    mut effects,
                    mut memos,
                    mut computed,
                    mut memo_chains,
                ),
                 value| {
                    match &value.kind {
                        ValueKind::Signal => signals += 1,
                        ValueKind::Effect { .. } => effects += 1,
                        ValueKind::Memo { .. } => memos += 1,
                        ValueKind::Computed { .. } => computed += 1,
                        ValueKind::MemoChain { .. } => memo_chains += 1,
                        ValueKind::Observer { .. } => {},
                    }

                    (signals, effects, memos, computed, memo_chains)
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
            self.signals + self.effects + self.memos + self.memo_chains
        )?;
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
            writeln!(f, "  by subscribers: {top_by_subs} ({count})")?;
        }

        #[cfg(feature = "debug-info")]
        if let Some((top_by_sources, count)) = self.top_by_sources {
            writeln!(f, "  by sources: {top_by_sources} ({count})")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{CURRENT_RUNTIME, observe};
    use crate::{
        memo::create_memo,
        read::ReadSignal,
        runtime::{RUNTIMES, with_new_runtime},
        signal::create_signal,
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
            observe(|| {
                signal.get();
                runs.set(runs.get() + 1);
            })
        };

        assert_eq!(run(), true);

        signal.set(2);

        assert_eq!(run(), true);
        assert_eq!(run(), false);
        assert_eq!(run(), false);
        assert_eq!(run(), false);
        assert_eq!(run(), false);

        assert_eq!(runs_count.get(), 2);
    }

    #[test]
    fn observe_works_with_memos() {
        let mut calls = create_signal(0);
        let mut a = create_signal(0);
        let a_is_even = create_memo(move || a.get() % 2 == 0);

        // Run observe only for even `a` values
        let mut run = move || {
            observe(|| {
                calls.update_untracked(|calls| *calls += 1);
                a_is_even.get();
            })
        };

        assert_eq!(run(), true, "observe didn't runs first time");
        assert_eq!(a_is_even.get(), true);
        assert_eq!(calls.get(), 1);
        assert_eq!(run(), false);

        a.set(3);
        assert_eq!(run(), true, "observe didn't run on value change");
        assert_eq!(a_is_even.get(), false);
        assert_eq!(calls.get(), 2);
        assert_eq!(run(), false);

        // `a` is still odd, so observe shouldn't rerun
        a.set(5);
        assert_eq!(run(), false, "observe rerun on unchanged memo");
        assert_eq!(a_is_even.get(), false);
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn recursive_observe() {
        let mut signal = create_signal(123);

        let mut run = move || {
            observe(|| {
                signal.get();
                signal.set(69);
            })
        };

        assert_eq!(run(), true);
        signal.set(0);
        assert_eq!(run(), true);
        assert_eq!(run(), false);
    }

    #[test]
    fn nested_observe() {
        let mut signal = create_signal(123);

        let run = move || {
            observe(move || {
                observe(|| {
                    signal.get();
                    signal.set(69);
                });
            })
        };

        assert_eq!(run(), true);
        signal.set(0);
        assert_eq!(run(), true);
        assert_eq!(run(), false);
    }
}
