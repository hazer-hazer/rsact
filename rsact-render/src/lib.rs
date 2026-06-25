#![no_std]

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
