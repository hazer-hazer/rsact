//! What rsact needs from tiny-skia, once it stopped being a renderer.
//!
//! **PR D deleted `TinySkiaRenderer`.** tiny-skia is an L2 `Rasterizer` now —
//! [`TinySkiaRasterizer`](rasterizer::TinySkiaRasterizer) — over an L3
//! [`PixmapBlitter`](blitter::PixmapBlitter). Both live here, in the module
//! gated on the crate they need: the tree is cut by **feature gate**, not by
//! architectural layer, so `blitter.rs` and `raster.rs` in `src/` stay
//! unconditional and free of `#[cfg]`.
//!
//! The rest is conversion: colors, geometry, and the `PathBuilder` extensions
//! the rasterizer builds its paths with.
//!
//! # Why the clip mask went with it
//!
//! `TinySkiaRenderer` cached a `Mask` and passed it to all four draw entry
//! points, because tiny-skia has no scissor rect and every call takes an
//! `Option<&Mask>` — WS6.11 existed because all four had `None` hard-coded and
//! `Scrollable` overflowed its bounds on this backend.
//!
//! There are **no tiny-skia draw entry points left**. `TinySkiaRasterizer` fills
//! a *coverage* `Mask` and hands spans to `RasterCtx::blend`, which has already
//! clipped them before any blitter sees one. So `clip_mask`, `rebuild_clip_mask`
//! and WS6.11's five clip tests are deleted rather than ported: clipping stops
//! being per-backend behaviour a backend can forget and becomes impossible to
//! escape by construction, and it is tested once, where it is now enforced —
//! `raster::tests::nothing_escapes_the_clip_however_hard_a_rasterizer_sprays`.
//!
//! **One trap those tests knew, re-homed here before they went:** a fresh
//! tiny-skia canvas is **opaque white**, not transparent. A test asserting
//! `alpha != 0` therefore passes on every pixel of an untouched surface and
//! proves nothing. Paint black and look for pixels that are not white. (This
//! trap has now appeared twice in this project — the other time was a golden
//! test drawing `WHITE` on a `WHITE` background.)

pub mod blitter;
pub mod color;
pub mod geometry;
pub mod path;
pub mod rasterizer;
