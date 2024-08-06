use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

pub trait AnyCallback {
    fn run(&self, value: Rc<RefCell<dyn Any>>);
}
