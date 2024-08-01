#![no_std]
#![feature(thread_local)]

extern crate alloc;

// pub struct Observer<T> {
//     inner: T,
// }

// impl<T> Observer<T> {
//     pub fn new(value: T) -> Self {
//         Self { inner: value }
//     }
// }

// // impl<T> core::ops::Deref for Observer<T> {
// //     type Target = T;

// //     fn deref(&self) -> &Self::Target {
// //         &self.inner
// //     }
// // }

// // impl<T> core::ops::DerefMut for Observer<T> {
// //     fn deref_mut(&mut self) -> &mut Self::Target {
// //         &mut self.inner
// //     }
// // }

// impl<T> Clone for Observer<T> {
//     fn clone(&self) -> Self {
//         Self {
//             inner: self.inner.clone(),
//         }
//     }
// }

// struct DieSlowly {}

// pub fn observe<T>(value: T) -> Observer<T> {
//     Observer::new(value)
// }
pub mod reactive;
pub mod runtime;
pub mod signal;
