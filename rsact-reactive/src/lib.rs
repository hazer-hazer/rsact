// #![feature(thread_local)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate rsact_macros;

extern crate alloc;

mod callback;
pub mod computed;
pub mod eco;
pub mod effect;
pub mod maybe;
pub mod memo;
pub mod memo_chain;
pub mod read;
pub mod runtime;
pub mod scope;
pub mod signal;
mod storage;
mod thread_local;
pub mod trigger;
pub mod write;

pub mod prelude {
    pub use super::{
        effect::{create_effect, Effect},
        maybe::{
            Inert, IntoInert, IntoMaybeSignal, MaybeReactive, MaybeSignal,
            SignalMapReactive,
        },
        memo::{create_memo, IntoMemo, Memo, MemoTree},
        memo_chain::{create_memo_chain, IntoMemoChain, MemoChain},
        read::{map, with, ReadSignal, SignalMap},
        rsact_macros::IntoMaybeReactive,
        runtime::{create_runtime, with_current_runtime, with_new_runtime},
        signal::{create_signal, IntoSignal, RwSignal, Signal},
        trigger::{create_trigger, Trigger},
        write::{SignalSetter, UpdateNotification, WriteSignal},
    };
}

/// SignalValue is used as HKT abstraction over reactive (or not) types such as Signal<T> (Value = T), Memo<T>, MaybeReactive<T>, etc.
pub trait ReactiveValue: 'static {
    type Value;

    fn is_alive(&self) -> bool;
    fn dispose(self);
    // TODO: try_dispose?
}
