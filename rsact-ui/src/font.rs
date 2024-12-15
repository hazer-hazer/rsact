use alloc::string::String;
use embedded_graphics::{
    mono_font::MonoTextStyleBuilder, prelude::Point, primitives::Rectangle,
};
use rsact_reactive::{
    maybe::{IntoMaybeReactive, MaybeReactive},
    memo::{Keyed, Memo, NeverEqual},
    prelude::IntoMaybeReactive,
    read::{ReadSignal, SignalMap},
    signal::Signal,
    with,
};

use crate::{
    layout::{size::Size, Limits},
    render::Renderable as _,
    widget::{DrawResult, WidgetCtx},
};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextHorizontalAlign {
    #[default]
    Left,
    Center,
    Right,
    // TODO: Justified
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextVerticalAlign {
    Top,
    Middle,
    #[default]
    Baseline,
    Bottom,
}

/// User-specified font size
#[derive(Clone, Copy, Debug, PartialEq, IntoMaybeReactive)]
pub enum FontSize {
    Unset,
    /// Fixed font-size in pixels.
    Fixed(u32),
    /// Relative to viewport value where 1.0 is given by default Unset variant
    Relative(f32),
}

impl From<u32> for FontSize {
    fn from(value: u32) -> Self {
        Self::Fixed(value)
    }
}

impl From<f32> for FontSize {
    fn from(value: f32) -> Self {
        Self::Relative(value)
    }
}

impl FontSize {
    pub fn resolve(&self, viewport: Size) -> u32 {
        let base = match viewport.width.max(viewport.height) {
            ..64 => 6,
            ..96 => 8,
            ..128 => 9,
            ..192 => 10,
            ..256 => 12,
            ..296 => 13,
            ..400 => 15,
            400.. => 16,
        };

        match self {
            FontSize::Unset => base,
            &FontSize::Fixed(fixed) => fixed,
            &FontSize::Relative(rel) => (base as f32 * rel) as u32,
        }
    }
}

#[derive(Clone, Copy, PartialEq, IntoMaybeReactive)]
pub enum FontStyle {
    Normal,
    Italic,
    Bold,
}

pub const GLOBAL_FALLBACK_FONT: Font =
    Font::EGMonoFont(&embedded_graphics::mono_font::ascii::FONT_6X10);

// TODO: Font fallback
#[derive(Clone, Debug)]
pub enum Font {
    // TODO: Common fonts similar to egui: small, button, heading, etc.
    Auto,
    /// Font inherited from parent element or global
    /// Not allowed to be created by user
    #[non_exhaustive]
    Inherited(Memo<Font>),
    /// Static inert embedded_graphics mono font of fixed size.
    EGMonoFont(&'static embedded_graphics::mono_font::MonoFont<'static>),
    /// Static inert u8g2 font of fixed size.
    U8G2(u8g2_fonts::FontRenderer),
}

impl PartialEq for Font {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Inherited(lhs), Self::Inherited(rhs)) => {
                with!(move |lhs, rhs| lhs == rhs)
            },
            (Self::EGMonoFont(lhs), Self::EGMonoFont(rhs)) => {
                // TODO: Check this
                core::ptr::eq(lhs, rhs)
            },
            (Self::U8G2(lhs), Self::U8G2(rhs)) => core::ptr::eq(lhs, rhs),
            _ => false,
        }
    }
}

impl Font {
    pub fn is_auto(&self) -> bool {
        matches!(self, Font::Auto)
    }
}

pub enum StoredFont {}

/// Dynamic fonts library. Allows having dynamically-sized font with generic styles such as monospace, bold/italic, etc.
pub struct FontLib {
    // TODO
}

pub struct FontCtx {
    lib: FontLib,
}

impl FontCtx {
    pub fn new() -> Self {
        Self { lib: FontLib {} }
    }

    pub fn measure_text_size(&self, font: &Font, content: &str) -> Limits {
        match font {
            Font::Auto => todo!(),
            Font::Inherited(font) => {
                font.with(|font| self.measure_text_size(font, content))
            },
            Font::EGMonoFont(font) => {
                let char_size = font.character_size;

                let max_size = content.split(|char| char == '\n').fold(
                    Size::zero(),
                    |size, a| {
                        if a == "\r" {
                            size
                        } else {
                            let chars_count = a.chars().count() as u32;
                            let line_len = chars_count * char_size.width
                                + chars_count.saturating_sub(1)
                                    * font.character_spacing;
                            Size::new(
                                size.width.max(line_len),
                                size.height + char_size.height,
                            )
                        }
                    },
                );

                Limits::new(max_size, max_size)
            },
            // TODO: How does initial point affects dimensions? Maybe we should add position to size to compute real bounding box
            Font::U8G2(font) => {
                let max_size = font
                    .get_rendered_dimensions_aligned(
                        content,
                        Point::zero(),
                        u8g2_fonts::types::VerticalPosition::Baseline,
                        u8g2_fonts::types::HorizontalAlignment::Left,
                    )
                    .unwrap()
                    .unwrap()
                    .size;

                Limits::new(max_size.into(), max_size.into())
            },
        }
    }

    pub fn draw<W: WidgetCtx>(
        &self,
        font: &Font,
        content: &str,
        bounds: Rectangle,
        color: W::Color,
        renderer: &mut W::Renderer,
    ) -> DrawResult {
        match font {
            Font::Auto => todo!(),
            Font::Inherited(font) => font.with(|font| {
                self.draw::<W>(font, content, bounds, color, renderer)
            }),
            Font::EGMonoFont(mono_font) => embedded_text::TextBox::new(
                &content,
                bounds,
                MonoTextStyleBuilder::new()
                    .font(mono_font)
                    .text_color(color)
                    .build(),
            )
            .render(renderer),
            Font::U8G2(u8g2_font) => {
                let _ = u8g2_font.render_aligned(
                    content,
                    bounds.top_left,
                    u8g2_fonts::types::VerticalPosition::Baseline,
                    u8g2_fonts::types::HorizontalAlignment::Left,
                    u8g2_fonts::types::FontColor::Transparent(color),
                    renderer,
                );
                Ok(())
            },
        }
    }
}
