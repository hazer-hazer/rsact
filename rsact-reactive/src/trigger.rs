use crate::{
    prelude::create_signal, read::ReadSignal, signal::Signal,
    write::WriteSignal,
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

impl ReadSignal<()> for Trigger {
    fn track(&self) {
        self.track();
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&()) -> U) -> U {
        f(&())
    }
}

impl WriteSignal<()> for Trigger {
    fn notify(&self) {
        self.notify();
    }

    fn update_untracked<U>(&mut self, f: impl FnOnce(&mut ()) -> U) -> U {
        f(&mut ())
    }
}
