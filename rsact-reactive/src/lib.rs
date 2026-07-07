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
// Shared allocation-tracking global allocator for the metrics bench/tool
// (WS0.7j). std-only, doc-hidden — a measurement utility, not public API.
#[cfg(feature = "std")]
#[doc(hidden)]
pub mod alloc_probe;
#[cfg(feature = "async")]
pub mod async_rt;
pub mod effect;
pub mod inert;
pub mod maybe;
pub mod memo;
pub mod probe;
pub mod read;
#[cfg(feature = "async")]
pub mod resource;
pub mod runtime;
pub mod scope;
pub mod signal;
pub mod storage;
mod thread_local;
pub mod trigger;
// pub mod versioned;
mod macros;
pub mod stored;
pub mod write;

#[cfg(feature = "debug-info")]
pub mod debug;

pub mod prelude {
    #[cfg(feature = "async")]
    pub use super::async_rt::AsyncState;
    #[cfg(feature = "async")]
    pub use super::resource::{Resource, create_resource};
    pub use super::{
        ReactiveValue,
        computed::{Computed, create_computed},
        // cow::CowSignal,
        effect::{Effect, create_effect},
        inert::{Inert, IntoInert},
        maybe::{
            IsInert, IsReactive, ReactivityMarker,
            maybe_reactive::IntoMaybeReactive, maybe_reactive::MaybeReactive,
            maybe_signal::IntoMaybeSignal, maybe_signal::MaybeSignal,
        },
        memo::{IntoMemo, Memo, MemoTree, create_memo},
        probe::{Probe, create_probe},
        read::{
            ReadSignal, SignalMap, SignalMapRef, SignalMapRefMaybeReactive,
            SignalMapSlice, SignalWithRef, SignalWithSlice, map, with,
        },
        // TODO: Is this right to reexport from other crate?
        rsact_macros::IntoMaybeReactive,
        runtime::{
            batch, defer_effects, observe, observe_by_location,
            observe_with_force, untrack, with_current_runtime,
        },
        signal::{
            IntoSignal, RwSignal, Signal, create_signal, marker::ReadOnly,
            marker::Rw, marker::WriteOnly,
        },
        trigger::{Trigger, create_trigger},
        write::{SignalSetter, UpdateNotification, WriteSignal},
    };

    // Dev-only multi-runtime helpers, gated behind `test-utils` so they never
    // exist in a production build graph (WS1.2). `create_runtime` alone is not
    // re-exported here — `with_new_runtime` is the scoped, restore-safe entry
    // point.
    #[cfg(any(test, feature = "test-utils"))]
    pub use super::runtime::{create_runtime, with_new_runtime};
}

/// Core trait implemented by every reactive (and inert) value in the runtime.
///
/// [`ReactiveValue`] provides a uniform interface for identity, liveness
/// queries, and disposal. The associated type `Value` is the plain Rust type
/// stored inside the reactive node (e.g. `T` for `Signal<T>`).
///
/// All high-level types — [`signal::Signal`], [`memo::Memo`],
/// [`effect::Effect`], [`trigger::Trigger`],
/// [`maybe::Inert`], [`maybe::MaybeReactive`], [`maybe::MaybeSignal`] —
/// implement this trait.
///
/// # Safety
///
/// [`ReactiveValue::dispose`] is `unsafe` because calling it while the value
/// is still referenced by a live effect or memo causes use-after-free in the
/// dependency graph. Prefer letting the owning [`scope::ScopeHandle`] manage
/// lifetimes automatically.
pub trait ReactiveValue {
    type Value;

    fn id(&self) -> Option<ValueId>;
    fn is_alive(&self) -> bool;
    unsafe fn dispose(self);

    fn name(self, _name: &'static str) -> Self
    where
        Self: Sized,
    {
        #[cfg(feature = "debug-info")]
        if let Some(id) = self.id() {
            id.set_name(_name);
        }
        self
    }
    // TODO: try_dispose?
}
