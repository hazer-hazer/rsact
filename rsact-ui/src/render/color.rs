use core::fmt::Debug;

use embedded_graphics::{
    pixelcolor::{Rgb555, Rgb565, Rgb666, Rgb888},
    prelude::{PixelColor, RgbColor},
};

pub trait Color:
    PixelColor + From<<Self as PixelColor>::Raw> + Default + Debug
{
    fn default_foreground() -> Self;
    fn default_background() -> Self;
}

macro_rules! impl_rgb_colors {
    ($($color_ty:ty),* $(,)?) => {
        $(
            impl Color for $color_ty {
                fn default_foreground() -> Self {
                    <$color_ty>::BLACK
                }

                fn default_background() -> Self {
                    <$color_ty>::WHITE
                }
            }
        )*
    };
}

impl_rgb_colors!(Rgb555, Rgb565, Rgb666, Rgb888);
