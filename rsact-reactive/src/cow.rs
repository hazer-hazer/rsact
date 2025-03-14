use crate::{
    ReactiveValue,
    maybe::{IntoMaybeReactive, MaybeReactive},
    read::ReadSignal,
    signal::{Signal, create_signal},
    write::{SignalSetter, WriteSignal},
};

/// Clone On Write signal mooooo
/// Works similar to alloc::borrow::Cow, but Borrowed variant is a Memo (or readonly Signal) and Owned value is a signal.
#[derive(Clone, Debug, PartialEq)]
pub enum CowSignal<T: PartialEq + Clone + 'static> {
    Borrowed(MaybeReactive<T>),
    Owned(Signal<T>),
}

impl<T: PartialEq + Clone + 'static> CowSignal<T> {
    pub fn new_owned(signal: Signal<T>) -> Self {
        Self::Owned(signal)
    }

    pub fn new_borrowed(memo: impl IntoMaybeReactive<T>) -> Self {
        Self::Borrowed(memo.maybe_reactive())
    }
}

impl<T: PartialEq + Clone + 'static> CowSignal<T> {
    pub fn to_mut(&mut self) -> Signal<T> {
        match self {
            CowSignal::Borrowed(memo) => {
                let signal = create_signal(memo.get_cloned());
                *self = CowSignal::Owned(signal);
                signal
            },
            CowSignal::Owned(signal) => *signal,
        }
    }
}

impl<T: PartialEq + Clone + 'static> ReadSignal<T> for CowSignal<T> {
    fn track(&self) {
        match self {
            CowSignal::Borrowed(memo) => memo.track(),
            CowSignal::Owned(signal) => signal.track(),
        }
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        match self {
            CowSignal::Borrowed(memo) => memo.with_untracked(f),
            CowSignal::Owned(signal) => signal.with_untracked(f),
        }
    }
}

impl<T: PartialEq + Clone + 'static> WriteSignal<T> for CowSignal<T> {
    /// Notify subscribers about changes. No op for CowSignal storing Borrowed data!
    fn notify(&self) {
        match self {
            CowSignal::Borrowed(_) => {},
            CowSignal::Owned(signal) => signal.notify(),
        }
    }

    fn update_untracked<U>(&mut self, f: impl FnOnce(&mut T) -> U) -> U {
        self.to_mut().update_untracked(f)
    }
}

// impl<T: PartialEq + Clone + 'static> SignalMap<T> for CowSignal<T> {
//     type Output<U: PartialEq + 'static> = Memo<U>;

//     fn map<U: PartialEq + 'static>(
//         &self,
//         map: impl FnMut(&T) -> U + 'static,
//     ) -> Self::Output<U> {
//         match self {
//             CowSignal::Borrowed(mr) => mr.map(map),
//             CowSignal::Owned(signal) => signal.map(map),
//         }
//     }
// }

impl<T: PartialEq + Clone + 'static, S: ReactiveValue> SignalSetter<T, S>
    for CowSignal<T>
where
    Signal<T>: SignalSetter<T, S>,
{
    fn setter(
        &mut self,
        source: S,
        set_map: impl FnMut(&mut T, &<S as ReactiveValue>::Value) + 'static,
    ) {
        self.to_mut().setter(source, set_map);
    }
}
