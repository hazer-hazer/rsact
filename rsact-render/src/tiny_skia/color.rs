use crate::{
    color::{Color, RgbColor},
    output::MapColor,
};

impl Color for tiny_skia::Color {
    const WHITE: Self = tiny_skia::Color::WHITE;
    const BLACK: Self = tiny_skia::Color::BLACK;

    fn default_foreground() -> Self {
        tiny_skia::Color::BLACK
    }

    fn default_background() -> Self {
        tiny_skia::Color::WHITE
    }

    fn accents() -> [Self; 6] {
        [
            Self::from_rgba8(255, 0, 0, 255),
            Self::from_rgba8(0, 255, 0, 255),
            Self::from_rgba8(0, 0, 255, 255),
            Self::from_rgba8(255, 255, 0, 255),
            Self::from_rgba8(0, 255, 255, 255),
            Self::from_rgba8(255, 0, 255, 255),
        ]
    }

    fn from_rgba(rgba: crate::color::Rgba) -> Self {
        tiny_skia::Color::from_rgba8(rgba.r, rgba.g, rgba.b, rgba.a)
    }

    fn into_rgba(&self) -> crate::color::Rgba {
        let u8 = self.to_color_u8();
        crate::color::Rgba {
            r: u8.red(),
            g: u8.green(),
            b: u8.blue(),
            a: u8.alpha(),
        }
    }

    // TODO: Does mapping f32 -> u8 -> f32 lose any precision significant for
    // tiny_skia or it is only required for tiny_skia internals and we are okay
    // operating on u8?
    fn map(&self, f: impl Fn(u8) -> u8) -> Self {
        let u8 = self.to_color_u8();
        tiny_skia::Color::from_rgba8(
            f(u8.red()),
            f(u8.green()),
            f(u8.blue()),
            f(u8.alpha()),
        )
    }

    fn fold(&self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self {
        let u8_self = self.to_color_u8();
        let u8_other = other.to_color_u8();
        tiny_skia::Color::from_rgba8(
            f(u8_self.red(), u8_other.red()),
            f(u8_self.green(), u8_other.green()),
            f(u8_self.blue(), u8_other.blue()),
            f(u8_self.alpha(), u8_other.alpha()),
        )
    }
}

impl RgbColor for tiny_skia::Color {
    fn rgb(r: u8, g: u8, b: u8) -> Self {
        tiny_skia::Color::from_rgba8(r, g, b, 255)
    }

    fn r(&self) -> u8 {
        self.to_color_u8().red()
    }

    fn g(&self) -> u8 {
        self.to_color_u8().green()
    }

    fn b(&self) -> u8 {
        self.to_color_u8().blue()
    }
}

#[cfg(feature = "embedded-graphics")]
impl MapColor<embedded_graphics::pixelcolor::Rgb888>
    for tiny_skia::PremultipliedColorU8
{
    fn map_color(&self) -> embedded_graphics::pixelcolor::Rgb888 {
        embedded_graphics::pixelcolor::Rgb888::new(
            self.red(),
            self.green(),
            self.blue(),
        )
    }
}
