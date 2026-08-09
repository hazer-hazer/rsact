use crate::render::prelude::*;
use alloc::vec::Vec;
use core::fmt::{Debug, Display};
use fixed::{FixedFont, FixedFontCollection};
// portable-atomic gives `AtomicUsize::fetch_add` on no-CAS targets (thumbv6m),
// where `core::sync::atomic::AtomicUsize` has no CAS ops. Native instructions
// on thumbv7m+. See the thumbv6m note in the README for the fallback the end
// product must select.
use portable_atomic::{AtomicUsize, Ordering};
use rsact_reactive::prelude::*;

pub mod fixed;
pub mod measure;

/// How a text leaf behaves when its assigned width is smaller than its
/// unwrapped (max-content) width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub enum TextOverflow {
    /// Soft-wrap at word boundaries into the available width; height grows.
    #[default]
    Wrap,
    /// Keep one visual line per hard `'\n'`; clip horizontally at draw.
    Clip,
    /// Like [`TextOverflow::Clip`] but the last visible run is truncated with
    /// an ellipsis at draw.
    Ellipsis,
}

/// Intrinsic sizing of a text leaf along the inline (width) axis, plus the
/// single-line height. Height for a concrete width is derived separately via
/// [`FontHandler::text_height_for_width`] because it depends on wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextIntrinsics {
    /// Width of the widest unbreakable unit (longest word) for [`TextOverflow::Wrap`];
    /// `0` for [`TextOverflow::Clip`]/[`TextOverflow::Ellipsis`] (squeezable).
    pub min_content_width: u32,
    /// Width of the longest hard-`'\n'` line, with no soft wrapping.
    pub max_content_width: u32,
    /// Height of a single line.
    pub line_height: u32,
}

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

// WS21.1: the cascading font properties that used to live here are
// `env::LayoutEnv` — the box group of the environment. They moved because they
// are one instance of a general mechanism (see `crate::env`), not a font-module
// concern; what stays here is the font *machinery* they resolve against.

/// User-specified font size
#[derive(Clone, Copy, Debug, PartialEq, IntoMaybeReactive)]
pub enum FontSize {
    /// Fixed font-size in pixels.
    Fixed(u32),
    /// Relative to viewport value where 1.0 is given by default Unset variant
    Relative(f32),
}

impl Default for FontSize {
    fn default() -> Self {
        Self::Relative(1.0)
    }
}

impl Display for FontSize {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FontSize::Fixed(fixed) => write!(f, "{fixed}"),
            FontSize::Relative(relative) => write!(f, "{relative:.2}"),
        }
    }
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
            &FontSize::Fixed(fixed) => fixed,
            &FontSize::Relative(rel) => (base as f32 * rel) as u32,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    IntoMaybeReactive,
    Default,
)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    // TODO: Should be separate font weight
    Bold,
    // Oblique,
}

/// Resolved font properties
#[derive(Debug, Clone, Copy)]
pub struct ResolvedFontProps {
    /// Absolute font size, i.e. font height in pixels
    pub size: u32,
    pub style: FontStyle,
}

pub enum FontFamily {
    Monospace,
    Proportional,
    // TODO: Custom?
}

/// Font setting found in text widget. It is an identifier pointing to the
/// actual font or a fixed-size font set for a specific text widget (e.g.
/// embedded_graphics MonoFont or u8g2 font)
#[derive(Clone, Copy, Debug, PartialEq, IntoMaybeReactive)]
pub enum Font {
    // TODO: Common fonts similar to egui: small, button, heading, etc.
    Auto,
    /// Font from library
    Id(FontId),
    /// Fixed-size font
    Fixed(FixedFont),
}

impl Display for Font {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Font::Auto => write!(f, "auto"),
            Font::Id(font_id) => write!(f, "font#{font_id}"),
            Font::Fixed(fixed_font) => write!(f, "{fixed_font}"),
        }
    }
}

impl Font {
    pub fn is_auto(&self) -> bool {
        matches!(self, Font::Auto)
    }
}

/// The logic implemented by actual fonts such as StoredFont, FixedFont.
pub trait FontHandler {
    /// Intrinsic width range (min-content/max-content) and single-line height
    /// of `content`. `overflow` only affects `min_content_width` (it is `0`
    /// for clip/ellipsis, which can be squeezed). Returns `None` if the font
    /// cannot measure the text.
    fn measure_text(
        &self,
        content: &str,
        props: ResolvedFontProps,
        overflow: TextOverflow,
    ) -> Option<TextIntrinsics>;

