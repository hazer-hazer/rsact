use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

// pub enum CallbackResult {
//     None,
//     Changed,
// }

pub trait AnyCallback {
    fn run(&mut self, value: Rc<RefCell<dyn Any>>) -> bool;
}
