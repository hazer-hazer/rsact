use crate::color::{Color, RgbColor};
use embedded_graphics::pixelcolor::{
    BinaryColor, Rgb555, Rgb565, Rgb666, Rgb888,
};

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

                fn map(&self, f: impl Fn(u8) -> u8) -> Self {
                    Self::rgb(f(self.r()), f(self.g()), f(self.b()))
                }

                fn fold(&self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self {
                    Self::rgb(
                        f(self.r(), other.r()),
                        f(self.g(), other.g()),
                        f(self.b(), other.b()),
                    )
                }
            }

            impl RgbColor for $color_ty {
                fn r(&self) -> u8 {
                    embedded_graphics::pixelcolor::RgbColor::r(self)
                }

                fn g(&self) -> u8 {
                    embedded_graphics::pixelcolor::RgbColor::g(self)
                }

                fn b(&self) -> u8 {
                    embedded_graphics::pixelcolor::RgbColor::b(self)
                }

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
        Self::On
    }

    fn default_background() -> Self {
        Self::Off
    }

    fn accents() -> [Self; 6] {
        [Self::On; 6]
    }

    fn map(&self, f: impl Fn(u8) -> u8) -> Self {
        if f(match self {
            BinaryColor::Off => 0,
            BinaryColor::On => 255,
        }) > 127
        {
            Self::On
        } else {
            Self::Off
        }
    }

    fn fold(&self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self {
        if f(
            match self {
                BinaryColor::Off => 0,
                BinaryColor::On => 255,
            },
            match other {
                BinaryColor::Off => 0,
                BinaryColor::On => 255,
            },
        ) > 127
        {
            Self::On
        } else {
            Self::Off
        }
    }
}
