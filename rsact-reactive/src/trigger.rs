use crate::{
    prelude::create_signal,
    signal::{ReadSignal, Signal, WriteSignal},
};

pub struct Trigger {
    inner: Signal<()>,
}

impl Trigger {
    pub fn new() -> Self {
        Self { inner: create_signal(()) }
    }

    pub fn is_alive(self) -> bool {
        self.inner.is_alive()
    }

    #[track_caller]
    pub fn track(&self) {
        self.inner.track();
    }

    #[track_caller]
    pub fn notify(&self) {
        self.inner.notify();
    }
}

pub fn use_trigger() -> Trigger {
    Trigger::new()
}
