use core::fmt::Debug;

use embedded_graphics::{
    pixelcolor::{BinaryColor, Rgb555, Rgb565, Rgb666, Rgb888},
    prelude::PixelColor,
};

pub trait Color:
    PixelColor + From<<Self as PixelColor>::Raw> + Default + Debug
{
    fn default_foreground() -> Self;
    fn default_background() -> Self;
}

pub trait RgbColor: Sized {
    fn rgb(r: u8, g: u8, b: u8) -> Self;

    fn hex(hex: u32) -> Self {
        Self::rgb(
            (hex & 0xff0000 >> 4) as u8,
            (hex & 0x00ff00 >> 2) as u8,
            (hex & 0x0000ff) as u8,
        )
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
            }

            impl RgbColor for $color_ty {
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
}
