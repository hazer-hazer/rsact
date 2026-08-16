//! Everything that needs tiny-skia.
//!
//! tiny-skia enters as an L2 [`TinySkiaRasterizer`](rasterizer::TinySkiaRasterizer)
//! over an L3 [`PixmapBlitter`](blitter::PixmapBlitter); the rest is conversion
//! — colors, geometry, and the `PathBuilder` extensions paths are built with.
//!
//! It has no scissor rect, so clipping is not expressible here: every draw call
//! takes an `Option<&Mask>` and a backend that forgets one silently overdraws.
//! The rasterizer therefore never draws — it fills a *coverage* mask and hands
//! spans to [`RasterCtx::blend`](crate::raster::RasterCtx::blend), which clips
//! before a blitter sees them.

pub mod blitter;
pub mod color;
pub mod geometry;
pub mod path;
pub mod rasterizer;
