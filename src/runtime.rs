use core::{
    cell::{Ref, RefCell, RefMut},
    marker::PhantomData,
};

use alloc::{collections::btree_map::BTreeMap, rc::Rc, sync::Arc};
use lazy_static::lazy_static;
use spin::Mutex;

use crate::reactive::Storage;

lazy_static! {
    pub static ref RUNTIME: Mutex<Runtime> = Mutex::new(Runtime::new());
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct ScopeId(usize);

#[derive(Debug)]
struct Scope {
    parent: Option<ScopeId>,
}

pub struct Runtime {
    pub(crate) scopes: BTreeMap<ScopeId, Scope>,
    pub(crate) storage: Storage,
}

impl Runtime {
    fn new() -> Self {
        Self {
            scopes: Default::default(),
            storage: Default::default(),
        }
    }

    pub(crate) fn with<T>(f: impl FnOnce(&Runtime) -> T) -> T {
        f(&RUNTIME.lock())
    }

    pub fn create_signal<T>(&self, value: T) {
        
    }

    // pub(crate) fn storage_mut<'a>(&'a mut self) -> RefMut<'a, Storage> {
    //     self.storage.borrow_mut()
    // }

    // pub(crate) fn storage(&self) -> Ref<'_, Storage> {
    //     self.storage.borrow()
    // }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }
}
