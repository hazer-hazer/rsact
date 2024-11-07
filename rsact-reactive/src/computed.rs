/*!
 * Computed is a lens to a Signal.
 * It is needed to avoid making every part of a structure stored in signal to be reactive.
 * By setting computed, some signal is updated by passed parameters (or without any).
 * By getting computed, signal data is retrieved and possibly mapped.
 */

use alloc::boxed::Box;

use crate::{memo::Memo, signal::Signal};

// TODO: Static setter/getter?

pub enum SignalGetter<G: PartialEq> {
    Signal(Signal<G>),
    Memo(Memo<G>),
    Derived(Box<dyn Fn()>),
}

pub enum SignalSetter<S> {
    Signal(Signal<S>),
    // TODO: Use StoredValue
    Map(Box<dyn Fn(S)>),
}

impl<S: 'static> SignalSetter<S> {
    pub fn signal(signal: Signal<S>) -> Self {
        Self::Signal(signal)
    }

    pub fn map(f: impl Fn(S) + 'static) -> Self {
        Self::Map(Box::new(f))
    }
}

pub struct Computed<G: PartialEq, S> {
    getter: SignalGetter<G>,
    setter: SignalSetter<S>,
}
