// #![feature(thread_local)]
#![cfg_attr(all(not(feature = "std"), test), no_std)]

extern crate alloc;
extern crate rsact_macros;

#[cfg(any(feature = "std", test))]
extern crate std;

mod callback;
pub mod computed;
// pub mod cow;
// pub mod eco;
pub mod effect;
pub mod maybe;
pub mod memo;
pub mod memo_chain;
pub mod read;
pub mod resource;
pub mod runtime;
pub mod scope;
pub mod signal;
pub mod storage;
mod thread_local;
pub mod trigger;
pub mod versioned;
pub mod write;

pub mod prelude {
    pub use super::{
        computed::{Computed, create_computed},
        // cow::CowSignal,
        effect::{Effect, create_effect},
        maybe::{
            Inert, IntoInert, IntoMaybeSignal, MaybeReactive, MaybeSignal,
            SignalMapReactive,
        },
        memo::{IntoMemo, Memo, MemoTree, create_memo},
        memo_chain::{IntoMemoChain, MemoChain, create_memo_chain},
        read::{ReadSignal, SignalMap, map, with},
        resource::create_resource,
        rsact_macros::IntoMaybeReactive,
        runtime::{create_runtime, with_current_runtime, with_new_runtime},
        signal::{IntoSignal, RwSignal, Signal, create_signal},
        trigger::{Trigger, create_trigger},
        write::{SignalSetter, UpdateNotification, WriteSignal},
    };
}

/// SignalValue is used as HKT abstraction over reactive (or not) types such as Signal<T> (Value = T), Memo<T>, MaybeReactive<T>, etc.
pub trait ReactiveValue: 'static {
    type Value;

    fn is_alive(&self) -> bool;
    unsafe fn dispose(self);
    // TODO: try_dispose?
}
