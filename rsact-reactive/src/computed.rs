/*!  
 * A `Computed<T>` is a reactive lens into a [`crate::signal::Signal`].
 *
 * It is similar to a [`Memo`] but does **not** perform equality comparison:
 * every time its closure runs the result is stored and downstream subscribers
 * are always notified. Use `Computed` for values where equality testing is
 * expensive or meaningless (e.g. collections that are always rebuilt from
 * scratch).
 *
 * Prefer [`Memo`] when `T: PartialEq` and you want the runtime to skip
 * re-notifications when the value did not actually change.
 */

use crate::{
    ReactiveValue,
    callback::{AnyCallback, CallbackFn},
    memo::{Memo, create_memo},
    read::{ReadSignal, SignalMap},
    runtime::with_current_runtime,
    storage::ValueId,
};
use core::{marker::PhantomData, panic::Location};

/// Create a new [`Computed<T>`] in the current runtime scope.
///
/// Similar to [`crate::memo::Memo`] but without equality-gated notification:
/// downstream subscribers are re-evaluated on every update regardless of
/// whether `T` changed. Prefer [`create_memo`] when `T: PartialEq`.
pub fn create_computed<T: 'static, P: 'static>(
    f: impl CallbackFn<T, P> + 'static,
) -> Computed<T> {
    Computed::new(f)
}

pub(crate) struct ComputedCallback<T, F, P>
where
    F: CallbackFn<T, P>,
{
    pub f: F,
    pub ty: PhantomData<T>,
    pub p: PhantomData<P>,
}

impl<T: 'static> Computed<T> {
    #[track_caller]
    pub fn new<P: 'static>(f: impl CallbackFn<T, P>) -> Self {
        let caller = Location::caller();
        Self {
            id: with_current_runtime(|rt| rt.create_computed(f, caller)),
            ty: PhantomData,
        }
    }

    pub fn id(&self) -> ValueId {
        self.id
    }
}

impl<T: 'static, F, P> AnyCallback for ComputedCallback<T, F, P>
where
    F: CallbackFn<T, P>,
{
    fn run(
        &mut self,
        value: alloc::rc::Rc<core::cell::RefCell<dyn core::any::Any>>,
    ) -> bool {
        let new_value = {
            let value = value.borrow();
            let value = value.downcast_ref::<Option<T>>().unwrap().as_ref();

            let new_value = self.f.run(value);
            new_value
        };

        let mut value = value.borrow_mut();
        let value = value.downcast_mut::<Option<T>>().unwrap();
        value.replace(new_value);

        true
    }
}

/// A reactive derived value that always notifies subscribers when its
/// source changes, without equality checking.
///
/// See the [module-level documentation](self) for a comparison with [`crate::memo::Memo`].
/// Construct with [`create_computed`].
pub struct Computed<T> {
    id: ValueId,
    ty: PhantomData<T>,
}

impl<T: 'static, U: PartialEq + 'static> SignalMap<T, U> for Computed<T> {
    type Output = Memo<U>;

    fn map(
        &self,
        mut map: impl FnMut(&T) -> U + 'static,
    ) -> Self::Output {
        let this = *self;
        create_memo(move || this.with(&mut map))
    }
}

impl<T: 'static> ReactiveValue for Computed<T> {
    type Value = T;

    fn id(&self) -> Option<ValueId> {
        Some(self.id)
    }

    fn is_alive(&self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }

    unsafe fn dispose(self) { unsafe {
        with_current_runtime(|rt| rt.dispose(self.id))
    }}
}

impl<T: 'static> ReadSignal<T> for Computed<T> {
    #[track_caller]
    fn track(&self) {
        with_current_runtime(|rt| {
            self.id.subscribe(rt);
        })
    }

    #[track_caller]
    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        let caller = Location::caller();
        with_current_runtime(|rt| {
            self.id.with_untracked(
                rt,
                |memoized: &Option<T>| {
                    f(memoized.as_ref().expect("Must already been set"))
                },
                caller,
            )
        })
    }
}

impl<T> Clone for Computed<T> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), ty: self.ty.clone() }
    }
}

impl<T> Copy for Computed<T> {}
