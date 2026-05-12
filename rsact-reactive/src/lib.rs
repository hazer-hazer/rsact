// #![feature(thread_local)]
#![cfg_attr(any(not(feature = "std"), test), no_std)]

use storage::ValueId;

extern crate alloc;
extern crate rsact_macros;

#[cfg(any(feature = "std", test))]
extern crate std;

mod callback;
pub mod computed;
// pub mod cow;
// pub mod eco;
#[cfg(feature = "async")]
pub mod async_rt;
pub mod effect;
pub mod maybe;
pub mod memo;
pub mod memo_chain;
pub mod observer;
pub mod read;
#[cfg(feature = "async")]
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
            Inert, IntoInert, IntoMaybeReactive, IntoMaybeSignal,
            MaybeReactive, MaybeSignal, SignalMapReactive,
        },
        memo::{IntoMemo, Memo, MemoTree, create_memo},
        memo_chain::{IntoMemoChain, MemoChain, create_memo_chain},
        read::{ReadSignal, SignalMap, SignalWithSlice, map, with},
        rsact_macros::IntoMaybeReactive,
        runtime::{
            batch, create_runtime, defer_effects, observe, observe_by_location,
            untrack, with_current_runtime, with_new_runtime,
        },
        signal::{IntoSignal, RwSignal, Signal, create_signal},
        trigger::{Trigger, create_trigger},
        write::{SignalSetter, UpdateNotification, WriteSignal},
    };

    #[cfg(feature = "async")]
    pub use super::async_rt::AsyncState;

    #[cfg(feature = "async")]
    pub use super::resource::{Resource, create_resource};
}

/// Core trait implemented by every reactive (and inert) value in the runtime.
///
/// [`ReactiveValue`] provides a uniform interface for identity, liveness
/// queries, and disposal. The associated type `Value` is the plain Rust type
/// stored inside the reactive node (e.g. `T` for `Signal<T>`).
///
/// All high-level types — [`signal::Signal`], [`memo::Memo`], [`memo_chain::MemoChain`], [`effect::Effect`],
/// [`trigger::Trigger`], [`maybe::Inert`], [`maybe::MaybeReactive`], [`maybe::MaybeSignal`] — implement
/// this trait.
///
/// # Safety
///
/// [`ReactiveValue::dispose`] is `unsafe` because calling it while the value
/// is still referenced by a live effect or memo causes use-after-free in the
/// dependency graph. Prefer letting the owning [`scope::ScopeHandle`] manage
/// lifetimes automatically.
pub trait ReactiveValue: 'static {
    type Value;

    fn id(&self) -> Option<ValueId>;
    fn is_alive(&self) -> bool;
    unsafe fn dispose(self);

    fn name(self, name: &'static str) -> Self
    where
        Self: Sized,
    {
        #[cfg(feature = "debug-info")]
        if let Some(id) = self.id() {
            id.set_name(name);
        }
        self
    }
    // TODO: try_dispose?
}
