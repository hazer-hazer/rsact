use crate::layout::size::Size;

/// User-specified font size
#[derive(Clone, Copy, Debug, PartialEq)]
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
            400.. => 20,
        };

        match self {
            FontSize::Unset => base,
            &FontSize::Fixed(fixed) => fixed,
            &FontSize::Relative(rel) => (base as f32 * rel) as u32,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum FontStyle {
    Normal,
    Italic,
    Bold,
}
