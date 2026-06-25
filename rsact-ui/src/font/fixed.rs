use super::{FontHandler, FontStyle, ResolvedFontProps};
use crate::{el::ctx::*, layout::Limits, render::prelude::*};
use alloc::collections::btree_map::BTreeMap;
use core::fmt::Display;

/// Fixed-size fonts
#[derive(Clone, Copy, Debug)]
pub enum FixedFont {
    /// embedded_graphics mono font of fixed size.
    #[cfg(feature = "embedded-graphics")]
    EGMonoFont(&'static embedded_graphics::mono_font::MonoFont<'static>),
    /// u8g2 font of fixed size.
    #[cfg(feature = "u8g2-fonts")]
    U8G2(&'static u8g2_fonts::FontRenderer),
}

impl FontHandler for FixedFont {
    fn measure_text_size(
        &self,
        content: &str,
        _props: ResolvedFontProps,
    ) -> Option<Limits> {
        match self {
            #[cfg(feature = "embedded-graphics")]
            Self::EGMonoFont(font) => {
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

                Some(Limits::new(max_size, max_size))
            },
            // TODO: How does initial point affects dimensions? Maybe we should
            // add position to size to compute real bounding box
            #[cfg(feature = "u8g2-fonts")]
            Self::U8G2(font) => {
                let bounds = font
                    .get_rendered_dimensions(
                        content,
                        embedded_graphics::prelude::Point::new(0, 0),
                        u8g2_fonts::types::VerticalPosition::Top,
                        // u8g2_fonts::types::HorizontalAlignment::Left,
                    )
                    .unwrap()
                    .bounding_box
                    .unwrap_or(embedded_graphics::primitives::Rectangle::zero());

                // let max_size = Size::new(
                //     (bounds.size.width as i32 - bounds.top_left.x) as u32,
                //     (bounds.size.height as i32 - bounds.top_left.y) as u32,
                // );
                let max_size = bounds.size.into();

                Some(Limits::new(max_size, max_size))
            },
            _ => unreachable!(),
        }
    }

    fn draw<W: WidgetCtx>(
        &self,
        content: &str,
        _props: ResolvedFontProps,
        bounds: Rect,
        color: W::Color,
        renderer: &mut W::Renderer,
    ) -> Option<RenderResult> {
        match self {
            #[cfg(feature = "embedded-graphics")]
            FixedFont::EGMonoFont(mono_font) => {
                use embedded_graphics::Drawable as _;
                let eg_bounds: embedded_graphics::primitives::Rectangle =
                    bounds.into();
                Some(
                embedded_text::TextBox::new(
                    &content,
                    eg_bounds,
                    embedded_graphics::mono_font::MonoTextStyleBuilder::new()
                        .font(mono_font)
                        .text_color(color.map_through_rgba::<embedded_graphics::pixelcolor::Rgb888>())
                        .build(),
                )
                .draw(&mut rsact_render::eg::renderer::DrawTargetProxy::new(renderer))
                .map(|_| ())
                .map_err(|_| ()))
            },
            #[cfg(feature = "u8g2-fonts")]
            FixedFont::U8G2(u8g2_font) => {
                let _ = u8g2_font.render_aligned(
                    content,
                    embedded_graphics::prelude::Point::new(
                        bounds.top_left.x,
                        bounds.top_left.y,
                    ),
                    u8g2_fonts::types::VerticalPosition::Top,
                    u8g2_fonts::types::HorizontalAlignment::Left,
                    u8g2_fonts::types::FontColor::Transparent(color.map_through_rgba::<embedded_graphics::pixelcolor::Rgb888>()),
                    &mut rsact_render::eg::renderer::DrawTargetProxy::new(renderer),
                );
                Some(Ok(()))
            },
        }
    }
}

impl Display for FixedFont {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            #[cfg(feature = "embedded-graphics")]
            FixedFont::EGMonoFont(_) => write!(f, "EG"),
            #[cfg(feature = "u8g2-fonts")]
            FixedFont::U8G2(_) => write!(f, "u8g2"),
            _ => unreachable!(),
        }
    }
}

impl PartialEq for FixedFont {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            #[cfg(feature = "embedded-graphics")]
            (FixedFont::EGMonoFont(lhs), FixedFont::EGMonoFont(rhs)) => {
                embedded_graphics::mono_font::MonoFont::eq(lhs, rhs)
            },
            // TODO: Is pointer comparison right?
            #[cfg(feature = "u8g2-fonts")]
            (&FixedFont::U8G2(lhs), &FixedFont::U8G2(rhs)) => core::ptr::eq(
                lhs as *const u8g2_fonts::FontRenderer,
                rhs as *const u8g2_fonts::FontRenderer,
            ),
            _ => false,
        }
    }
}

/// Fixed-size fonts collection mapped by size and style.
/// It is used for dynamically sized pre-rendered fonts such as
/// embedded_graphics MonoFont and U8G2 which aren't vector graphics font and
/// not rendered at runtime, so we only have pre-generated sizes sets. Font size
/// here is absolute font height.
pub struct FixedFontCollection {
    sizes_styles: BTreeMap<u32, BTreeMap<FontStyle, FixedFont>>,
}

impl FontHandler for FixedFontCollection {
    fn measure_text_size(
        &self,
        content: &str,
        props: ResolvedFontProps,
    ) -> Option<Limits> {
        self.with(props, |font| font.measure_text_size(content, props))
    }

    fn draw<W: WidgetCtx>(
        &self,
        content: &str,
        props: ResolvedFontProps,
        bounds: Rect,
        color: W::Color,
        renderer: &mut W::Renderer,
    ) -> Option<RenderResult> {
        self.with(props, |font| {
            font.draw::<W>(content, props, bounds, color, renderer)
        })
    }
}

impl FixedFontCollection {
    pub fn new() -> Self {
        Self { sizes_styles: Default::default() }
    }

    pub fn with_font(
        mut self,
        size: u32,
        style: FontStyle,
        data: impl Into<FixedFont>,
    ) -> Self {
        let redefined = self
            .sizes_styles
            .entry(size)
            .or_default()
            .insert(style, data.into())
            .is_some();

        // TODO: Maybe just warn instead of panic?
        debug_assert!(!redefined);

        self
    }

    pub fn with<U>(
        &self,
        props: ResolvedFontProps,
        with_font: impl FnMut(&FixedFont) -> Option<U>,
    ) -> Option<U> {
        self.sizes_styles
            .get(&props.size)
            .map(|styles| styles.get(&props.style).map(with_font).flatten())
            .flatten()
    }
}
