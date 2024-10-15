use crate::{
    effect::EffectOrder,
    memo_chain::MemoChainCallback,
    storage::{Storage, ValueId, ValueKind, ValueState},
};
use alloc::{
    boxed::Box,
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

// TODO: Maybe better use Slab instead of SlotMap for efficiency?

impl RuntimeId {
    pub fn leave(&self) {
        let rt = RUNTIMES
            .lock()
            .remove(*self)
            .expect("Attempt to leave non-existent runtime");

        let mut current = CURRENT_RUNTIME.lock();
        if *current == Some(*self) {
            current.take();
        }

        drop(rt);
    }
}

#[inline(always)]
#[track_caller]
pub fn with_current_runtime<T>(f: impl FnOnce(&mut Runtime) -> T) -> T {
    let mut runtimes = RUNTIMES.try_lock().unwrap();
    let rt = runtimes.get_mut(CURRENT_RUNTIME.lock().unwrap()).unwrap();

    f(rt)
}

#[inline(always)]
pub fn with_scoped_runtime<T>(f: impl FnOnce(&mut Runtime) -> T) -> T {
    let rt = create_runtime();
    let result = with_current_runtime(f);
    rt.leave();
    result
}

#[must_use]
#[inline(always)]
pub fn create_runtime() -> RuntimeId {
    RUNTIMES.lock().insert(Runtime::new())
}

#[cfg(feature = "std")]
thread_local! {
    static CURRENT_RUNTIME: Cell<Option<RuntimeId>> = Mutex::new(None);
}

#[cfg(feature = "spin")]
static CURRENT_RUNTIME: spin::Mutex<Option<RuntimeId>> = Mutex::new(None);

#[cfg(feature = "mutex-critical-section")]
static CURRENT_RUNTIME: critical_section::Mutex<Option<RuntimeId>> =
    critical_section::Mutex::new(None);

static RUNTIMES: once_cell::sync::Lazy<Mutex<SlotMap<RuntimeId, Runtime>>> =
    once_cell::sync::Lazy::new(|| {
        let mut runtimes = SlotMap::default();
        {
            let mut current = CURRENT_RUNTIME.lock();
            *current = Some(runtimes.insert(Runtime::new()));
        }
        Mutex::new(runtimes)
    });

// #[derive(Clone, Copy, Default)]
// pub enum Observer {
//     /// Should only be used outside of actual reactive context.
//     #[default]
//     None,
//     Effect(ValueId),
// }

// #[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
// pub enum Observer {
//     /// TODO: Remove? Useless unreachable
//     None,
//     #[default]
//     Root,
//     // FIXME: Wrong name, can be Memo, not only Effect
//     Effect(ValueId),
// }

// struct PendingEffects {
//     ordered: BTreeMap<EffectOrder, Btre>,
// }

#[derive(Default)]
pub struct Runtime {
    pub(crate) storage: Storage,
    pub(crate) observer: Option<ValueId>,
    // TODO: Use SlotMap
    pub(crate) subscribers: BTreeMap<ValueId, BTreeSet<ValueId>>,
    pub(crate) sources: BTreeMap<ValueId, BTreeSet<ValueId>>,
    pub(crate) pending_effects: BTreeSet<ValueId>,
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
        }
    }

    #[track_caller]
    pub(crate) fn with_observer<T>(
        &mut self,
        observer: ValueId,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let prev_observer = self.observer;

        self.observer = Some(observer);

        let result = f(self);

        self.observer = prev_observer;

        result
    }

    pub(crate) fn subscribe(&mut self, id: ValueId) {
        if let Some(observer) = self.observer {
            self.sources.entry(observer).or_default().insert(id);

            self.subscribers.entry(id).or_default().insert(observer);
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

    pub(crate) fn maybe_update(&mut self, id: ValueId) {
        if self.is(id, ValueState::Check) {
            let subs = self.sources.get(&id).cloned().into_iter().flatten();
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

    pub(crate) fn update(&mut self, id: ValueId) {
        let value = self.storage.get_mut(id);

        if let Some(value) = value {
            let changed = match &value.kind {
                ValueKind::MemoChain { initial, fs } => {
                    // let value = value.value;

                    let prev_observer = self.observer;
                    self.observer = Some(id);
                    // self.cleanup(id);
                    if let Some(sources) = self.sources.get(&id) {
                        for source in sources.iter() {
                            if let Some(source) =
                                self.subscribers.get_mut(source)
                            {
                                source.remove(&id);
                            }
                        }
                    }

                    let result = fs.values().fold(
                        initial.run(value.value.as_mut()),
                        |changed, cbs| {
                            cbs.iter().fold(changed, |changed, cb| {
                                cb.run(value.value.as_mut()) || changed
                            })
                        },
                    );
                    self.observer = prev_observer;

                    result
                    // self.with_observer(id, |rt| {
                    //     rt.cleanup(id);

                    //     fs.values().fold(
                    //         initial.run(&mut value.value),
                    //         |changed, cbs| {
                    //             cbs.iter().fold(changed, |changed, cb| {
                    //                 cb.run(&mut value.value) || changed
                    //             })
                    //         },
                    //     )
                    // })
                },
                ValueKind::Memo { f } | ValueKind::Effect { f } => {
                    let prev_observer = self.observer;
                    self.observer = Some(id);
                    if let Some(sources) = self.sources.get(&id) {
                        for source in sources.iter() {
                            if let Some(source) =
                                self.subscribers.get_mut(source)
                            {
                                source.remove(&id);
                            }
                        }
                    }

                    let result = f.run(value.value.as_mut());
                    self.observer = prev_observer;

                    result
                    // self
                    // .with_observer(id, |rt| {
                    //     rt.cleanup(id);

                    //     f.run(&mut value.value)
                    // })},
                },
                ValueKind::Signal { .. } => true,
            };

            if changed {
                if let Some(subs) = self.subscribers.get(&id) {
                    for sub in subs {
                        self.storage.mark(*sub, ValueState::Dirty, None);
                    }
                }
            }

            self.mark_clean(id);
        }
    }

    #[track_caller]
    pub(crate) fn mark_clean(&mut self, id: ValueId) {
        self.storage.mark(id, ValueState::Clean, None);
        // TODO: Cleanup subs?
    }

    #[track_caller]
    pub(crate) fn mark_dir(
        &mut self,
        id: ValueId,
        caller: Option<&'static Location<'static>>,
    ) {
        // self.mark_deep_dirty(id, caller);

        self.mark_node(id, ValueState::Dirty, caller);

        let mut deps = Vec::new();
        Self::get_deep_deps(&self.subscribers, &mut deps, id);
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
        &mut self,
        id: ValueId,
        state: ValueState,
        caller: Option<&'static Location<'static>>,
    ) {
        if self.state(id) <= state {
            self.storage.mark(id, state, caller);
        }

        if let Some(node) = self.storage.get(id) {
            if let (ValueKind::Effect { .. }, true) =
                (&node.kind, self.observer != Some(id))
            {
                self.pending_effects.insert(id);
            }
        }
    }

    fn cleanup(&mut self, id: ValueId) {
        if let Some(sources) = self.sources.get(&id) {
            for source in sources.iter() {
                if let Some(source) = self.subscribers.get_mut(source) {
                    source.remove(&id);
                }
            }
        }
    }

    #[track_caller]
    pub(crate) fn run_effects(&mut self) {
        while let Some(effect) = self.pending_effects.pop_last() {
            self.maybe_update(effect);
        }
    }

    pub(crate) fn add_memo_chain<T: PartialEq + Send + 'static>(
        &mut self,
        id: ValueId,
        order: EffectOrder,
        map: impl Fn(&T) -> T + Send + 'static,
    ) {
        let kind = &mut self.storage.get_mut(id).unwrap().kind;
        match kind {
            ValueKind::MemoChain { initial: _, fs } => {
                fs.entry(order)
                    .or_default()
                    .push(Box::new(MemoChainCallback::new(map)));
            },
            _ => panic!("Cannot add memo chain to {}", kind),
        }
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
                // .borrow()
                .lock()
                .contains_key(CURRENT_RUNTIME.lock().unwrap()),
            "First insertion into RUNTIMES does not have key of RuntimeId::default()"
        );
    }
}
