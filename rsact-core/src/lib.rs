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
pub mod trigger;
pub mod vec;
pub mod memo;

pub mod prelude {
    pub use super::composables::*;
    pub use super::effect::{use_effect, Effect};
    pub use super::runtime::{
        create_runtime, with_current_runtime, with_scoped_runtime,
    };
    pub use super::signal::{
        EcoSignal, IntoSignal, ReadSignal, RwSignal, Signal, SignalTree,
        WriteSignal,
    };
    pub use super::trigger::{use_trigger, Trigger};
}
