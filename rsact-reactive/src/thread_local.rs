#[cfg(any(
    all(feature = "single-thread", feature = "std",),
    not(any(feature = "single-thread", feature = "std"))
))]
compile_error!("Either `std` or `single-thread` feature is required!");

#[cfg(feature = "single-thread")]
pub mod fake_thread_local {
    use once_cell::sync::Lazy;

    pub struct FakeThreadLocal<T: 'static>(Lazy<T>);

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
