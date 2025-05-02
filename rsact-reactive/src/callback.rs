use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

// pub enum CallbackResult {
//     None,
//     Changed,
// }

pub trait AnyCallback {
    fn run(&mut self, value: Rc<RefCell<dyn Any>>) -> bool;
}

/// Function used with single or no parameters in effects, memos and computed. It is only needed to allow to optionally accept single parameter of previous value.
pub trait CallbackFn<T, P>: 'static {
    fn run(&mut self, p: Option<&T>) -> T;
}

pub struct SingleParam;
pub struct NoParam;

impl<T, F> CallbackFn<T, SingleParam> for F
where
    F: FnMut(Option<&T>) -> T + 'static,
{
    #[inline(always)]
    fn run(&mut self, p: Option<&T>) -> T {
        self(p)
    }
}

impl<T, F> CallbackFn<T, NoParam> for F
where
    F: FnMut() -> T + 'static,
{
    #[inline(always)]
    fn run(&mut self, _: Option<&T>) -> T {
        self()
    }
}

fn foo() {
    fn f<T, P>(f: impl CallbackFn<T, P>) {}
}
