//! Everything that needs embedded-graphics.
//!
//! The module tree is cut by **feature gate**, not by architectural layer: this
//! directory exists because these files cannot compile without the crate, and
//! `blitter.rs` / `raster.rs` / `scan.rs` are single unconditional files in
//! `src/` because they can. Enabling a backend adds a directory rather than
//! scattering `#[cfg]` through shared ones.

pub mod color;
pub mod framebuf;
pub mod image;
pub mod interop;
pub mod rasterizer;
