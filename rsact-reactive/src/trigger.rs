use crate::{
    prelude::create_signal, read::ReadSignal as _, signal::Signal,
    write::WriteSignal as _,
};

#[track_caller]
pub fn create_trigger() -> Trigger {
    Trigger::new()
}

pub struct Trigger {
    inner: Signal<()>,
}

impl Trigger {
    #[track_caller]
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
