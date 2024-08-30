use core::{marker::PhantomData, ops::Deref, panic::Location};

use alloc::{collections::btree_map::BTreeMap, vec::Vec};

use crate::{
    runtime::{with_current_runtime, Observer},
    signal::{
        marker::{self, CanRead, CanWrite},
        ReadSignal, Signal, WriteSignal,
    },
    storage::ValueId,
};

pub enum VecOperation<T> {
    InsertAt { index: usize, value: T },
    UpdateAt { index: usize, value: T },
    RemoveAt { index: usize },
    Move { from: usize, to: usize },
    Push(T),
    Pop,
    Clear,
}

// TODO: `Operator` generalized struct

struct SignalVecState<T, M: marker::Any> {
    values: Vec<T>,
    ops: BTreeMap<Observer, Vec<VecOperation<T>>>,
    rw: PhantomData<M>,
}

#[derive(Clone, Copy)]
pub struct SignalVec<T, M: marker::Any = marker::Rw> {
    id: ValueId,
    ty: PhantomData<T>,
    rw: PhantomData<M>,
}

impl<T: 'static, M: marker::Any + 'static> SignalVec<T, M> {
    #[track_caller]
    pub fn with_values(values: Vec<T>) -> Self {
        let caller = Location::caller();
        Self {
            id: with_current_runtime(|rt| {
                rt.storage.create_signal(
                    SignalVecState {
                        values,
                        ops: Default::default(),
                        rw: PhantomData::<M>,
                    },
                    caller,
                )
            }),
            ty: PhantomData,
            rw: PhantomData,
        }
    }

    pub fn new() -> Self {
        Self::with_values(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_values(Vec::with_capacity(capacity))
    }
}

impl<T: 'static, M: marker::CanRead> ReadSignal<Vec<T>> for SignalVec<T, M> {
    fn track(&self) {
        with_current_runtime(|rt| self.id.subscribe(rt))
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&Vec<T>) -> U) -> U {
        with_current_runtime(|rt| self.id.with_untracked(rt, f))
    }
}

impl<T: 'static, M: marker::CanWrite> WriteSignal<Vec<T>> for SignalVec<T, M> {
    #[track_caller]
    fn notify(&self) {
        let caller = Location::caller();
        with_current_runtime(|rt| self.id.notify(rt, caller)).ok().unwrap();
    }

    fn update_untracked<U>(&self, f: impl FnOnce(&mut Vec<T>) -> U) -> U {
        with_current_runtime(|rt| self.id.update_untracked(rt, f))
    }
}

impl<T: 'static, M: marker::CanRead> SignalVec<T, M> {
    pub fn capacity(&self) -> usize {
        self.with(|this| this.capacity())
    }

    pub fn len(&self) -> usize {
        self.with(|this| this.len())
    }

    // pub fn as_slice(&self) -> &[T] {
    //     self.with(|this| this.as_slice())
    // }
}

impl<T: 'static, M: marker::CanWrite> SignalVec<T, M> {
    // fn operation<U>(
    //     &self,
    //     f: impl FnOnce(&mut Vec<T>) -> (U, VecOperation<T>),
    // ) -> U {
    //     let (result, op) = self.update(|this| f(this));
    //     with_current_runtime(|rt| self.id.)
    //     result
    // }

    pub fn pop(&self) -> Option<T> {
        self.update(|this| this.pop())
    }
}

// TODO
pub trait SignalVecExt<T> {
    fn signal_vec(self) -> SignalVec<T>;
}
