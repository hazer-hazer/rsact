use crate::storage::{Storage, ValueId, ValueKind, ValueState};
use alloc::collections::{btree_map::BTreeMap, btree_set::BTreeSet};
use core::{
    cell::{Cell, RefCell},
    default,
};
use slotmap::SlotMap;

slotmap::new_key_type! {
    pub struct RuntimeId;
}

// TODO: Maybe better use Slab instead of SlotMap for efficiency

impl RuntimeId {
    pub fn leave(&self) {
        let rt = RUNTIMES
            .borrow_mut()
            .remove(*self)
            .expect("Attempt to leave non-existent runtime");

        if CURRENT_RUNTIME.get() == Some(*self) {
            CURRENT_RUNTIME.take();
        }

        drop(rt);
    }
}

#[inline(always)]
pub fn with_current_runtime<T>(f: impl FnOnce(&Runtime) -> T) -> T {
    let runtimes = RUNTIMES.borrow();
    let rt = runtimes.get(CURRENT_RUNTIME.get().unwrap()).unwrap();

    f(rt)
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
    RUNTIMES.borrow_mut().insert(Runtime::new())
}

#[thread_local]
static CURRENT_RUNTIME: Cell<Option<RuntimeId>> = Cell::new(None);

#[thread_local]
static RUNTIMES: once_cell::unsync::Lazy<RefCell<SlotMap<RuntimeId, Runtime>>> =
    once_cell::unsync::Lazy::new(|| {
        let mut runtimes = SlotMap::default();

        CURRENT_RUNTIME.set(Some(runtimes.insert(Runtime::new())));

        RefCell::new(runtimes)
    });

// #[derive(Clone, Copy, Default)]
// pub enum Observer {
//     /// Should only be used outside of actual reactive context.
//     #[default]
//     None,
//     Effect(ValueId),
// }

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Observer {
    /// TODO: Remove? Useless unreachable
    None,
    #[default]
    Root,
    Effect(ValueId),
}

#[derive(Default)]
pub struct Runtime {
    pub(crate) storage: Storage,
    pub(crate) observer: Cell<Observer>,
    pub(crate) subscribers: RefCell<BTreeMap<ValueId, BTreeSet<ValueId>>>,
    pub(crate) pending_effects: RefCell<BTreeSet<ValueId>>,
}

impl Runtime {
    fn new() -> Self {
        Self {
            storage: Default::default(),
            subscribers: Default::default(),
            observer: Default::default(),
            pending_effects: Default::default(),
        }
    }

    pub(crate) fn with_observer<T>(
        &self,
        observer: Observer,
        f: impl FnOnce(&Self) -> T,
    ) -> T {
        let prev_observer = self.observer.get();

        self.observer.set(observer);

        let result = f(self);

        self.observer.set(prev_observer);

        result
    }

    pub(crate) fn subscribe(&self, id: ValueId) {
        match self.observer.get() {
            Observer::None => panic!(
                "[BUG] Attempt to subscribe to reactive value updates out of reactive context.",
            ),
            Observer::Root => todo!(),
            Observer::Effect(observer) => {
                self.subscribers
                    .borrow_mut()
                    .entry(id)
                    .or_default()
                    .insert(observer);
            },
        }
    }

    pub(crate) fn maybe_update(&self, id: ValueId) {
        if self.storage.get(id).state == ValueState::Dirty {
            self.notify_updated(id);
        }

        self.mark_clean(id);
    }

    pub(crate) fn notify_updated(&self, id: ValueId) {
        let value = self.storage.get(id);

        match value.kind {
            ValueKind::Signal => {},
            ValueKind::Effect { f } => {
                let effect_value = value.value;
                self.with_observer(Observer::Effect(id), move |_rt| {
                    f.run(effect_value);
                })
            },
            ValueKind::Operator { mut scheduled, operator } => {
                let value = value.value;
                scheduled.entry(self.observer).or_default().push(value)
                if let Some(scheduled) = scheduled.remove(&self.observer.get())
                {
                    scheduled.into_iter().for_each(|op| {
                        operator.operate(op, value.clone());
                    });
                }
            },
        }

        self.mark_dirty(id);
    }

    fn mark_clean(&self, id: ValueId) {
        self.storage.mark(id, ValueState::Clean);
        // TODO: Cleanup subs?
    }

    pub(crate) fn mark_dirty(&self, id: ValueId) {
        self.mark_dirty_recursive(id)
    }

    fn mark_dirty_recursive(&self, id: ValueId) {
        self.storage.mark(id, ValueState::Dirty);

        if matches!(self.storage.get(id).kind, ValueKind::Effect { .. })
            && self.observer.get() != Observer::Effect(id)
        {
            let mut pending_effects =
                RefCell::borrow_mut(&self.pending_effects);
            pending_effects.insert(id);
        }

        if let Some(subscribers) = self.subscribers.borrow().get(&id) {
            subscribers.iter().copied().for_each(|sub| {
                self.mark_dirty_recursive(sub);
            });
        }
    }

    pub(crate) fn run_effects(&self) {
        self.pending_effects.take().iter().copied().for_each(|effect| {
            self.maybe_update(effect);
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime::RUNTIMES;

    use super::CURRENT_RUNTIME;

    #[test]
    fn primary_runtime() {
        assert!(
            RUNTIMES
                .borrow()
                .contains_key(CURRENT_RUNTIME.get().unwrap()),
            "First insertion into RUNTIMES does not have key of RuntimeId::default()"
        );
    }
}
