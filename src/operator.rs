// use core::marker::PhantomData;

// use alloc::{collections::btree_map::BTreeMap, vec::Vec};

// use crate::{runtime::Observer, storage::ValueId};

// pub struct Operator<O> {
//     id: ValueId,
//     ops: BTreeMap<Observer, Vec<O>>,
// }

use core::{any::Any, cell::RefCell, marker::PhantomData};

use alloc::rc::Rc;

use crate::{runtime::with_current_runtime, storage::ValueId};

pub trait Operation: Any {}

pub trait AnyOperator {
    // TODO: Batch operate for efficiency?
    fn operate(&self, op: Rc<dyn Operation>, value: Rc<RefCell<dyn Any>>);
}

pub struct OperatorState<T, F, O>
where
    O: Operation,
    F: Fn(&O, &mut T),
{
    pub(crate) ty: PhantomData<T>,
    pub(crate) op: PhantomData<O>,
    pub(crate) f: F,
}

impl<T, F, O> AnyOperator for OperatorState<T, F, O>
where
    T: 'static,
    F: Fn(&O, &mut T),
    O: Operation,
{
    fn operate(&self, op: Rc<dyn Operation>, value: Rc<RefCell<dyn Any>>) {
        let mut value = RefCell::borrow_mut(&value);
        let value = value.downcast_mut::<T>().unwrap();
        let op = <dyn Any>::downcast_ref::<O>(&op).unwrap();

        (self.f)(op, value);
    }
}

pub struct Operator<T, O> {
    id: ValueId,
    ty: PhantomData<T>,
    op: PhantomData<O>,
}

impl<T, O> Operator<T, O>
where
    T: Default + 'static,
    O: Operation,
{
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&O, &mut T) + 'static,
    {
        Self {
            id: with_current_runtime(|rt| rt.storage.create_operator(f)),
            ty: PhantomData,
            op: PhantomData,
        }
    }
}

pub fn use_operator<T, F, O>(f: F) -> Operator<T, O>
where
    T: Default + 'static,
    O: Operation,
    F: Fn(&O, &mut T) + 'static,
{
    Operator::new(f)
}
