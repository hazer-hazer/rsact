use crate::{
    callback::AnyCallback, runtime::with_current_runtime, storage::ValueId,
};
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell, marker::PhantomData};

pub struct Effect<T> {
    id: ValueId,
    ty: PhantomData<T>,
}

impl<T> Effect<T> {
    fn new<F>(f: F) -> Self
    where
        T: 'static,
        F: Fn(Option<T>) -> T + 'static,
    {
        let effect = with_current_runtime(|rt| rt.storage.create_effect(f));

        Self { id: effect, ty: PhantomData }
    }
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
            // Create RefMut dropped in this scope and take it to avoid mutual
            // exclusion problem
            let mut pass_value = RefCell::borrow_mut(&value);
            let pass_value =
                pass_value.downcast_mut::<Option<T>>().unwrap().take();
            pass_value
        };

        let new_value = (self.f)(pass_value);

        let mut value = RefCell::borrow_mut(&value);
        value.downcast_mut::<Option<T>>().unwrap().replace(new_value);
    }
}

pub fn use_effect<T, F>(f: F) -> Effect<T>
where
    T: 'static,
    F: Fn(Option<T>) -> T + 'static,
{
    let effect = Effect::new(f);

    with_current_runtime(|rt| rt.maybe_update(effect.id));

    effect
}
