use crate::render::color::Color;
use rsact_render::style::block::Radius;

pub mod rgb;

#[cfg(feature = "embedded-graphics")]
pub mod binary_color;

#[cfg(feature = "embedded-graphics")]
pub use binary_color::BinaryTheme;

/// Application-level theme for RGB color spaces: provides default styles for
/// all built-in widgets.
///
/// Construct with [`Theme::default()`] and optionally customize with the
/// builder methods (see [`rgb`]).
///
/// 1-bit / monochrome displays are a very special case and get their own
/// dedicated theme instead, [`BinaryTheme`](binary_color::BinaryTheme), since
/// the RGB strategy of dimming/tinting surfaces is impossible with only two
/// colors.
#[derive(Clone, Copy, PartialEq)]
pub struct Theme<C: Color> {
    bg: C,
    fg: C,
    primary: C,
    border_radius: Radius,

    // Derived //
    bg_muted: C,
    fg_muted: C,
}

// TODO: MaterialYou? HCT it is too complex and heavy, better implement just
// something like a seed color scheme generation.
