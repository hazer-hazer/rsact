use crate::{
    callback::AnyCallback, runtime::with_current_runtime, storage::ValueId,
};
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell, marker::PhantomData, panic::Location};

pub fn create_effect<T, F>(f: F) -> Effect<T>
where
    T: 'static,
    F: FnMut(Option<T>) -> T + 'static,
{
    let effect = Effect::new(f);

    with_current_runtime(|rt| rt.maybe_update(effect.id));

    effect
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum EffectOrder {
    First,
    #[default]
    Normal,
    Last,
}

impl EffectOrder {
    pub fn each() -> impl Iterator<Item = EffectOrder> {
        [Self::First, Self::Normal, Self::Last].iter().copied()
    }
}

pub struct Effect<T> {
    id: ValueId,
    ty: PhantomData<T>,
}

impl<T> Effect<T> {
    #[track_caller]
    fn new<F>(f: F) -> Self
    where
        T: 'static,
        F: FnMut(Option<T>) -> T + 'static,
    {
        let caller = Location::caller();
        let effect = with_current_runtime(|rt| rt.create_effect(f, caller));

        Self { id: effect, ty: PhantomData }
    }

    pub fn is_alive(self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }
}

pub struct EffectCallback<T, F>
where
    F: FnMut(Option<T>) -> T,
{
    pub f: F,
    pub ty: PhantomData<T>,
}

impl<T: 'static, F> AnyCallback for EffectCallback<T, F>
where
    F: FnMut(Option<T>) -> T,
{
    fn run(&mut self, value: Rc<RefCell<dyn Any>>) -> bool {
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

        true
    }
}

#[cfg(test)]
mod tests {
    use super::create_effect;
    use crate::prelude::*;

    #[test]
    fn effects_work() {
        let mut calls = create_signal(0);
        let mut a = create_signal(0);

        create_effect(move |_| {
            calls.update_untracked(|calls| *calls += 1);
            a.get();
        });

        assert_eq!(calls.get(), 1);

        a.set(1);
        assert_eq!(calls.get(), 2);

        a.set(2);
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn no_unnecessary_rerun() {
        let mut calls = create_signal(0);
        let mut a = create_signal(0);
        let a_is_even = create_memo(move |_| a.get() % 2 == 0);

        // Run effect only for even `a` values
        create_effect(move |_| {
            calls.update_untracked(|calls| *calls += 1);
            a_is_even.get();
        });

        assert_eq!(a_is_even.get(), true);
        assert_eq!(calls.get(), 1);

        a.set(3);
        assert_eq!(a_is_even.get(), false);
        assert_eq!(calls.get(), 2);

        // `a` is still odd, so effect doesn't rerun
        a.set(5);
        assert_eq!(a_is_even.get(), false);
        assert_eq!(calls.get(), 2);
    }
}
