#![no_std]

pub mod color;
pub mod geometry;
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
    pub use crate::{
        color::{Color, RgbColor as _},
        geometry::{
            Rect, Size, block_model::BlockModel, border::Border,
            padding::Padding, *,
        },
        output::RenderTarget,
        path::*,
        primitives::{
            Primitive, arc::Arc, block::Block, circle::Circle,
            ellipse::Ellipse, line::Line, polygon::Polygon,
            rounded_rect::RoundedRect, sector::Sector,
        },
        renderer::{
            AntiAliasing, NullColor, NullRenderer, RenderResult, Renderer,
            Viewport, ViewportKind,
        },
        style::{ColorStyle, DrawStyle, StrokeAlignment, block::*},
    };

    #[cfg(feature = "embedded-graphics")]
    pub use crate::eg::{
        framebuf::{Framebuf, PackedColor, PackedFramebuf},
        primitives::*,
        renderer::EGRenderer,
    };
}
