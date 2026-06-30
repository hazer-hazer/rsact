use super::{
    FontHandler, FontStyle, ResolvedFontProps, TextIntrinsics, TextOverflow,
    measure,
};
use crate::{el::ctx::*, render::prelude::*};
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
    fn measure_text(
        &self,
        content: &str,
        _props: ResolvedFontProps,
        overflow: TextOverflow,
    ) -> Option<TextIntrinsics> {
        match self {
            #[cfg(feature = "embedded-graphics")]
            Self::EGMonoFont(font) => Some(measure::mono_intrinsics(
                content,
                font.character_size.width,
                font.character_spacing,
                font.character_size.height,
                overflow,
            )),
            // TODO: How does initial point affects dimensions? Maybe we should
            // add position to size to compute real bounding box
            #[cfg(feature = "u8g2-fonts")]
            Self::U8G2(font) => {
                let measure_run = |run: &str| -> Size {
                    font.get_rendered_dimensions(
                        run,
                        embedded_graphics::prelude::Point::new(0, 0),
                        u8g2_fonts::types::VerticalPosition::Top,
                    )
                    .ok()
                    .and_then(|dims| dims.bounding_box)
                    .map(|bb| bb.size.into())
                    .unwrap_or(Size::zero())
                };

                let lines = measure::hard_line_count(content);
                let total_height = measure_run(content).height;
                let line_height =
                    total_height.checked_div(lines).unwrap_or(total_height);

                let max_content_width = content
                    .split('\n')
                    .map(|line| measure_run(line).width)
                    .max()
                    .unwrap_or(0);

                // u8g2's `render_aligned` does not soft-wrap, so min-content is
                // either the widest word (Wrap) or 0 (Clip/Ellipsis).
                let min_content_width = match overflow {
                    TextOverflow::Clip | TextOverflow::Ellipsis => 0,
                    TextOverflow::Wrap => content
                        .split_whitespace()
                        .map(|word| measure_run(word).width)
                        .max()
                        .unwrap_or(0),
                };

                Some(TextIntrinsics {
                    min_content_width,
                    max_content_width,
                    line_height,
                })
            },
            _ => unreachable!(),
        }
    }

    fn text_height_for_width(
        &self,
        content: &str,
        _props: ResolvedFontProps,
        width: u32,
        overflow: TextOverflow,
    ) -> u32 {
        match self {
            #[cfg(feature = "embedded-graphics")]
            Self::EGMonoFont(font) => measure::mono_height_for_width(
                content,
                font.character_size.width,
                font.character_spacing,
                font.character_size.height,
                width,
                overflow,
            ),
            // u8g2's renderer does not soft-wrap, so the height is one line per
            // hard '\n' regardless of width.
            // TODO: support soft wrapping for u8g2 fonts.
            #[cfg(feature = "u8g2-fonts")]
            Self::U8G2(_) => {
                let line_height = self
                    .measure_text(content, _props, overflow)
                    .map(|i| i.line_height)
                    .unwrap_or(0);
                measure::hard_line_count(content) * line_height
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
                // TextBox word-wraps into `bounds`, which now matches the
                // height computed by `text_height_for_width` for
                // `TextOverflow::Wrap`.
                // TODO: honor `Clip`/`Ellipsis` here (disable soft wrap + draw
                // an ellipsis) by threading the overflow mode into `draw`;
                // currently they render as wrap-into-a-short-box (overflow is
                // clipped, no ellipsis glyph).
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
            _ => unreachable!(),
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
    fn measure_text(
        &self,
        content: &str,
        props: ResolvedFontProps,
        overflow: TextOverflow,
    ) -> Option<TextIntrinsics> {
        self.with(props, |font| font.measure_text(content, props, overflow))
    }

    fn text_height_for_width(
        &self,
        content: &str,
        props: ResolvedFontProps,
        width: u32,
        overflow: TextOverflow,
    ) -> u32 {
        self.with(props, |font| {
            Some(font.text_height_for_width(content, props, width, overflow))
        })
        .unwrap_or(0)
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

#[cfg(all(test, feature = "embedded-graphics"))]
mod eg_tests {
    use super::*;
    use embedded_graphics::mono_font::ascii::FONT_6X10;

    fn props() -> ResolvedFontProps {
        ResolvedFontProps { size: 10, style: FontStyle::Normal }
    }

    // Pixel width of `n` chars derived from the font's own metrics, so the
    // assertions verify the field mapping rather than hard-coding 6x10.
    fn line_px(n: u32) -> u32 {
        n * FONT_6X10.character_size.width
            + n.saturating_sub(1) * FONT_6X10.character_spacing
    }

    #[test]
    fn egmono_measure_text_maps_word_line_and_height() {
        let font = FixedFont::EGMonoFont(&FONT_6X10);
        let m = font
            .measure_text("ab cde", props(), TextOverflow::Wrap)
            .unwrap();
        assert_eq!(
            m,
            TextIntrinsics {
                min_content_width: line_px(3), // widest word "cde"
                max_content_width: line_px(6), // unwrapped "ab cde"
                line_height: FONT_6X10.character_size.height,
            }
        );
    }

    #[test]
    fn egmono_clip_min_content_is_zero() {
        let font = FixedFont::EGMonoFont(&FONT_6X10);
        let m = font
            .measure_text("ab cde", props(), TextOverflow::Clip)
            .unwrap();
        assert_eq!(m.min_content_width, 0);
        assert_eq!(m.max_content_width, line_px(6));
    }

    #[test]
    fn egmono_height_for_width_wraps_and_clips() {
        let font = FixedFont::EGMonoFont(&FONT_6X10);
        let lh = FONT_6X10.character_size.height;
        // Width fits "hello" (5) but not "hello world" => 2 wrapped lines.
        let w = line_px(5) + 2;
        assert_eq!(
            font.text_height_for_width(
                "hello world",
                props(),
                w,
                TextOverflow::Wrap
            ),
            2 * lh
        );
        // Clip keeps one line per hard '\n' regardless of width.
        assert_eq!(
            font.text_height_for_width("a\nb", props(), 0, TextOverflow::Clip),
            2 * lh
        );
    }
}
