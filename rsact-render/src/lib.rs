#![no_std]

// The float-math backend. `FloatExt` supplies `sqrt`, `atan2` and friends on
// no_std; on std the inherent `f32` methods shadow it and a backend is not
// needed, which is why the second check excludes std.
#[cfg(all(feature = "libm", feature = "micromath"))]
compile_error!(
    "rsact-render: features `libm` and `micromath` are mutually exclusive — enable exactly one math backend"
);
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
// Empty on std-without-a-backend so `use crate::FloatExt as _;` still resolves.
#[cfg(all(feature = "std", not(feature = "libm"), not(feature = "micromath")))]
pub trait FloatExt {}
#[cfg(all(feature = "std", not(feature = "libm"), not(feature = "micromath")))]
impl FloatExt for f32 {}
#[cfg(all(feature = "std", not(feature = "libm"), not(feature = "micromath")))]
impl FloatExt for f64 {}

// The three drawing layers: `renderer` (L1) drives `raster` (L2), which emits
// into `blitter` (L3). Each is unconditional; the backend-specific rasterizers
// and blitters live under `eg` and `tiny_skia`.
pub mod blitter;
pub mod color;
pub mod framebuf;
pub mod geometry;
pub mod image;
pub mod output;
pub mod path;
pub mod primitives;
pub mod raster;
pub mod record;
pub mod region;
pub mod renderer;
pub mod scan;
pub mod style;
// Test harness, not API. A feature rather than `#[cfg(test)]` because its users
// are in other crates and `cfg(test)` does not cross a crate boundary.
#[cfg(feature = "test-utils")]
#[doc(hidden)]
pub mod test_support;

#[macro_use]
extern crate alloc;

// `#![no_std]` drops `std` from the extern prelude; the golden harness needs it.
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
