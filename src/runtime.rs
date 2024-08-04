use core::cell::{Cell, RefCell};

use alloc::collections::{btree_map::BTreeMap, btree_set::BTreeSet};
use lazy_static::lazy_static;
use slotmap::SlotMap;

use crate::storage::{Storage, ValueId, ValueKind, ValueState};

slotmap::new_key_type! {
    pub struct RuntimeId;
}

impl RuntimeId {
    pub fn leave(&self) {
        critical_section::with(|cs| {
            let rt = RUNTIMES
                .borrow_ref_mut(cs)
                .remove(*self)
                .expect("Attempt to leave non-existent runtime");

            if CURRENT_RUNTIME.borrow(cs).get() == Some(*self) {
                CURRENT_RUNTIME.borrow(cs).take();
            }

            drop(rt);
        });
    }
}

#[inline(always)]
pub fn with_current_runtime<T>(f: impl FnOnce(&Runtime) -> T) -> T {
    critical_section::with(|cs| {
        let runtimes = RUNTIMES.borrow_ref(cs);
        let rt = runtimes
            .get(CURRENT_RUNTIME.borrow(cs).get().unwrap())
            .unwrap();

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
    critical_section::with(|cs| RUNTIMES.borrow_ref_mut(cs).insert(Runtime::new()))
}

// Note: Multiple runtimes are now only used in tests
lazy_static! {
    static ref RUNTIMES: critical_section::Mutex<RefCell<SlotMap<RuntimeId, Runtime>>> = {
        let mut runtimes = SlotMap::default();
        let primary_rt = runtimes.insert(Runtime::new());
        critical_section::with(|cs| {
            CURRENT_RUNTIME.borrow(cs).set(Some(primary_rt));
        });
        critical_section::Mutex::new(RefCell::new(runtimes))
    };
    static ref CURRENT_RUNTIME: critical_section::Mutex<Cell<Option<RuntimeId>>> =
        critical_section::Mutex::new(Cell::new(None));
}

// #[derive(Clone, Copy)]
// pub struct ScopeId(usize);

#[derive(Default)]
pub struct Runtime {
    pub(crate) storage: Storage,
    pub(crate) observer: Cell<Option<ValueId>>,
    // pub(crate) watchers: SlotMap<ValueId, Vec<ScopeId>>,
    // TODO: Use slotmap
    pub(crate) subscribers: RefCell<BTreeMap<ValueId, BTreeSet<ValueId>>>,
    pub(crate) pending_effects: RefCell<BTreeSet<ValueId>>,
}

// AHAHAHAHAHAHHAHAAH
unsafe impl Send for Runtime {}

impl Runtime {
    fn new() -> Self {
        Self {
            storage: Default::default(),
            subscribers: Default::default(),
            observer: Default::default(),
            pending_effects: Default::default(),
        }
    }

    pub(crate) fn with_observer<T>(&self, observer: ValueId, f: impl FnOnce(&Self) -> T) -> T {
        let prev_observer = self.observer.get();

        self.observer.set(Some(observer));

        let result = f(self);

        self.observer.set(prev_observer);

        result
    }

    pub(crate) fn subscribe(&self, id: ValueId) {
        self.subscribers
            .borrow_mut()
            .entry(id)
            .or_default()
            .insert(self.observer.get().expect(
                "[BUG] Attempt to subscribe to reactive value updates out of reactive context.",
            ));
    }

    pub(crate) fn maybe_update(&self, id: ValueId) {
        if self.storage.get(id).state == ValueState::Dirty {
            self.update(id);
        }

        self.mark_clean(id);
    }

    pub(crate) fn update(&self, id: ValueId) {
        let value = self.storage.get(id);

        match value.kind {
            ValueKind::Signal => {}
            ValueKind::Effect { f } => {
                let effect_value = value.value;
                self.with_observer(id, move |_rt| {
                    f.run(effect_value);
                })
            }
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
            && self.observer.get() != Some(id)
        {
            let mut pending_effects = RefCell::borrow_mut(&self.pending_effects);
            pending_effects.insert(id);
        }

        if let Some(subscribers) = self.subscribers.borrow().get(&id) {
            subscribers.iter().copied().for_each(|sub| {
                self.mark_dirty_recursive(sub);
            });
        }
    }

    pub(crate) fn run_effects(&self) {
        self.pending_effects
            .take()
            .iter()
            .copied()
            .for_each(|effect| {
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
        critical_section::with(|cs| {
            assert!(
                RUNTIMES
                    .borrow(cs)
                    .borrow()
                    .contains_key(CURRENT_RUNTIME.borrow(cs).get().unwrap()),
                "First insertion into RUNTIMES does not have key of RuntimeId::default()"
            );
        });
    }
}
