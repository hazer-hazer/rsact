//! Everything that needs tiny-skia: a
//! [`TinySkiaRasterizer`](rasterizer::TinySkiaRasterizer) for anti-aliased
//! output, a [`PixmapBlitter`](blitter::PixmapBlitter) to draw into a `Pixmap`,
//! and the color, geometry and `PathBuilder` conversions they use.

pub mod blitter;
pub mod color;
pub mod geometry;
pub mod path;
pub mod rasterizer;
