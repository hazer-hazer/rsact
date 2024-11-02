use crate::{
    effect::EffectOrder,
    memo_chain::MemoChainCallback,
    storage::{Storage, ValueDebugInfo, ValueId, ValueKind, ValueState},
};
use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    rc::Rc,
    vec::Vec,
};
use core::{
    cell::{Cell, RefCell},
    panic::Location,
};
use slotmap::SlotMap;

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
pub fn with_scoped_runtime<T>(f: impl FnOnce(&Runtime) -> T) -> T {
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

crate::thread_local::thread_local_impl! {
    static CURRENT_RUNTIME: Cell<Option<RuntimeId>> = Cell::new(None);

    static RUNTIMES: RefCell<SlotMap<RuntimeId, Runtime>> = {
        let mut runtimes = SlotMap::default();

        CURRENT_RUNTIME.with(|current| current.set(Some(runtimes.insert(Runtime::new()))));

        RefCell::new(runtimes)
    };
}

#[derive(Default)]
pub struct Runtime {
    pub(crate) storage: Storage,
    pub(crate) observer: Cell<Option<ValueId>>,
    // TODO: Use SlotMap
    pub(crate) subscribers: RefCell<BTreeMap<ValueId, BTreeSet<ValueId>>>,
    pub(crate) sources: RefCell<BTreeMap<ValueId, BTreeSet<ValueId>>>,
    pub(crate) pending_effects: RefCell<BTreeSet<ValueId>>,
    // pub(crate) updating: Cell<usize>,
}

impl Runtime {
    fn new() -> Self {
        Self {
            storage: Default::default(),
            subscribers: Default::default(),
            sources: Default::default(),
            // observer: Default::default(),
            observer: Default::default(),
            pending_effects: Default::default(),
            // updating: Cell::new(0),
        }
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
        if let Some(observer) = self.observer.get() {
            self.sources.borrow_mut().entry(observer).or_default().insert(id);

            self.subscribers
                .borrow_mut()
                .entry(id)
                .or_default()
                .insert(observer);
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
                subs.get(&id).cloned().into_iter().flatten()
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
                if let Some(subs) = self.subscribers.borrow().get(&id) {
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
        subscribers: &BTreeMap<ValueId, BTreeSet<ValueId>>,
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

        if let Some(subs) = subscribers.get(&id) {
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
        let sources = self.sources.borrow_mut();

        if let Some(sources) = sources.get(&id) {
            let mut subs = self.subscribers.borrow_mut();
            for source in sources.iter() {
                if let Some(source) = subs.get_mut(source) {
                    source.remove(&id);
                }
            }
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

pub struct Profile {
    
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
