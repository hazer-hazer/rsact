use crate::{
    callback::CallbackFn,
    computed::ComputedCallback,
    effect::EffectCallback,
    memo::MemoCallback,
    memo_chain::{MemoChainCallback, MemoChainErr},
    scope::{ScopeData, ScopeHandle, ScopeId},
    storage::{
        Storage, StoredValue, ValueDebugInfo, ValueId, ValueKind, ValueState,
    },
};
use alloc::{
    boxed::Box,
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    rc::Rc,
    vec::Vec,
};
use core::{
    any::type_name,
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

// TODO: Debug call-stack. Value get -> value get -> ... -> value get
#[derive(Default)]
pub struct Runtime {
    pub(crate) storage: Storage,
    scopes: RefCell<SlotMap<ScopeId, ScopeData>>,
    current_scope: Cell<Option<ScopeId>>,
    /// Values owned by observers
    owned: RefCell<SecondaryMap<ValueId, BTreeSet<ValueId>>>,
    pub(crate) observer: Cell<Option<ValueId>>,
    pub(crate) subscribers: RefCell<SecondaryMap<ValueId, BTreeSet<ValueId>>>,
    pub(crate) sources: RefCell<SecondaryMap<ValueId, BTreeSet<ValueId>>>,
    pub(crate) pending_effects: RefCell<BTreeSet<ValueId>>,
    // pub(crate) updating: Cell<usize>,
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
            // observer: Default::default(),
            observer: Default::default(),
            pending_effects: Default::default(),
            // updating: Cell::new(0),
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

    #[track_caller]
    fn add_value(&self, value: StoredValue) -> ValueId {
        let mut scopes = self.scopes.borrow_mut();
        let scope = self
            .current_scope
            .get()
            .map(|current| scopes.get_mut(current))
            .flatten();

        let id = self.storage.add_value(value);

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

    #[track_caller]
    pub fn create_signal<T: 'static>(
        &self,
        value: T,
        _caller: &'static Location<'static>,
    ) -> ValueId {
        self.add_value(StoredValue {
            value: Rc::new(RefCell::new(value)),
            kind: ValueKind::Signal,
            state: ValueState::Clean,
            #[cfg(feature = "debug-info")]
            debug: ValueDebugInfo {
                created_at: Some(_caller),
                dirten: None,
                borrowed: None,
                borrowed_mut: None,
                ty: Some(type_name::<T>()),
                observer: None,
            },
        })
    }

    #[track_caller]
    pub fn create_effect<T, F>(
        &self,
        f: F,
        _caller: &'static Location<'static>,
    ) -> ValueId
    where
        T: 'static,
        F: FnMut(Option<T>) -> T + 'static,
    {
        self.add_value(StoredValue {
            value: Rc::new(RefCell::new(None::<T>)),
            kind: ValueKind::Effect {
                f: Rc::new(RefCell::new(EffectCallback { f, ty: PhantomData })),
            },
            state: ValueState::Dirty,
            #[cfg(feature = "debug-info")]
            debug: ValueDebugInfo {
                created_at: Some(_caller),
                dirten: None,
                borrowed: None,
                borrowed_mut: None,
                ty: Some(type_name::<F>()),
                observer: None,
            },
        })
    }

    #[track_caller]
    pub fn create_memo<T, F, P: 'static>(
        &self,
        f: F,
        _caller: &'static Location<'static>,
    ) -> ValueId
    where
        T: PartialEq + 'static,
        F: CallbackFn<T, P> + 'static,
    {
        self.add_value(StoredValue {
            value: Rc::new(RefCell::new(None::<T>)),
            kind: ValueKind::Memo {
                f: Rc::new(RefCell::new(MemoCallback {
                    f,
                    ty: PhantomData,
                    p: PhantomData,
                })),
            },
            state: ValueState::Dirty,
            #[cfg(feature = "debug-info")]
            debug: ValueDebugInfo {
                created_at: Some(_caller),
                dirten: None,
                borrowed: None,
                borrowed_mut: None,
                ty: Some(type_name::<F>()),
                observer: None,
            },
        })
    }

    #[track_caller]
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
        self.add_value(StoredValue {
            value: Rc::new(RefCell::new(None::<T>)),
            kind: ValueKind::Computed {
                f: Rc::new(RefCell::new(ComputedCallback {
                    f,
                    ty: PhantomData,
                    p: PhantomData,
                })),
            },
            state: ValueState::Dirty,
            #[cfg(feature = "debug-info")]
            debug: ValueDebugInfo {
                created_at: Some(_caller),
                dirten: None,
                borrowed: None,
                borrowed_mut: None,
                ty: Some(type_name::<F>()),
                observer: None,
            },
        })
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
        self.add_value(StoredValue {
            value: Rc::new(RefCell::new(None::<T>)),
            kind: ValueKind::MemoChain {
                memo: Rc::new(RefCell::new(MemoCallback {
                    f,
                    ty: PhantomData,
                    p: PhantomData,
                })),
                first: Rc::new(RefCell::new(None)),
                last: Rc::new(RefCell::new(None)),
            },
            state: ValueState::Dirty,
            #[cfg(feature = "debug-info")]
            debug: ValueDebugInfo {
                created_at: Some(_caller),
                dirten: None,
                borrowed: None,
                borrowed_mut: None,
                ty: Some(type_name::<F>()),
                observer: None,
            },
        })
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

    #[track_caller]
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

    pub(crate) fn maybe_update(&self, id: ValueId) {
        if self.is(id, ValueState::Check) {
            let subs = {
                let subs = self.sources.borrow();
                subs.get(id).cloned().into_iter().flatten()
            };
            for source in subs {
                self.maybe_update(source);
                if self.is(id, ValueState::Dirty) {
                    break;
                }
            }
        }

        if self.is(id, ValueState::Dirty) {
            self.update(id);
        }

        self.mark_clean(id);
    }

    pub(crate) fn update(&self, id: ValueId) {
        let value = self.storage.get(id);

        if let Some(value) = value {
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
            };

            if changed {
                if let Some(subs) = self.subscribers.borrow().get(id) {
                    for sub in subs {
                        self.storage.mark(*sub, ValueState::Dirty, None);
                    }
                }
            }

            self.mark_clean(id);
        }
    }

    #[track_caller]
    pub(crate) fn mark_clean(&self, id: ValueId) {
        self.storage.mark(id, ValueState::Clean, None);
    }

    #[track_caller]
    pub(crate) fn mark_dirty(
        &self,
        id: ValueId,
        caller: Option<&'static Location<'static>>,
    ) {
        self.mark_node(id, ValueState::Dirty, caller);

        let mut deps = Vec::new();
        Self::get_deep_deps(&self.subscribers.borrow(), &mut deps, id);
        for dep in deps {
            self.mark_node(dep, ValueState::Check, caller);
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
    pub fn debug_info(&self, id: ValueId) -> ValueDebugInfo {
        let debug_info = self.storage.debug_info(id).unwrap();

        if let Some(ValueDebugInfo { created_at: Some(observer), .. }) = self
            .observer
            .get()
            .map(|observer| self.storage.debug_info(observer))
            .flatten()
        {
            debug_info.with_observer(observer)
        } else {
            debug_info
        }
    }

    fn get_deep_deps(
        subscribers: &SecondaryMap<ValueId, BTreeSet<ValueId>>,
        deps: &mut Vec<ValueId>,
        id: ValueId,
    ) {
        if let Some(subs) = subscribers.get(id) {
            for sub in subs {
                deps.push(*sub);
                Self::get_deep_deps(subscribers, deps, *sub);
            }
        }
    }

    #[track_caller]
    fn mark_node(
        &self,
        id: ValueId,
        state: ValueState,
        caller: Option<&'static Location<'static>>,
    ) {
        if state > self.state(id) {
            self.storage.mark(id, state, caller);
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
                self.dispose(owned);
            });
            owned.clear();
        }
    }

    #[track_caller]
    pub(crate) fn run_effects(&self) {
        // if self.updating.get() == 0 {
        self.pending_effects.take().iter().copied().for_each(|effect| {
            self.maybe_update(effect);
        });
        // }
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
            .and_then(|(top_by_subs, subs)| {
                rt.storage
                    .values
                    .borrow()
                    .get(top_by_subs)
                    .unwrap()
                    .debug
                    .created_at
                    .map(|created_at| (*created_at, subs))
            });

        #[cfg(feature = "debug-info")]
        let top_by_sources = rt
            .sources
            .borrow()
            .iter()
            .map(|(id, sources)| (id, sources.len()))
            .max_by_key(|(_, sources)| *sources)
            .and_then(|(top_by_sources, sources)| {
                rt.storage
                    .values
                    .borrow()
                    .get(top_by_sources)
                    .unwrap()
                    .debug
                    .created_at
                    .map(|created_at| (*created_at, sources))
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
    top_by_subs: Option<(Location<'static>, usize)>,
    #[cfg(feature = "debug-info")]
    top_by_sources: Option<(Location<'static>, usize)>,
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
    use super::CURRENT_RUNTIME;
    use crate::runtime::RUNTIMES;

    #[test]
    fn primary_runtime() {
        assert!(
            RUNTIMES.with(|rts| rts.borrow().contains_key(
                CURRENT_RUNTIME.with(|current| current.get().unwrap())
            )),
            "First insertion into RUNTIMES does not have key of RuntimeId::default()"
        );
    }
}