    /// Total height `content` occupies when laid out into `width` pixels under
    /// `overflow` (wrapping grows the height; clip/ellipsis keep one line per
    /// hard `'\n'`).
    fn text_height_for_width(
        &self,
        content: &str,
        props: ResolvedFontProps,
        width: u32,
        overflow: TextOverflow,
    ) -> u32;

    /// Draw `content` into `bounds`.
    ///
    /// WS6.4c(A): generic over the **renderer**, not over `WidgetCtx`. `W` was
    /// only ever used for `W::Color` and `W::Renderer`, so this is strictly
    /// simpler — and it is what lets text go through rsact-ui's drawing seam
    /// (`RenderCtx` implements `Renderer`) rather than around it. Without it the
    /// render mode would have a hole exactly where the most expensive drawing
    /// happens.
    fn draw<R: Renderer>(
        &self,
        content: &str,
        props: ResolvedFontProps,
        bounds: Rect,
        color: R::Color,
        renderer: &mut R,
    ) -> Option<RenderResult>;
}

static FONT_UNIQUE_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FontId {
    Unique(usize),
    Name(&'static str),
}

impl Display for FontId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FontId::Unique(id) => write!(f, "{id}"),
            FontId::Name(name) => write!(f, "{name}"),
        }
    }
}

impl FontId {
    pub fn unique() -> Self {
        Self::Unique(FONT_UNIQUE_ID.fetch_add(1, Ordering::Relaxed))
    }
}

pub enum StoredFont {
    Fixed(FixedFont),
    FixedCollection(FixedFontCollection),
}

impl FontHandler for StoredFont {
    fn measure_text(
        &self,
        content: &str,
        props: ResolvedFontProps,
        overflow: TextOverflow,
    ) -> Option<TextIntrinsics> {
        match self {
            StoredFont::Fixed(fixed_font) => {
                fixed_font.measure_text(content, props, overflow)
            },
            StoredFont::FixedCollection(fixed_font_collection) => {
                fixed_font_collection.measure_text(content, props, overflow)
            },
        }
    }

    fn text_height_for_width(
        &self,
        content: &str,
        props: ResolvedFontProps,
        width: u32,
        overflow: TextOverflow,
    ) -> u32 {
        match self {
            StoredFont::Fixed(fixed_font) => fixed_font
                .text_height_for_width(content, props, width, overflow),
            StoredFont::FixedCollection(fixed_font_collection) => {
                fixed_font_collection
                    .text_height_for_width(content, props, width, overflow)
            },
        }
    }

    fn draw<R: Renderer>(
        &self,
        content: &str,
        props: ResolvedFontProps,
        bounds: Rect,
        color: R::Color,
        renderer: &mut R,
    ) -> Option<RenderResult> {
        match self {
            StoredFont::Fixed(fixed_font) => {
                fixed_font.draw::<R>(content, props, bounds, color, renderer)
            },
            StoredFont::FixedCollection(fixed_font_collection) => {
                fixed_font_collection
                    .draw::<R>(content, props, bounds, color, renderer)
            },
        }
    }
}

pub struct FontImport {
    // TODO: Add font family/usage: small, button, monospace, etc.
    id: FontId,
    data: StoredFont,
}

impl FontImport {
    fn new(data: StoredFont) -> Self {
        Self { id: FontId::unique(), data }
    }

    #[cfg(feature = "embedded-graphics")]
    pub fn fixed_eg_mono_font(
        font: &'static embedded_graphics::mono_font::MonoFont<'static>,
    ) -> Self {
        Self::new(StoredFont::Fixed(FixedFont::EGMonoFont(font)))
    }

    #[cfg(feature = "u8g2-fonts")]
    pub fn fixed_u8g2(font: &'static u8g2_fonts::FontRenderer) -> Self {
        Self::new(StoredFont::Fixed(FixedFont::U8G2(font)))
    }

    pub fn fixed_collection(collection: FixedFontCollection) -> Self {
        Self::new(StoredFont::FixedCollection(collection))
    }

    pub fn id(&self) -> FontId {
        self.id
    }

    pub fn named(mut self, name: &'static str) -> Self {
        // Note: Here unique identifier is left unused, but AtomicUsize range is
        // very large and we can ignore this
        self.id = FontId::Name(name);
        self
    }
}

