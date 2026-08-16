//! Harness code — **not API**, and behind the `test-utils` feature so a
//! production build graph never contains it.
//!
//! A feature and not `#[cfg(test)]`, because `cfg(test)` holds only for the
//! crate being test-compiled: rsact-ui's tests link this crate built normally,
//! and every consumer of these modules is in another crate. This crate's own dev
//! targets get the feature from a self dev-dependency.
//!
//! `record` is deliberately **not** gated. `RecordingRenderer` is a `Renderer`
//! like any other and a devtool that inspects draw calls is a plausible product
//! feature, so gating it would assert it never will be. [`schedule`] depending
//! on it is the sound direction — gated may depend on ungated, never the
//! reverse.

pub mod schedule;

// File I/O, so it needs `std` on top of the feature.
#[cfg(feature = "std")]
pub mod golden;
