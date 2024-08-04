use core::{any::Any, cell::RefCell, marker::PhantomData};

use alloc::rc::Rc;

use crate::{runtime::with_current_runtime, storage::ValueId};

pub struct Effect<T> {
    id: ValueId,
    ty: PhantomData<T>,
}

impl<T> Effect<T> {
    fn new<F>(f: F) -> Self
    where
        T: 'static,
        F: Fn(Option<T>) -> T + 'static,
        // F: Fn(Option<T>) -> T + 'static + Send,
    {
        let effect = with_current_runtime(|rt| rt.storage.create_effect::<T, F>(f));

        // Run effect on initial value to observe it in the future

        Self {
            id: effect,
            ty: PhantomData,
        }
    }
}

/**
 * Effects work by this principle:
 * Reactive values are copy-type identifiers that point to actual values. When we create `move || { some_reactive_value }` and pass this closure to effect, we can save the scope in which this closure will run, and only when closure is called, `some_reactive_value` is copied and subscribed to this scope.
 */

// pub struct StoredEffect {
//     f: Box<dyn AnyCallback>,
// }

// impl StoredEffect {
//     pub(crate) fn run(&self, value: Rc<RefCell<dyn Any>>) {
//         self.f.run(value)
//     }

//     pub(crate) fn value(&self) -> ValueId {
//         self.value
//     }
// }

pub trait AnyCallback {
    fn run(&self, value: Rc<RefCell<dyn Any>>);
}

pub struct EffectCallback<T, F>
where
    F: Fn(Option<T>) -> T,
{
    pub f: F,
    pub ty: PhantomData<T>,
}

impl<T: 'static, F> AnyCallback for EffectCallback<T, F>
where
    F: Fn(Option<T>) -> T,
{
    fn run(&self, value: Rc<RefCell<dyn Any>>) {
        let pass_value = {
            // Create RefMut dropped in this scope to avoid mutual exclusion problem
            let mut pass_value = RefCell::borrow_mut(&value);
            let pass_value = pass_value.downcast_mut::<Option<T>>().unwrap().take();
            pass_value
        };

        let new_value = (self.f)(pass_value);

        let mut value = RefCell::borrow_mut(&value);
        value
            .downcast_mut::<Option<T>>()
            .unwrap()
            .replace(new_value);
    }
}

// slotmap::new_key_type! {
//     pub struct ScopeId;
// }

// impl ScopeId {
//     pub(crate) fn with_runtime(&self, runtime: &Runtime, f: impl FnOnce(&StoredEffect)) {
//         let effects = runtime.effects.effects.borrow();
//         let effect = effects.get(*self).unwrap();
//         f(effect);
//     }
// }

// #[derive(Default)]
// pub struct EffectStorage {
//     effects: RefCell<SlotMap<ScopeId, StoredEffect>>,
// }

// impl EffectStorage {
//     pub(crate) fn create<T, F>(&self, f: F) -> ScopeId
//     where
//         T: 'static,
//         F: Fn(Option<T>) -> T + 'static,
//     {
//         let value =
//         self.effects.borrow_mut().insert(StoredEffect {
//             f: Box::new(EffectCallback {
//                 f,
//                 ty: PhantomData::<T>,
//             }),
//             value: None,
//         })
//     }
// }

pub fn create_effect<T, F>(f: F) -> Effect<T>
where
    T: 'static,
    F: Fn(Option<T>) -> T + 'static,
{
    let effect = Effect::new(f);

    with_current_runtime(|rt| rt.maybe_update(effect.id));

    effect
}
