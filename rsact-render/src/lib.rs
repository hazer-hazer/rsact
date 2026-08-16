#![no_std]

// no_std f32 math backend. Exactly one of `libm` (default) / `micromath` must
// be enabled — same mutually-exclusive contract as rsact-reactive's storage
// backends. `FloatExt` is the float-method trait the geometry and primitive
// code brings into scope with `use crate::FloatExt as _;`. On `std` builds the
// inherent `f32` methods shadow the trait, so the simulator uses std math with
// zero cfg; the trait only supplies the methods on no_std targets.
#[cfg(all(feature = "libm", feature = "micromath"))]
compile_error!(
    "rsact-render: features `libm` and `micromath` are mutually exclusive — enable exactly one math backend"
);
// A backend is required only on no_std: with `std`, the inherent `f32` methods
// shadow `FloatExt` and the trait is never called, so a std build needs no math
// backend feature (a bare `--features std` builds).
#[cfg(all(
    not(feature = "std"),
    not(any(feature = "libm", feature = "micromath"))
))]
compile_error!(
    "rsact-render: a float-math backend is required on no_std — enable `libm` (default) or `micromath` (std uses inherent f32 math)"
);

#[cfg(all(feature = "micromath", not(feature = "libm")))]
pub use micromath::F32Ext as FloatExt;
#[cfg(all(feature = "libm", not(feature = "micromath")))]
pub use num_traits::Float as FloatExt;
// std with no explicit backend: `FloatExt` must still exist so the unconditional
// `use crate::FloatExt as _;` imports resolve; it's an empty marker because the
// inherent `f32` methods do the work.
#[cfg(all(feature = "std", not(feature = "libm"), not(feature = "micromath")))]
pub trait FloatExt {}
#[cfg(all(feature = "std", not(feature = "libm"), not(feature = "micromath")))]
impl FloatExt for f32 {}
#[cfg(all(feature = "std", not(feature = "libm"), not(feature = "micromath")))]
impl FloatExt for f64 {}

// The render layer split's L3: `Blitter` (spans -> pixels), `Span`, and the
// addressing helpers. Unconditional — an L3 blitter must be definable without
// embedded-graphics, which is what makes a direct-to-panel or DMA2D blitter
// expressible; only the pixmap blitter is gated.
pub mod blitter;
pub mod color;
// WS6.4e: packed pixel storage — `Framebuf`, `FramebufStorage`, `PackedColor`.
// Unconditional, and that is the point of the item: nothing in it is specific
// to embedded-graphics, and the render layer split's L3 blitter must be able to
// use it without that dependency. What genuinely needs the crate stayed in
// `eg/framebuf.rs`.
pub mod framebuf;
pub mod geometry;
// `golden` is a std-only test-support module (file I/O for the WS6.9 golden
// harness). It is reusable across crates — hence a real `#[cfg(feature="std")]`
// module, not `#[cfg(test)]` (downstream crates' tests can't see test code).
#[cfg(feature = "std")]
pub mod golden;
pub mod image;
pub mod output;
pub mod path;
pub mod primitives;
// The render layer split's L2: `Rasterizer` (geometry -> spans) and `RasterCtx`
// (the clip gate). Unconditional; the concrete rasterizers live in their
// backend's module (`eg::rasterizer`, `tiny_skia::rasterizer`), so the tree is
// cut by feature gate rather than by layer.
pub mod raster;
pub mod record;
// WS6.4d(1): damage rects -> the regions a frame is painted in. Pure geometry,
// no renderer and no steady-state allocation, so it belongs beside the geometry
// it operates on rather than in the UI crate that drives it.
pub mod region;
pub mod renderer;
// The shared scan conversion every `Rasterizer` default delegates to. A sibling
// of `raster` rather than a child: that file is the contract, this one is 600
// lines of algorithm, and the two are read for different reasons.
pub mod scan;
// WS6.4a's measurement + tile-invariance arithmetic over `record`'s op logs.
// Unconditional for the same reason `record` is: pure `alloc` math with no file
// I/O (unlike `golden`), so a no_std integration test can use it too.
pub mod schedule;
pub mod style;

#[macro_use]
extern crate alloc;

// `#![no_std]` drops `std` from the extern prelude; the golden harness's file
// I/O needs it, so bring it back on std builds only.
#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "embedded-graphics")]
pub mod eg;

#[cfg(feature = "tiny-skia")]
pub mod tiny_skia;

pub mod prelude {
    #[cfg(feature = "embedded-graphics")]
    pub use crate::eg::{interop::DrawTargetProxy, rasterizer::EgRasterizer};
    #[cfg(feature = "tiny-skia")]
    pub use crate::tiny_skia::{
        blitter::PixmapBlitter, rasterizer::TinySkiaRasterizer,
    };
    pub use crate::{
        blitter::{Blitter, FramebufBlitter, Span},
        color::{BigEndian, ByteOrder, Color, LittleEndian, RgbColor as _},
        framebuf::{Framebuf, FramebufStorage, PackedColor},
        geometry::{Rect, Size, block_model::BlockModel, padding::Padding, *},
        output::MapColor,
        path::*,
        primitives::{
            Primitive, PrimitiveKind, arc::Arc, block::Block, circle::Circle,
            ellipse::Ellipse, line::Line, polygon::Polygon,
            rounded_rect::RoundedRect, sector::Sector,
        },
        raster::{RasterCtx, Rasterizer},
        region::{
            FramePolicy, RegionLimits, Tiles, Unbounded, assert_policy_fits,
            plan_regions, plan_regions_into, policy_units,
        },
        renderer::{
            Attached, Attachment, Detached, NullColor, NullRenderer,
            RasterRenderer, RenderResult, Renderer, region_units,
        },
        style::{ColorStyle, DrawStyle, StrokeAlignment, block::*},
    };
}
