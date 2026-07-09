use crate::{
    ReactiveValue,
    memo::{IntoMemo, Memo, create_memo},
    read::{ReadSignal, SignalMap, impl_read_signal_traits},
    storage::ValueId,
};

/// A static, non-reactive value stored **inline** (WS4.1).
///
/// Previously `Inert` was a `Copy` handle into a `ValueKind::Stored` runtime
/// node, so every `.inert()` / builder literal minted (and, with no scope
/// active during view construction, permanently leaked) a node. It now holds
/// its value directly — zero runtime presence — mirroring `MaybeSignal::Inert`.
/// Reads are never tracked and there is no node to dispose.
///
/// `Inert<T>` is `Copy` iff `T: Copy` and `Clone` iff `T: Clone` (G1: blanket
/// `Copy` is dropped now that the value, not an id, lives in the wrapper).
pub struct Inert<T>(T);

impl<T: Clone> Clone for Inert<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: Copy> Copy for Inert<T> {}

impl_read_signal_traits!(Inert<T>);

impl<T: 'static> From<T> for Inert<T> {
    fn from(value: T) -> Self {
        Inert(value)
    }
}

impl<T: 'static> ReactiveValue for Inert<T> {
    type Value = T;

    fn id(&self) -> Option<ValueId> {
        // Inline value — no runtime node. Mirrors `MaybeSignal::Inert::id`.
        None
    }

    fn is_alive(&self) -> bool {
        true
    }

    unsafe fn dispose(self) {
        // Nothing to dispose — the value drops with `self`.
    }
}

impl<T: 'static> ReadSignal<T> for Inert<T> {
    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        f(&self.0)
    }

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
        // No node created: maps an inline value to a new inline value.
        Inert::from(self.with_untracked(&mut map))
    }
}

impl<T: PartialEq + Clone + 'static> IntoMemo<T> for Inert<T> {
    /// Materialize a constant as a `Memo`. Unlike the old `Memo::Inert` variant
    /// (removed in WS4.1 so `Memo<T>` stays unconditionally `Copy`), this mints
    /// a real constant memo node — so it allocates. This is a cold path:
    /// builder literals stay `MaybeReactive::Inert` (inline, node-free) and
    /// never reach here.
    #[track_caller]
    fn memo(self) -> Memo<T> {
        let Inert(value) = self;
        create_memo(move || value.clone())
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
