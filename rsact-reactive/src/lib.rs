// #![feature(thread_local)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

mod callback;
pub mod composables;
pub mod eco;
pub mod effect;
pub mod macros;
pub mod maybe_reactive;
pub mod memo;
pub mod memo_chain;
pub mod runtime;
pub mod scope;
pub mod signal;
mod storage;
mod thread_local;
pub mod trigger;
pub mod computed;

pub mod prelude {
    pub use super::{
        composables::*,
        effect::{use_effect, Effect},
        macros::*,
        memo::{AsMemo, Memo, MemoTree},
        memo_chain::{use_memo_chain, IntoMemoChain, MemoChain},
        runtime::{create_runtime, with_current_runtime, with_new_runtime},
        signal::{
            IntoSignal, MaybeSignal, ReadSignal, RwSignal, Signal,
            SignalMapper, SignalTree, WriteSignal,
        },
        trigger::{use_trigger, Trigger},
    };
}
