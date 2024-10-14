use crate::layout::size::Size;

/// User-specified font size
/// For now, FontSize is about the width of the mono font, but it might be
/// extended in the future with specific height as there're different variants
/// of same width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FontSize {
    Unset,
    /// Fixed font-size in pixels.
    Fixed(u32),
    /// Relative to viewport value where 1.0 is given by auto-setting Unset
    /// variant
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
            0..=63 => 4,
            64..=127 => 5,
            128..=191 => 6,
            192..=255 => 7,
            256.. => 8,
        };

        match self {
            FontSize::Unset => base,
            &FontSize::Fixed(fixed) => fixed,
            &FontSize::Relative(rel) => (base as f32 * rel) as u32,
        }
    }
}

#[derive(Clone, Copy)]
pub enum FontStyle {
    Normal,
    Italic,
    Bold,
}
