use core::{fmt::Debug, ops::Add};
use embedded_graphics::{
    pixelcolor::{BinaryColor, Rgb555, Rgb565, Rgb666, Rgb888},
    prelude::PixelColor,
};

pub trait Color:
    PixelColor + From<<Self as PixelColor>::Raw> + Default + Debug
{
    fn default_foreground() -> Self;
    fn default_background() -> Self;
    fn accents() -> [Self; 6];
}

pub trait RgbColor: Sized + embedded_graphics::pixelcolor::RgbColor {
    fn rgb(r: u8, g: u8, b: u8) -> Self;

    #[inline]
    fn hex(hex: u32) -> Self {
        Self::rgb(
            (hex & 0xff0000 >> 4) as u8,
            (hex & 0x00ff00 >> 2) as u8,
            (hex & 0x0000ff) as u8,
        )
    }

    #[inline]
    fn fold(&self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self {
        Self::rgb(
            f(self.r(), other.r()),
            f(self.g(), other.g()),
            f(self.b(), other.b()),
        )
    }

    fn mix(&self, alpha: f32, other: Self) -> Self {
        let this_alpha = 1.0 - alpha;
        self.fold(other, |this, other| {
            (this as f32 * this_alpha + other as f32 * alpha) as u8
        })
    }
}

macro_rules! impl_rgb_colors {
    ($($color_ty:ty),* $(,)?) => {
        $(
            impl Color for $color_ty {
                fn default_foreground() -> Self {
                    <$color_ty as embedded_graphics::pixelcolor::RgbColor>::BLACK
                }

                fn default_background() -> Self {
                    <$color_ty as embedded_graphics::pixelcolor::RgbColor>::WHITE
                }

                fn accents() -> [Self; 6] {
                    [
                        <$color_ty as embedded_graphics::pixelcolor::RgbColor>::RED,
                        <$color_ty as embedded_graphics::pixelcolor::RgbColor>::GREEN,
                        <$color_ty as embedded_graphics::pixelcolor::RgbColor>::BLUE,
                        <$color_ty as embedded_graphics::pixelcolor::RgbColor>::YELLOW,
                        <$color_ty as embedded_graphics::pixelcolor::RgbColor>::MAGENTA,
                        <$color_ty as embedded_graphics::pixelcolor::RgbColor>::CYAN,
                    ]
                }
            }

            impl RgbColor for $color_ty {
                #[inline]
                fn rgb(r: u8, g: u8, b: u8) -> Self {
                    Self::new(r, g, b)
                }
            }
        )*
    };
}

impl_rgb_colors!(Rgb555, Rgb565, Rgb666, Rgb888);

impl Color for BinaryColor {
    fn default_foreground() -> Self {
        Self::Off
    }

    fn default_background() -> Self {
        Self::On
    }

    fn accents() -> [Self; 6] {
        [Self::On; 6]
    }
}
