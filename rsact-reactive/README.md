# rsact-reactive

> Important caveat: I needed thread locals to avoid wrapping many things in Mutex'es. But `thread_local!` macro is std-only as `LocalKey` has platform-specific implementation, and `#[thread_local]` attribute is unstable (but I used it throughout) initial development and requires nightly. To make this library usable on stable, I implemented unsafe-dependent faker for thread local data which IS UNSOUND. Please, be careful, and if you have suggestions how to fix this, be pleasant to help me with that 🙏
> Use `cargo test -- --test-threads=1`, otherwise tests will fail due to borrowing errors from `RefCell`s.
>
> Waiting for `#[thread_local]` to stabilize.

To build `rsact-reactive` you need to either specify `single-thread` or `std` feature.
