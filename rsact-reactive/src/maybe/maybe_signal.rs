use crate::{
    ReactiveValue,
    maybe::maybe_reactive::MaybeReactive,
    read::{ReadSignal, SignalMap, impl_read_signal_traits},
    signal::{IntoSignal, Signal, create_signal, marker},
    write::{SignalSetter, WriteSignal},
};

/// An optionally reactive, **read-write** value.
///
/// Unifies a static inline value and a reactive [`Signal`] under one type:
/// - `Inert(Option<T>)` — value stored inline in the enum. Reads are not
///   tracked; writes mutate the value but have no subscribers to notify.
///   The `Option` is `None` only transiently during lazy promotion to a
///   `Signal` inside `now_reactive` — externally it is always
///   `Some`.
/// - `Signal(Signal<T>)` — full reactive signal with tracked reads and
///   notified writes.
///
/// # Lazy promotion
///
/// The inert variant is promoted to a signal automatically via
/// `now_reactive`, which is called automatically by
/// [`SignalSetter::setter`] when a reactive source is bound. This means a
/// widget field can start as a plain value and only allocate a runtime node
/// when it is first bound to reactive data:
///
/// ```rust
/// # use rsact_reactive::prelude::*;
/// # use rsact_reactive::maybe::{MaybeSignal, IntoMaybeReactive};
/// # use rsact_reactive::write::{WriteSignal, SignalSetter};
/// let mut state: MaybeSignal<u32> = MaybeSignal::new_inert(0);
/// assert!(state.as_inert().is_some()); // currently plain data
///
/// let source = create_signal(42u32);
/// state.set_from(source.maybe_reactive()); // promotes to Signal
/// assert!(state.as_signal().is_some());
/// ```
///
/// # Mapping
///
/// [`SignalMap::map`] on `MaybeSignal` is **snapshot-only for inert**:
/// mapping an inert value evaluates the closure once and returns an inert
/// [`MaybeReactive`]; future writes to the `MaybeSignal` do not update the
/// result. Mapping a `Signal` variant produces a tracked [`Memo`].
/// Use [`SignalMapReactive::map_reactive`] when you always need a live
/// [`Memo<U>`] regardless of whether the source is inert or reactive.
///
/// For a **read-only** optionally reactive value see [`MaybeReactive`].
pub enum MaybeSignal<T: 'static, M: marker::Any = marker::Rw> {
    /// Option needed to deal with conversion from [`Inert`] into [`Signal`]
    /// TODO: Can be replaced with MaybeUninit for performance
    #[non_exhaustive]
    Inert(Option<T>),
    #[non_exhaustive]
    Signal(Signal<T, M>),
}

impl<T: Clone + 'static, M: marker::Any> Clone for MaybeSignal<T, M> {
    fn clone(&self) -> Self {
        match self {
            Self::Inert(arg0) => Self::Inert(arg0.clone()),
            Self::Signal(arg0) => Self::Signal(arg0.clone()),
        }
    }
}

impl<T: Copy + 'static, M: marker::Any> Copy for MaybeSignal<T, M> {}

impl<T: 'static, M: marker::Any> ReactiveValue for MaybeSignal<T, M> {
    type Value = T;

    fn id(&self) -> Option<crate::storage::ValueId> {
        match self {
            MaybeSignal::Inert(_) => None,
            MaybeSignal::Signal(signal) => signal.id(),
        }
    }

    fn is_alive(&self) -> bool {
        match self {
            MaybeSignal::Inert(_) => true,
            MaybeSignal::Signal(signal) => signal.is_alive(),
        }
    }

    unsafe fn dispose(self) {
        match self {
            MaybeSignal::Inert(_) => core::mem::drop(self),
            MaybeSignal::Signal(signal) => unsafe { signal.dispose() },
        }
    }
}

/// Conversion into [`MaybeSignal<T>`].
///
/// Implemented for:
/// - `T` → [`MaybeSignal::Inert`] (value stored inline, no runtime node).
/// - [`Signal<T>`] → [`MaybeSignal::Signal`] (existing reactive node).
///
/// Prefer accepting `impl Into<MaybeSignal<T>>` or
/// `impl IntoMaybeSignal<T>` in APIs that may receive either a static
/// value or a live signal:
///
/// ```rust
/// # use rsact_reactive::prelude::*;
/// # use rsact_reactive::maybe::{MaybeSignal, IntoMaybeSignal};
/// fn checkbox(value: impl Into<MaybeSignal<bool>>) {
///     let value: MaybeSignal<bool> = value.into();
///     // Static: checkbox(false)
///     // Reactive: checkbox(create_signal(false))
/// }
/// ```
pub trait IntoMaybeSignal<T> {
    fn maybe_signal(self) -> MaybeSignal<T>;
}

