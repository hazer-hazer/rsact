//! Reactive-storage backend for the global runtime.
//!
//! The reactive [`Runtime`](crate::runtime::Runtime) is `!Send`/`!Sync` (it
//! holds `Rc`/`RefCell` — that is what makes the engine cheap), so it can't
//! normally live in a `static`. Exactly one backend must be selected:
//!
//! - **`std`** — real [`std::thread_local!`]; one runtime per OS thread. No
//!   `unsafe`. Used for host builds/tests.
//! - **`single-thread`** — no_std, **sound**: every access is wrapped in
//!   `critical_section::with`, which serialises access (interrupts off on a
//!   single core, a hardware spinlock on multicore, an RTOS primitive under an
//!   RTOS). Requires a `critical-section` impl from the target.
//! - **`unsafe-single-thread`** — no_std, **fast + unsafe**: a bare global with
//!   no critical section. Sound *only* if the runtime is touched from exactly
//!   one execution context (no interrupt handlers, one core). Pulls no
//!   `critical-section` dependency; the promise is entirely on the caller (same
//!   trade-off as Slint's `unsafe-single-threaded`).

#[cfg(not(any(
    feature = "std",
    feature = "single-thread",
    feature = "unsafe-single-thread"
)))]
compile_error!(
    "rsact-reactive needs one reactive-storage backend: enable `std`, \
     `single-thread` (no_std, critical-section-guarded), or \
     `unsafe-single-thread` (no_std, single-execution-context only)."
);
#[cfg(all(
    feature = "std",
    any(feature = "single-thread", feature = "unsafe-single-thread")
))]
compile_error!(
    "`std` is mutually exclusive with the no_std backends `single-thread` / \
     `unsafe-single-thread`."
);
#[cfg(all(feature = "single-thread", feature = "unsafe-single-thread"))]
compile_error!("Enable only one of `single-thread` or `unsafe-single-thread`.");

#[cfg(any(feature = "single-thread", feature = "unsafe-single-thread"))]
pub mod fake_thread_local {
    use core::cell::OnceCell;

    /// A `thread_local!` stand-in for `no_std`: one lazily-initialised
    /// process-global cell. See the [module docs](crate::thread_local) for the
    /// backend semantics.
    pub struct FakeThreadLocal<T: 'static> {
        cell: OnceCell<T>,
        init: fn() -> T,
    }

    // SAFETY:
    // - `single-thread`: `with` is the only accessor and always runs inside
    //   `critical_section::with`, which grants exclusive, memory-barriered
    //   access across cores/interrupts. The `!Send` value is therefore never
    //   accessed concurrently — exactly what `Sync` requires. (`critical-section`
    //   itself only requires `T: Send` for its `Mutex`; we relax that because we
    //   never hand out a reference outside the critical section, so the value is
    //   effectively pinned to one context.)
    // - `unsafe-single-thread`: an unchecked promise (documented on the feature)
    //   that the caller only ever touches the runtime from a single execution
    //   context. Using it from an ISR or a second core is undefined behaviour.
    unsafe impl<T: 'static> Sync for FakeThreadLocal<T> {}

    impl<T: 'static> FakeThreadLocal<T> {
        pub const fn new(init: fn() -> T) -> Self {
            Self { cell: OnceCell::new(), init }
        }

        #[inline]
        pub fn with<R>(&'static self, f: impl FnOnce(&T) -> R) -> R {
            #[cfg(feature = "single-thread")]
            {
                // `critical_section::with` is reentrant: nested `with` calls (a
                // signal read inside an effect inside a flush, or the lazy-init
                // block below touching another cell) are fine — only the
                // outermost actually toggles interrupts / takes the lock.
                critical_section::with(|_| f(self.cell.get_or_init(self.init)))
            }
            #[cfg(all(
                feature = "unsafe-single-thread",
                not(feature = "single-thread")
            ))]
            {
                f(self.cell.get_or_init(self.init))
            }
        }
    }

    macro_rules! thread_local_impl {
        () => {};

        ($(#[$attr:meta])* $vis:vis static $name:ident: $t:ty = $init:expr; $($rest:tt)*) => (
            $(#[$attr])*
            $vis static $name: $crate::thread_local::fake_thread_local::FakeThreadLocal<$t> =
                $crate::thread_local::fake_thread_local::FakeThreadLocal::new(|| $init);

            $crate::thread_local::fake_thread_local::thread_local_impl!($($rest)*);
        );
    }

    pub(crate) use thread_local_impl;
}

#[cfg(any(feature = "single-thread", feature = "unsafe-single-thread"))]
pub(crate) use fake_thread_local::thread_local_impl;
#[cfg(feature = "std")]
pub(crate) use std::thread_local as thread_local_impl;
