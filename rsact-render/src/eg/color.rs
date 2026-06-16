use crate::color::{Color, RgbColor, Rgba};
use embedded_graphics::pixelcolor::{
    BinaryColor, Rgb555, Rgb565, Rgb666, Rgb888,
};

macro_rules! impl_rgb_colors {
    ($($color_ty:ty),* $(,)?) => {
        $(
            impl Color for $color_ty {
                const WHITE: Self = <$color_ty as embedded_graphics::pixelcolor::RgbColor>::WHITE;
                const BLACK: Self = <$color_ty as embedded_graphics::pixelcolor::RgbColor>::BLACK;

                fn default_foreground() -> Self {
                    <$color_ty as embedded_graphics::pixelcolor::RgbColor>::BLACK
                }

                fn default_background() -> Self {
                    <$color_ty as embedded_graphics::pixelcolor::RgbColor>::WHITE
                }

                fn from_rgba(rgba: Rgba) -> Self {
                    Self::rgb(rgba.r, rgba.g, rgba.b)
                }

                fn into_rgba(&self) -> Rgba {
                    Rgba {
                        r: self.r(),
                        g: self.g(),
                        b: self.b(),
                        a: 255,
                    }
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
    const WHITE: Self = Self::On;
    const BLACK: Self = Self::Off;

    fn default_foreground() -> Self {
        Self::On
    }

    fn default_background() -> Self {
        Self::Off
    }

    fn accents() -> [Self; 6] {
        [Self::On; 6]
    }

    fn from_rgba(rgba: Rgba) -> Self {
        if rgba.a > 127 && (rgba.r > 127 || rgba.g > 127 || rgba.b > 127) {
            Self::On
        } else {
            Self::Off
        }
    }

    fn into_rgba(&self) -> Rgba {
        match self {
            BinaryColor::Off => Rgba { r: 0, g: 0, b: 0, a: 255 },
            BinaryColor::On => Rgba { r: 255, g: 255, b: 255, a: 255 },
        }
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
