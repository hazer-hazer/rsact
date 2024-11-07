use crate::{
    effect::{EffectCallback, EffectOrder},
    memo::MemoCallback,
    memo_chain::MemoChainCallback,
    scope::{ScopeData, ScopeHandle, ScopeId},
    storage::{
        Storage, StoredValue, ValueDebugInfo, ValueId, ValueKind, ValueState,
    },
};
use alloc::{
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
    let result = with_current_runtime(f);
    rt.leave();
    result
}

#[must_use]
#[inline(always)]
pub fn create_runtime() -> RuntimeId {
    RUNTIMES.with(|rts| rts.borrow_mut().insert(Runtime::new()))
}

/// Creates new scope, all reactive values will be dropped on scope drop. Scope dropped automatically when returned ScopeHandle drops.
#[must_use]
#[track_caller]
pub fn new_scope() -> ScopeHandle {
    let caller = Location::caller();
    with_current_runtime(|rt| {
        rt.new_scope(
            #[cfg(debug_assertions)]
            caller,
        )
    })
}

/// Creates new scope where creation of new reactive values is disallowed and will cause a panic. Useful mostly only for debugging.
#[track_caller]
pub fn new_deny_new_scope() -> ScopeHandle {
    let caller = Location::caller();
    with_current_runtime(|rt| {
        rt.new_deny_new_scope(
            #[cfg(debug_assertions)]
            caller,
        )
    })
}

