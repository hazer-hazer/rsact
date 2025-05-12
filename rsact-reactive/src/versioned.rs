use core::ops::{Deref, DerefMut};

/// Simply-versioned value. Made to be used in memos where comparison is expensive (e.g. Vec). [`Versioned`] implements [`Deref`] so it is a transparent wrapper while to update the value the [`Versioned::update`] must be used.
#[derive(Debug, Clone, Copy)]
pub struct Versioned<T> {
    version: usize,
    value: T,
}

impl<T> Deref for Versioned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> Versioned<T> {
    pub fn new(value: T) -> Self {
        Self { version: 0, value }
    }

    pub fn update(&mut self, mut f: impl FnMut(&mut T)) {
        self.version += 1;
        f(&mut self.value);
    }
}

impl<T> PartialEq for Versioned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
    }
}
