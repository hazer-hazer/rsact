//! Harness code — **not API**, and behind the `test-utils` feature so a
//! production build graph never contains it.
//!
//! # Why a feature and not `#[cfg(test)]`
//!
//! `cfg(test)` is set only for the crate *currently being test-compiled*. When
//! rsact-ui's tests build, they link rsact-render compiled normally — so a
//! `#[cfg(test)] mod` here would be invisible to every consumer that actually
//! uses it, and every consumer of these two modules is in another crate
//! (`rsact-ui/tests/tile_schedule.rs`, `rsact-ui/src/test_support`). A feature
//! is the only gate that crosses the crate boundary.
//!
//! Turned on for this crate's own dev targets by a self dev-dependency, the same
//! shape rsact-reactive uses for its `test-utils`; resolver 3 keeps
//! dev-dependency features out of normal builds.
//!
//! # What is here, and what deliberately is not
//!
//! [`schedule`] is WS6.4a's tile measurement and tile-invariance arithmetic;
//! [`golden`] is WS6.9's bless workflow. Both exist to *check* the renderer and
//! neither is reachable from a drawing path.
//!
//! `record` is **not** here, and that is a judgement rather than an oversight:
//! `RecordingRenderer` is a `Renderer` like any other, `PrimitiveKind` is kept
//! as a value vocabulary for recording, replay and `Canvas`'s command list, and
//! a devtool that inspects draw calls is a plausible product feature. Gating it
//! would assert it never will be. `schedule` depends on it, which is fine —
//! gated may depend on ungated, never the reverse.

pub mod schedule;

// File I/O, so it needs `std` on top of the feature.
#[cfg(feature = "std")]
pub mod golden;