crate::thread_local::thread_local_impl! {
    static CURRENT_RUNTIME: Cell<Option<RuntimeId>> = Cell::new(None);

    static RUNTIMES: RefCell<SlotMap<RuntimeId, Runtime>> = {
        let mut runtimes = SlotMap::default();

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
    // TODO: Use SlotMap
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
        #[cfg(debug_assertions)] created_at: &'static Location<'static>,
    ) -> ScopeHandle {
        let id = self.scopes.borrow_mut().insert(ScopeData::new(
            #[cfg(debug_assertions)]
            created_at,
        ));
        self.current_scope.set(Some(id));

        ScopeHandle::new(id)
    }

    pub fn new_deny_new_scope(
        &self,
        #[cfg(debug_assertions)] created_at: &'static Location<'static>,
    ) -> ScopeHandle {
        let id = self.scopes.borrow_mut().insert(ScopeData::new_deny_new(
            #[cfg(debug_assertions)]
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

        if let Some(ScopeData {
            deny_new: true,
            #[cfg(debug_assertions)]
            created_at,
            ..
        }) = scope
        {
            panic!("Creating new reactive values is disallowed in special `deny_new` scope. {}", if cfg!(debug_assertions) {
                created_at
            } else {
                Location::caller()
            })
        }

        let id = self.storage.add_value(value);

        if let Some(scope) = scope {
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
            #[cfg(debug_assertions)]
            debug: ValueDebugInfo {
                creator: Some(_caller),
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
        F: Fn(Option<T>) -> T + 'static,
    {
        self.add_value(StoredValue {
            value: Rc::new(RefCell::new(None::<T>)),
            kind: ValueKind::Effect {
                f: Rc::new(EffectCallback { f, ty: PhantomData }),
            },
            // Note: Check this, might need to be Dirty
            state: ValueState::Dirty,
            #[cfg(debug_assertions)]
            debug: ValueDebugInfo {
                creator: Some(_caller),
                dirten: None,
                borrowed: None,
                borrowed_mut: None,
                ty: Some(type_name::<F>()),
                observer: None,
            },
        })
    }

    #[track_caller]
    pub fn create_memo<T, F>(
        &self,
        f: F,
        _caller: &'static Location<'static>,
    ) -> ValueId
    where
        T: PartialEq + 'static,
        F: Fn(Option<&T>) -> T + 'static,
    {
        self.add_value(StoredValue {
            value: Rc::new(RefCell::new(None::<T>)),
            kind: ValueKind::Memo {
                f: Rc::new(MemoCallback { f, ty: PhantomData }),
            },
            state: ValueState::Dirty,
            #[cfg(debug_assertions)]
            debug: ValueDebugInfo {
                creator: Some(_caller),
                dirten: None,
                borrowed: None,
                borrowed_mut: None,
                ty: Some(type_name::<F>()),
                observer: None,
            },
        })
    }

    pub fn create_memo_chain<T, F>(
        &self,
        f: F,
        _caller: &'static Location<'static>,
    ) -> ValueId
    where
        T: PartialEq + 'static,
        F: Fn(Option<&T>) -> T + 'static,
    {
        self.add_value(StoredValue {
            value: Rc::new(RefCell::new(None::<T>)),
            kind: ValueKind::MemoChain {
                initial: Rc::new(MemoCallback { f, ty: PhantomData }),
                fs: Rc::new(RefCell::new(BTreeMap::new())),
            },
            state: ValueState::Dirty,
            #[cfg(debug_assertions)]
            debug: ValueDebugInfo {
                creator: Some(_caller),
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
                ValueKind::MemoChain { initial, fs } => {
                    let value = value.value;

                    self.with_observer(id, move |rt| {
                        rt.cleanup(id);

                        fs.borrow().values().fold(
                            initial.run(value.clone()),
                            |changed, cbs| {
                                cbs.iter().fold(changed, |changed, cb| {
                                    cb.run(value.clone()) || changed
                                })
                            },
                        )
                    })
                },
                ValueKind::Memo { f } | ValueKind::Effect { f } => {
                    let value = value.value;
                    self.with_observer(id, move |rt| {
                        rt.cleanup(id);

                        f.run(value)
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
        // TODO: Cleanup subs?
    }

    #[track_caller]
    pub(crate) fn mark_dirty(
        &self,
        id: ValueId,
        caller: Option<&'static Location<'static>>,
    ) {
        // self.mark_deep_dirty(id, caller);

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

    #[cfg(debug_assertions)]
    pub(crate) fn debug_info(&self, id: ValueId) -> ValueDebugInfo {
        let debug_info = self.storage.debug_info(id).unwrap();

        if let Some(ValueDebugInfo { creator: Some(observer), .. }) = self
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
        /*

        if let Some(children) = subscribers.get(node) {
            for child in children.borrow().iter() {
                descendants.insert(*child);
                Runtime::gather_descendants(subscribers, *child, descendants);
            }
        }
         */

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

    pub(crate) fn add_memo_chain<T: PartialEq + 'static>(
        &self,
        id: ValueId,
        order: EffectOrder,
        map: impl Fn(&T) -> T + 'static,
    ) {
        let kind = self.storage.get(id).unwrap().kind;
        match kind {
            ValueKind::MemoChain { initial: _, fs } => {
                fs.borrow_mut()
                    .entry(order)
                    .or_default()
                    .push(Rc::new(MemoChainCallback::new(map)));
            },
            _ => panic!("Cannot add memo chain to {}", kind),
        }
    }
}

pub fn current_runtime_profile() -> Profile {
    with_current_runtime(|rt| {
        let (signals, effects, memos, memo_chains) =
            rt.storage.values.borrow().values().fold(
                (0, 0, 0, 0),
                |(mut signals, mut effects, mut memos, mut memo_chains),
                 value| {
                    match &value.kind {
                        ValueKind::Signal => signals += 1,
                        ValueKind::Effect { .. } => effects += 1,
                        ValueKind::Memo { .. } => memos += 1,
                        ValueKind::MemoChain { .. } => memo_chains += 1,
                    }

                    (signals, effects, memos, memo_chains)
                },
            );

        let subscribers_bindings =
            rt.subscribers.borrow_mut().values().map(|subs| subs.len()).sum();
        let sources_bindings =
            rt.sources.borrow_mut().values().map(|sources| sources.len()).sum();

        Profile {
            signals,
            effects,
            memos,
            memo_chains,
            subscribers: rt.subscribers.borrow().len(),
            subscribers_bindings,
            sources: rt.sources.borrow().len(),
            sources_bindings,
            pending_effects: rt.pending_effects.borrow().len(),
        }
    })
}

#[derive(Clone, Copy)]
pub struct Profile {
    signals: usize,
    effects: usize,
    memos: usize,
    memo_chains: usize,
    subscribers: usize,
    subscribers_bindings: usize,
    sources: usize,
    sources_bindings: usize,
    pending_effects: usize,
}

impl Display for Profile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} values:\n  {} signals\n  {} effects\n  {} memos\n  {} memo chains\n{} subscribers ({} bindings), {} sources ({} bindings), {} pending effects",
            self.signals + self.effects + self.memos + self.memo_chains,
            self.signals,
            self.effects,
            self.memos,
            self.memo_chains,
            self.subscribers,
            self.subscribers_bindings,
            self.sources,
            self.sources_bindings,
            self.pending_effects
        )
    }
}

#[cfg(test)]
mod tests {
    use super::CURRENT_RUNTIME;
    use crate::runtime::RUNTIMES;

    #[test]
    fn primary_runtime() {
        assert!(
            RUNTIMES
                .with(|rts| rts.borrow().contains_key(CURRENT_RUNTIME.with(|current| current.get().unwrap()))),
            "First insertion into RUNTIMES does not have key of RuntimeId::default()"
        );
    }
}
