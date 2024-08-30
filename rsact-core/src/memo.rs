use core::{cell::RefCell, marker::PhantomData};

use alloc::rc::Rc;

use crate::{
    callback::{AnyCallback, CallbackResult},
    runtime::with_current_runtime,
    signal::ReadSignal,
    storage::ValueId,
};

pub struct MemoCallback<T, F>
where
    F: Fn(Option<&T>) -> T,
{
    pub(crate) f: F,
    pub(crate) ty: PhantomData<T>,
}

impl<T, F> AnyCallback for MemoCallback<T, F>
where
    F: Fn(Option<&T>) -> T,
    T: PartialEq + 'static,
{
    fn run(
        &self,
        value: Rc<RefCell<dyn core::any::Any>>,
    ) -> crate::callback::CallbackResult {
        let (new_value, changed) = {
            let value = value.borrow();
            let value = value.downcast_ref::<Option<T>>().unwrap().as_ref();

            let new_value = (self.f)(value);
            let changed = Some(&new_value) == value;
            (new_value, changed)
        };

        if changed {
            let mut value = value.borrow_mut();
            let value = value.downcast_mut::<Option<T>>().unwrap();
            value.replace(new_value);
        }

        if changed {
            CallbackResult::Changed
        } else {
            CallbackResult::None
        }
    }
}

pub struct Memo<T: PartialEq> {
    id: ValueId,
    ty: PhantomData<T>,
}

impl<T: PartialEq + 'static> ReadSignal<T> for Memo<T> {
    fn track(&self) {
        with_current_runtime(|rt| self.id.subscribe(rt))
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        with_current_runtime(|rt| self.id.with_untracked(rt, f))
    }
}
