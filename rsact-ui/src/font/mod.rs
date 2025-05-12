use crate::{
    layout::{Limits, size::Size},
    widget::{RenderResult, ctx::WidgetCtx},
};
use alloc::collections::btree_map::BTreeMap;
use core::{
    fmt::{Debug, Display},
    sync::atomic::AtomicUsize,
};
use embedded_graphics::primitives::Rectangle;
use fixed::{FixedFont, FixedFontCollection};
use rsact_reactive::{
    maybe::MaybeReactive, prelude::IntoMaybeReactive, read::ReadSignal,
};

pub mod fixed;

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

/// Tree-targeting font properties stored inside layouts with contents and passed on mount to widgets.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct FontProps {
    pub font: Option<MaybeReactive<Font>>,
    pub font_size: Option<MaybeReactive<FontSize>>,
    pub font_style: Option<MaybeReactive<FontStyle>>,
}

impl FontProps {
    pub fn inherit(&mut self, parent: &FontProps) {
        if let font @ None = &mut self.font {
            *font = parent.font.clone();
        }
        if let font_size @ None = &mut self.font_size {
            *font_size = parent.font_size.clone();
        }
        if let font_style @ None = &mut self.font_style {
            *font_style = parent.font_style.clone();
        }
    }

    pub fn resolve(&self, viewport: Size) -> AbsoluteFontProps {
        let font_size = self
            .font_size
            .map(|font_size| font_size.get())
            .unwrap_or_default()
            .resolve(viewport);

        let font_style = self
            .font_style
            .map(|font_style| font_style.get())
            .unwrap_or_default();

        AbsoluteFontProps { size: font_size, style: font_style }
    }

    pub fn font(&self) -> Font {
        // TODO: Is font required to be set at least by global default or we should fallback here?
        self.font.unwrap().get()
    }
}

impl Display for FontProps {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        todo!()
    }
}

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
    Bold,
    // Oblique,
}

/// Resolved font properties
#[derive(Debug, Clone, Copy)]
pub struct AbsoluteFontProps {
    /// Absolute font size, i.e. font height in pixels
    pub size: u32,
    pub style: FontStyle,
}

pub enum FontFamily {
    Monospace,
    Proportional,
    // TODO: Custom?
}

/// Font setting found in text widget. It is an identifier pointing to the actual font or a fixed-size font set for a specific text widget (e.g. embedded_graphics MonoFont or u8g2 font)
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
    fn measure_text_size(
        &self,
        content: &str,
        props: AbsoluteFontProps,
    ) -> Option<Limits>;

    fn draw<W: WidgetCtx>(
        &self,
        content: &str,
        props: AbsoluteFontProps,
        bounds: Rectangle,
        color: W::Color,
        renderer: &mut W::Renderer,
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
        Self::Unique(
            FONT_UNIQUE_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
        )
    }
}

pub enum StoredFont {
    Fixed(FixedFont),
    FixedCollection(FixedFontCollection),
}

impl FontHandler for StoredFont {
    fn measure_text_size(
        &self,
        content: &str,
        props: AbsoluteFontProps,
    ) -> Option<Limits> {
        match self {
            StoredFont::Fixed(fixed_font) => {
                fixed_font.measure_text_size(content, props)
            },
            StoredFont::FixedCollection(fixed_font_collection) => {
                fixed_font_collection.measure_text_size(content, props)
            },
        }
    }

    fn draw<W: WidgetCtx>(
        &self,
        content: &str,
        props: AbsoluteFontProps,
        bounds: Rectangle,
        color: W::Color,
        renderer: &mut W::Renderer,
    ) -> Option<RenderResult> {
        match self {
            StoredFont::Fixed(fixed_font) => {
                fixed_font.draw::<W>(content, props, bounds, color, renderer)
            },
            StoredFont::FixedCollection(fixed_font_collection) => {
                fixed_font_collection
                    .draw::<W>(content, props, bounds, color, renderer)
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

    pub fn fixed_eg_mono_font(
        font: &'static embedded_graphics::mono_font::MonoFont<'static>,
    ) -> Self {
        Self::new(StoredFont::Fixed(FixedFont::EGMonoFont(font)))
    }

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
        // Note: Here unique identifier is left unused, but AtomicUsize range is very large and we can ignore this
        self.id = FontId::Name(name);
        self
    }
}

pub struct FontCtx {
    fonts: BTreeMap<FontId, StoredFont>,
    fallback_font: FontId,
}

impl FontCtx {
    pub fn new() -> Self {
        // TODO: Replace with FixedFontCollection with size relative to viewport
        let default_fallback = FontImport::fixed_eg_mono_font(
            &embedded_graphics::mono_font::ascii::FONT_9X15,
        );

        let mut this = Self {
            fonts: Default::default(),
            fallback_font: default_fallback.id,
        };

        this.insert(default_fallback);

        this
    }

    pub(crate) fn insert(&mut self, import: FontImport) {
        self.fonts.insert(import.id, import.data);
    }

    pub(crate) fn expect(&self, id: FontId) -> &StoredFont {
        self.fonts
            .get(&id)
            .expect("Font not found, maybe you forgot to import it into UI")
    }

    pub(crate) fn set_default(&mut self, import: FontImport) {
        self.fallback_font = import.id;
        self.insert(import);
    }

    fn fallback_font(&self) -> &StoredFont {
        self.fonts
            .get(&self.fallback_font)
            .expect("[BUG] Fallback font not found")
    }

    fn auto_font(&self) -> &StoredFont {
        // TODO: More complex auto-font logic?
        self.fallback_font()
    }

    pub fn measure_text_size(
        &self,
        font: Font,
        content: &str,
        props: AbsoluteFontProps,
    ) -> Limits {
        match font {
            Font::Auto => self.auto_font().measure_text_size(content, props),
            Font::Id(font_id) => {
                self.expect(font_id).measure_text_size(content, props)
            },
            Font::Fixed(fixed_font) => {
                fixed_font.measure_text_size(content, props)
            },
        }
        .unwrap_or_else(|| {
            self.fallback_font()
                .measure_text_size(content, props)
                .expect("[BUG] Fallback font must be defined")
        })
    }

    pub fn render<W: WidgetCtx>(
        &self,
        font: Font,
        content: &str,
        props: AbsoluteFontProps,
        bounds: Rectangle,
        color: W::Color,
        renderer: &mut W::Renderer,
    ) -> RenderResult {
        match font {
            Font::Auto => self
                .auto_font()
                .draw::<W>(content, props, bounds, color, renderer),
            Font::Fixed(fixed_font) => {
                fixed_font.draw::<W>(content, props, bounds, color, renderer)
            },
            Font::Id(font_id) => self
                .expect(font_id)
                .draw::<W>(content, props, bounds, color, renderer),
        }
        .unwrap_or_else(|| {
            self.fallback_font()
                .draw::<W>(content, props, bounds, color, renderer)
                .expect("[BUG] Fallback font must be defined")
        })
    }
}