// TODO: Dynamically pick based on display size.
fn pick_default_font() -> FontImport {
    cfg_select! {
        // u8g2 takes precedence over embedded_graphics as it looks better
        feature = "u8g2-fonts" => {
            static DEFAULT_U8G2_FONT: u8g2_fonts::FontRenderer = u8g2_fonts::FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_ncenB14_tr>();
            FontImport::fixed_u8g2(&DEFAULT_U8G2_FONT)
        },
        feature = "embedded-graphics" => {
            FontImport::fixed_eg_mono_font(&embedded_graphics::mono_font::ascii::FONT_8X13)
        },
        _ => compile_error!("Some of u8g2-fonts or embedded-graphics features must be enabled to provide a default font"),
    }
}

pub struct FontCtx {
    // 9a.2: sorted `Vec` keyed by `FontId` instead of a `BTreeMap` — a UI
    // carries 1–3 fonts, so binary search over a flat vec drops the BTreeMap
    // monomorphization/allocation. Kept sorted by id; replace-on-duplicate
    // preserves the previous `BTreeMap::insert` semantics.
    fonts: Vec<(FontId, StoredFont)>,
    fallback_font: FontId,
}

impl FontCtx {
    pub fn new() -> Self {
        // TODO: Replace with FixedFontCollection with size relative to viewport
        let default_fallback = pick_default_font();

        let mut this = Self {
            fonts: Default::default(),
            fallback_font: default_fallback.id,
        };

        this.insert(default_fallback);

        this
    }

    pub(crate) fn insert(&mut self, import: FontImport) {
        match self.fonts.binary_search_by(|(k, _)| k.cmp(&import.id)) {
            // Replace-on-duplicate — the previous `BTreeMap::insert` semantics.
            Ok(i) => self.fonts[i].1 = import.data,
            Err(i) => self.fonts.insert(i, (import.id, import.data)),
        }
    }

    pub(crate) fn expect(&self, id: FontId) -> &StoredFont {
        let i = self
            .fonts
            .binary_search_by(|(k, _)| k.cmp(&id))
            .expect("Font not found, maybe you forgot to import it into UI");
        &self.fonts[i].1
    }

    pub(crate) fn set_default(&mut self, import: FontImport) {
        self.fallback_font = import.id;
        self.insert(import);
    }

    fn fallback_font(&self) -> &StoredFont {
        let i = self
            .fonts
            .binary_search_by(|(k, _)| k.cmp(&self.fallback_font))
            .expect("[BUG] Fallback font not found");
        &self.fonts[i].1
    }

    fn auto_font(&self) -> &StoredFont {
        // TODO: More complex auto-font logic?
        self.fallback_font()
    }

    pub fn measure_text(
        &self,
        font: Font,
        content: &str,
        props: ResolvedFontProps,
        overflow: TextOverflow,
    ) -> TextIntrinsics {
        match font {
            Font::Auto => {
                self.auto_font().measure_text(content, props, overflow)
            },
            Font::Id(font_id) => {
                self.expect(font_id).measure_text(content, props, overflow)
            },
            Font::Fixed(fixed_font) => {
                fixed_font.measure_text(content, props, overflow)
            },
        }
        .unwrap_or_else(|| {
            self.fallback_font()
                .measure_text(content, props, overflow)
                .expect("[BUG] Fallback font must be defined")
        })
    }

    pub fn text_height_for_width(
        &self,
        font: Font,
        content: &str,
        props: ResolvedFontProps,
        width: u32,
        overflow: TextOverflow,
    ) -> u32 {
        match font {
            Font::Auto => self
                .auto_font()
                .text_height_for_width(content, props, width, overflow),
            Font::Id(font_id) => self
                .expect(font_id)
                .text_height_for_width(content, props, width, overflow),
            Font::Fixed(fixed_font) => fixed_font
                .text_height_for_width(content, props, width, overflow),
        }
    }

    // TODO: Background color!
    // TODO: Alignment!
    pub fn render<R: Renderer>(
        &self,
        font: Font,
        content: &str,
        props: ResolvedFontProps,
        bounds: Rect,
        color: R::Color,
        renderer: &mut R,
    ) -> RenderResult {
        match font {
            Font::Auto => self
                .auto_font()
                .draw::<R>(content, props, bounds, color, renderer),
            Font::Fixed(fixed_font) => {
                fixed_font.draw::<R>(content, props, bounds, color, renderer)
            },
            Font::Id(font_id) => self
                .expect(font_id)
                .draw::<R>(content, props, bounds, color, renderer),
        }
        .unwrap_or_else(|| {
            self.fallback_font()
                .draw::<R>(content, props, bounds, color, renderer)
                .expect("[BUG] Fallback font must be defined")
        })
    }
}
