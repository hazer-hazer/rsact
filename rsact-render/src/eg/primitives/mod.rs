//! embedded-graphics' own primitive algorithms, one delegation per shape.
//!
//! **PR A deleted the two traits that used to live here**, `EgPrimitive` and
//! `EgPrimitiveRenderer`, along with every `draw_aa` half. They existed to hand
//! a primitive two things a plain draw target cannot give it — `pixel_alpha`
//! (the anti-aliased paths blended against the destination one pixel at a time)
//! and `draw_pixels` — plus an `AntiAliasing` type witness that kept the AA and
//! non-AA call graphs from resolving into each other.
//!
//! With the AA halves gone, what remains of each shape is a ~10-line call to
//! embedded-graphics' `StyledDrawable`, and that needs only a `DrawTarget`,
//! which `EGRenderer` already is. A trait whose sole job is to attach one
//! function to one type is that function with an extra name and an extra
//! import, so each is a free `draw` in its own module. The exception is
//! [`polygon::draw`], which needs a full `Renderer` and no embedded-graphics at
//! all.
//!
//! Anti-aliasing is **deleted, not migrated** (maintainer decision D1):
//! `EgRasterizer` is embedded-graphics as-is, and rsact's own anti-aliased
//! rasterizer — blending, coverage, spans rather than per-pixel `f32` — is
//! `RsactRasterizer`, written against the layer split rather than retrofitted
//! onto it.

pub mod arc;
pub mod circle;
pub mod ellipse;
pub mod line;
pub mod polygon;
pub mod rounded_rect;
pub mod sector;
