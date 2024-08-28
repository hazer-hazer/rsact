use crate::{
    prelude::use_signal,
    signal::{ReadSignal, Signal, WriteSignal},
};

pub struct Trigger {
    inner: Signal<()>,
}

impl Trigger {
    pub fn new() -> Self {
        Self { inner: use_signal(()) }
    }

    pub fn track(&self) {
        self.inner.track();
    }

    pub fn notify(&self) {
        self.inner.notify();
    }
}

pub fn use_trigger() -> Trigger {
    Trigger::new()
}