impl<T: 'static> IntoMaybeSignal<T> for Signal<T> {
    fn maybe_signal(self) -> MaybeSignal<T> {
        MaybeSignal::Signal(self)
    }
}

impl_read_signal_traits!(MaybeSignal<T>);

impl<T: 'static> IntoSignal<T> for MaybeSignal<T> {
    #[track_caller]
    fn signal(self) -> Signal<T> {
        match self {
            MaybeSignal::Inert(inert) => create_signal(inert.unwrap()),
            MaybeSignal::Signal(signal) => signal,
        }
    }
}

impl<T: 'static> MaybeSignal<T> {
    /// Creates new inert MaybeSignal
    pub fn new_inert(value: T) -> Self {
        Self::Inert(Some(value))
    }

    pub fn as_inert(&self) -> Option<&T> {
        match self {
            // Note: Option here is for lazy initialization, so unwrap is right
            MaybeSignal::Inert(inert) => Some(inert.as_ref().unwrap()),
            MaybeSignal::Signal(_) => None,
        }
    }

    pub fn as_inert_mut(&mut self) -> Option<&mut T> {
        match self {
            // Note: Option here is for lazy initialization, so unwrap is right
            MaybeSignal::Inert(inert) => Some(inert.as_mut().unwrap()),
            MaybeSignal::Signal(_) => None,
        }
    }

    pub fn as_signal(&self) -> Option<Signal<T>> {
        match self {
            MaybeSignal::Signal(signal) => Some(*signal),
            _ => None,
        }
    }

    #[track_caller]
    fn now_reactive(&mut self) -> Signal<T> {
        match self {
            MaybeSignal::Inert(inert) => {
                let signal = create_signal(inert.take().unwrap());
                *self = MaybeSignal::Signal(signal);
                signal
            },
            MaybeSignal::Signal(signal) => *signal,
        }
    }
}

impl<T: 'static> ReadSignal<T> for MaybeSignal<T> {
    #[track_caller]
    fn track(&self) {
        match self {
            MaybeSignal::Inert(_) => {},
            MaybeSignal::Signal(signal) => signal.track(),
        }
    }

    #[track_caller]
    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        match self {
            MaybeSignal::Inert(inert) => f(inert.as_ref().unwrap()),
            MaybeSignal::Signal(signal) => signal.with_untracked(f),
        }
    }
}

impl<T: 'static> WriteSignal<T> for MaybeSignal<T> {
    #[track_caller]
    fn notify(&self) {
        match self {
            MaybeSignal::Inert(_) => {},
            MaybeSignal::Signal(signal) => signal.notify(),
        }
    }

    #[track_caller]
    fn update_untracked<U>(&mut self, f: impl FnOnce(&mut T) -> U) -> U {
        match self {
            MaybeSignal::Inert(inert) => f(inert.as_mut().unwrap()),
            MaybeSignal::Signal(signal) => signal.update_untracked(f),
        }
    }
}

impl<T: 'static, U: PartialEq + 'static> SignalMap<T, U> for MaybeSignal<T> {
    type Output = MaybeReactive<U>;

    #[track_caller]
    fn map(&self, mut map: impl FnMut(&T) -> U + 'static) -> Self::Output {
        match self {
            MaybeSignal::Inert(inert) => {
                MaybeReactive::new_inert(map(inert.as_ref().unwrap()))
            },
            MaybeSignal::Signal(signal) => MaybeReactive::Memo(signal.map(map)),
        }
    }
}

