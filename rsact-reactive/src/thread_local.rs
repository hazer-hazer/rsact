#[cfg(any(
    all(feature = "single-thread", feature = "std",),
    not(any(feature = "single-thread", feature = "std"))
))]
compile_error!("Either `std` or `single-thread` feature is required!");

#[cfg(feature = "single-thread")]
pub mod fake_thread_local {
    use once_cell::sync::Lazy;

    /// A `thread_local!` stand-in for `no_std` single-threaded targets: one
    /// process-global cell, initialised lazily.
    ///
    /// It replaces the runtime's thread-local storage when the `single-thread`
    /// feature is on. The whole reactive runtime is reached through it, so it
    /// must be `Sync` to live in a `static` — hence the manual impl below.
    pub struct FakeThreadLocal<T: 'static>(Lazy<T>);

    // SAFETY: This `Sync` is a *promise the caller must uphold*, not a real one.
    // `T` (the reactive `Runtime`) is `!Sync` (it holds `Rc`/`RefCell`), so this
    // is only sound when the value is accessed from a single execution context.
    //
    // The `single-thread` feature is explicitly for bare-metal, single-core,
    // no-preemption targets where that holds. It is UNSOUND to enable on:
    //   - a multi-core MCU (e.g. RP2040, ESP32) where both cores could touch it,
    //   - any target where an interrupt/RTOS task reads or writes reactive state
    //     concurrently with the main context.
    // For those, access must be serialised by a real `critical_section` (a
    // `critical-section` impl is already required to link `single-thread`); that
    // is a known gap tracked in the audit (`thread-local-runtime-vs-embedded-
    // reality`), not something this impl provides.
    unsafe impl<T: 'static> Sync for FakeThreadLocal<T> {}

    impl<T: 'static> FakeThreadLocal<T> {
        pub const fn new(f: fn() -> T) -> Self {
            Self(Lazy::new(f))
        }
    }

    impl<T: 'static> FakeThreadLocal<T> {
        pub fn with<F, R>(&'static self, f: F) -> R
        where
            F: FnOnce(&T) -> R,
        {
            f(&self.0)
        }
    }

    macro_rules! thread_local_impl {
        () => {};

        ($(#[$attr:meta])* $vis:vis static $name:ident: $t:ty = $init:expr; $($rest:tt)*) => (
            $(#[$attr])*
            $vis static $name: $crate::thread_local::fake_thread_local::FakeThreadLocal<$t> = $crate::thread_local::fake_thread_local::FakeThreadLocal::new(|| $init);

            $crate::thread_local::fake_thread_local::thread_local_impl!($($rest)*);
        );
    }

    pub(crate) use thread_local_impl;
}

#[cfg(all(feature = "single-thread", not(feature = "std")))]
pub(crate) use fake_thread_local::thread_local_impl;
#[cfg(feature = "std")]
pub(crate) use std::thread_local as thread_local_impl;
