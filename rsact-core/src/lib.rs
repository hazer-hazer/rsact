#![feature(thread_local)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod callback;
pub mod composables;
pub mod effect;
pub mod operator;
pub mod runtime;
pub mod signal;
mod storage;
pub mod vec;

pub mod prelude {
    pub use super::composables::*;
    pub use super::effect::use_effect;
    pub use super::runtime::{
        create_runtime, with_current_runtime, with_scoped_runtime,
    };
    pub use super::signal::{
        EcoSignal as _, ReadSignal as _, RwSignal as _, Signal,
        WriteSignal as _,
    };
}
