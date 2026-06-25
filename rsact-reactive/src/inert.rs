use crate::{
    ReactiveValue,
    memo::{IntoMemo, Memo},
    read::{ReadSignal, SignalMap, impl_read_signal_traits},
    runtime::with_current_runtime,
    storage::ValueId,
};
use core::marker::PhantomData;

// TODO: Maybe can optimize this to a simple Rc<RefCell<T>> to avoid downcasting
// in runtime and lookups, while still we need the runtime to know about this
// value to be cleared when the scope is disposed.
pub struct Inert<T: ?Sized> {
    id: ValueId,
    ty: PhantomData<T>,
}

impl<T> Clone for Inert<T> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), ty: self.ty.clone() }
    }
}
impl<T> Copy for Inert<T> {}

impl_read_signal_traits!(Inert<T>);

impl<T: 'static> From<T> for Inert<T> {
    #[track_caller]
    fn from(value: T) -> Self {
        let caller = core::panic::Location::caller();

        Inert {
            id: with_current_runtime(|rt| rt.create_inert(value, caller)),
            ty: PhantomData,
        }
    }
}

impl<T: 'static> ReactiveValue for Inert<T> {
    type Value = T;

    fn id(&self) -> Option<ValueId> {
        Some(self.id)
    }

    fn is_alive(&self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.id))
    }

    unsafe fn dispose(self) {
        unsafe { with_current_runtime(|rt| rt.dispose(self.id)) }
    }
}

impl<T: 'static> ReadSignal<T> for Inert<T> {
    #[track_caller]
    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        let caller = core::panic::Location::caller();
        with_current_runtime(|rt| self.id.with_untracked(rt, f, caller))
    }

    #[track_caller]
    fn track(&self) {
        // Note: Inert values are not tracked
    }
}

// TODO: Implement WriteSignal? To be used in MaybeSignal? Maybe then we need
// R/W marker as Signal does?

impl<T: 'static, U: 'static> SignalMap<T, U> for Inert<T> {
    type Output = Inert<U>;

    #[track_caller]
    fn map(&self, mut map: impl FnMut(&T) -> U) -> Self::Output {
        Inert::from(self.with_untracked(&mut map))
    }
}

impl<T: PartialEq + 'static> IntoMemo<T> for Inert<T> {
    #[track_caller]
    fn memo(self) -> crate::memo::Memo<T> {
        Memo::Inert(self)
    }
}

pub trait IntoInert<T> {
    fn inert(self) -> Inert<T>;
}

impl<T: 'static> IntoInert<T> for T {
    fn inert(self) -> Inert<T> {
        Inert::from(self)
    }
}

// #[derive(Debug, Clone, Copy)]
// pub struct CopyInert<T: Copy>(T);

// impl<T: Copy> ReactiveValue for CopyInert<T> {
//     type Value = T;

//     fn id(&self) -> Option<ValueId> {
//         None
//     }

//     fn is_alive(&self) -> bool {
//         true
//     }

//     unsafe fn dispose(self) {}
// }

// impl<T: Copy> ReadSignal<T> for CopyInert<T> {
//     #[track_caller]
//     fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
//         f(&self.0)
//     }

//     #[track_caller]
//     fn track(&self) {
//         // Note: Inert values are not tracked
//     }
// }

// impl<T: Copy, U> SignalMap<T, U> for CopyInert<T> {
//     type Output = CopyInert<U>;

//     #[track_caller]
//     fn map(&self, mut map: impl FnMut(&T) -> U) -> Self::Output {
//         CopyInert(map(&self.0))
//     }
// }

// pub trait IntoCopyInert<T: Copy> {
//     fn copy_inert(self) -> CopyInert<T>;
// }

// impl<T: Copy> IntoCopyInert<T> for T {
//     fn copy_inert(self) -> CopyInert<T> {
//         CopyInert(self)
//     }
// }

#[cfg(test)]
mod tests {}