// TODO: Implement `SignalSetter` for `MaybeSignal` for any source `impl IntoMaybeReactive<U>`
/// [`SignalSetter`] implementation for [`MaybeSignal`] that accepts any
/// [`MaybeReactive<U>`] as a source.
///
/// Behaviour depends on the source variant:
/// - **`MaybeReactive::Inert`** — applies `set_map` once as a one-shot
///   update. No ongoing binding is created; if the source value changes
///   later the target is not updated.
/// - **`MaybeReactive::Memo`** / **`MaybeReactive::MemoChain`** — promotes
///   `self` to a [`Signal`] via `now_reactive` and creates
///   a reactive effect that keeps the signal in sync with the source memo.
///   After this call `self` is always [`MaybeSignal::Signal`].
///
/// Use [`SignalSetter::set_from`] for the common case of
/// `T = U` with a simple clone mapping:
/// ```rust
/// # use rsact_reactive::prelude::*;
/// # use rsact_reactive::maybe::{MaybeSignal, IntoMaybeReactive};
/// # use rsact_reactive::write::{WriteSignal, SignalSetter};
/// let mut target: MaybeSignal<u32> = MaybeSignal::new_inert(0);
/// let source = create_signal(99u32);
/// target.set_from(source.maybe_reactive()); // target is now a Signal
/// ```
impl<T: 'static, U: PartialEq + 'static> SignalSetter<T, MaybeReactive<U>>
    for MaybeSignal<T>
{
    #[track_caller]
    fn setter(
        &mut self,
        source: MaybeReactive<U>,
        mut set_map: impl FnMut(&mut T, &<MaybeReactive<U> as ReactiveValue>::Value)
        + 'static,
    ) {
        match source {
            MaybeReactive::Inert(inert) => inert.with_untracked(|inert| {
                self.update(|this| set_map(this, &inert))
            }),
            MaybeReactive::Memo(memo) => {
                self.now_reactive().setter(memo, set_map)
            },
            MaybeReactive::MemoChain(memo_chain) => {
                self.now_reactive().setter(memo_chain, set_map)
            },
            // MaybeReactive::Derived(derived) => {
            //     // TODO: use_effect or not to use effect? See [`Signal: SignalSetter<T, MaybeReactive<U>>`] case for Derived
            //     let derived = Rc::clone(&derived);
            //     self.update(|this| set_map(this, &derived.borrow_mut()()))
            // },
        }
    }
}

// TODO: Other setters
// impl<T: PartialEq + 'static> SignalSetter<T, StaticSignal<T>>
//     for MaybeSignal<T>
// {
//     fn setter(
//         &mut self,
//         source: StaticSignal<T>,
//         set_map: impl Fn(&mut T, &<StaticSignal<T> as SignalValue>::Value) + 'static,
//     ) {
//         match self {
//             MaybeSignal::Static(raw) => {
//                 source.with(|source| set_map(&mut raw.borrow_mut(), source))
//             },
//             MaybeSignal::Signal(signal) => signal.setter(source, set_map),
//         }
//     }
// }

// impl<T: PartialEq + 'static> SignalSetter<T, Signal<T>> for MaybeSignal<T> {
//     fn setter(
//         &mut self,
//         source: Signal<T>,
//         set_map: impl Fn(&mut T, &<Signal<T> as SignalValue>::Value) + 'static,
//     ) {
//         match self {
//             MaybeSignal::Static(raw) => {
//                 source.with(|source| set_map(&mut raw.borrow_mut(), source))
//             },
//             MaybeSignal::Signal(signal) => signal.setter(source, set_map),
//         }
//     }
// }

// impl<T: PartialEq + 'static> SignalSetter<T, Memo<T>> for MaybeSignal<T> {
//     fn setter(
//         &mut self,
//         source: Memo<T>,
//         set_map: impl Fn(&mut T, &<Memo<T> as SignalValue>::Value) + 'static,
//     ) {
//         match self {
//             MaybeSignal::Static(raw) => {
//                 source.with(|source| set_map(&mut raw.borrow_mut(), source))
//             },
//             MaybeSignal::Signal(signal) => signal.setter(source, set_map),
//         }
//     }
// }

// impl<T: PartialEq + 'static> SignalSetter<T, MemoChain<T>> for MaybeSignal<T> {
//     fn setter(
//         &mut self,
//         source: MemoChain<T>,
//         set_map: impl Fn(&mut T, &<MemoChain<T> as SignalValue>::Value) + 'static,
//     ) {
//         match self {
//             MaybeSignal::Static(raw) => {
//                 source.with(|source| set_map(&mut raw.borrow_mut(), source))
//             },
//             MaybeSignal::Signal(signal) => signal.setter(source, set_map),
//         }
//     }
// }

impl<T: 'static> From<T> for MaybeSignal<T> {
    fn from(value: T) -> Self {
        Self::new_inert(value)
    }
}

impl<T: 'static> From<Signal<T>> for MaybeSignal<T> {
    fn from(value: Signal<T>) -> Self {
        Self::Signal(value)
    }
}
