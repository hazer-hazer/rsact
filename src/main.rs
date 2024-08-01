#![no_std]

extern crate std;

use core::panic::Location;
use std::{
    boxed::Box,
    cell::{Ref, RefCell},
    collections::{btree_map::Entry, BTreeMap},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    println,
    ptr::NonNull,
    vec::Vec,
};

use futures::executor::block_on;
use futures_signals::signal::{Mutable, SignalExt};
use lazy_static::lazy_static;
use log::info;
use rsact::reactive::Reactive;
use spin::Mutex;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ValueLocation(core::panic::Location<'static>);

impl ValueLocation {
    #[track_caller]
    fn current() -> Self {
        Self(*core::panic::Location::caller())
    }
}

pub struct Stored(*const ());
unsafe impl Send for Stored {}

lazy_static! {
    pub static ref STORAGE: Mutex<BTreeMap<ValueLocation, Stored>> = Mutex::new(Default::default());
}

struct A<'a, T> {
    location: ValueLocation,
    data: PhantomData<&'a T>,
}

impl<'a, T: 'static> A<'a, T> {
    #[track_caller]
    fn new(value: T) -> Self {
        let mut this = Self {
            location: ValueLocation::current(),
            data: PhantomData,
        };
        this.set(value);
        this
    }
}

impl<'a, T> Clone for A<'a, T> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<'a, T> Copy for A<'a, T> {}

impl<'a, T: 'static> A<'a, T> {
    fn set(&mut self, value: T) {
        // FIXME: Drop previous leaking box
        let value: *const T = Box::into_raw(Box::new(value));

        println!("Write value to pointer {value:p}");

        let value = unsafe { core::mem::transmute(value) };

        STORAGE.lock().insert(self.location, Stored(value));
    }

    fn read(&self) -> &T {
        // let a = &*STRINGS.lock().as_ref().unwrap().as_ref();

        // unsafe { core::mem::transmute(a) }
        // todo!()

        // let a = Box::leak(Box::new(x))
        unsafe { core::mem::transmute(STORAGE.lock().get(&self.location).unwrap().0) }
    }
}

impl<'a, T: 'static> Deref for A<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.read()
    }
}

// impl<T: 'static> DerefMut for A<T> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         self.wr
//     }
// }

fn main() {
    let a = {
        let mut a = A::new("kek");

        a.set("asdccc");
        a.set("wrobn");
        a.set("alsk");
        a.set("asdccc");

        println!("{}", *a);
        a
    };

    println!("{}", *a);

    // let mutable = Mutable::new(5);

    // let future = mutable.signal().for_each(|value| {
    //     info!("Updated to {value}");
    //     async {}
    // });

    // let a = Reactive::new(123);

    // println!("{}", *a);
}
