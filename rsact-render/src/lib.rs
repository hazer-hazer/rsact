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

pub mod color;
pub mod geometry;
pub mod image;
pub mod layer;
pub mod output;
pub mod path;
pub mod primitives;
pub mod renderer;
pub mod style;

#[macro_use]
extern crate alloc;

#[cfg(feature = "embedded-graphics")]
pub mod eg;

#[cfg(feature = "tiny-skia")]
pub mod tiny_skia;

pub mod prelude {
    #[cfg(feature = "embedded-graphics")]
    pub use crate::eg::{
        framebuf::{Framebuf, PackedColor, PackedFramebuf},
        primitives::*,
        renderer::EGRenderer,
    };
    #[cfg(feature = "tiny-skia")]
    pub use crate::tiny_skia::TinySkiaRenderer;
    pub use crate::{
        color::{BigEndian, ByteOrder, Color, LittleEndian, RgbColor as _},
        geometry::{Rect, Size, block_model::BlockModel, padding::Padding, *},
        output::{ColorMapper, FinishRender, MapColor, RenderTarget},
        path::*,
        primitives::{
            Primitive, PrimitiveKind, arc::Arc, block::Block, circle::Circle,
            ellipse::Ellipse, line::Line, polygon::Polygon,
            rounded_rect::RoundedRect, sector::Sector,
        },
        renderer::{
            AntiAliasing, NullColor, NullRenderer, RenderResult, Renderer,
            Viewport, ViewportKind,
        },
        style::{ColorStyle, DrawStyle, StrokeAlignment, block::*},
    };
}
