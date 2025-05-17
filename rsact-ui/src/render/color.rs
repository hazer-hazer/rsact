use core::fmt::Debug;
use embedded_graphics::{
    pixelcolor::{BinaryColor, Rgb555, Rgb565, Rgb666, Rgb888},
    prelude::{PixelColor, RgbColor},
};

use super::framebuf::PackedColor;

/// Trait allows implicit conversion from one color type to another so user can reduce color depth of UI but still drawing on draw target with higher color depth.
pub trait MapColor<O> {
    fn map_color(self) -> O;
}

impl MapColor<BinaryColor> for BinaryColor {
    fn map_color(self) -> BinaryColor {
        self
    }
}

impl MapColor<Rgb555> for BinaryColor {
    fn map_color(self) -> Rgb555 {
        match self {
            BinaryColor::Off => Rgb555::BLACK,
            BinaryColor::On => Rgb555::WHITE,
        }
    }
}

impl MapColor<Rgb565> for BinaryColor {
    fn map_color(self) -> Rgb565 {
        match self {
            BinaryColor::Off => Rgb565::BLACK,
            BinaryColor::On => Rgb565::WHITE,
        }
    }
}

impl MapColor<Rgb666> for BinaryColor {
    fn map_color(self) -> Rgb666 {
        match self {
            BinaryColor::Off => Rgb666::BLACK,
            BinaryColor::On => Rgb666::WHITE,
        }
    }
}

impl MapColor<Rgb888> for BinaryColor {
    fn map_color(self) -> Rgb888 {
        match self {
            BinaryColor::Off => Rgb888::BLACK,
            BinaryColor::On => Rgb888::WHITE,
        }
    }
}

impl<T: RgbExt, O: RgbExt> MapColor<O> for T {
    fn map_color(self) -> O {
        O::rgb(self.r(), self.g(), self.b())
    }
}

pub trait Color:
    PixelColor + PackedColor + From<<Self as PixelColor>::Raw> + Default + Debug
{
    fn default_foreground() -> Self;
    fn default_background() -> Self;
    fn accents() -> [Self; 6];
    fn map(&self, f: impl Fn(u8) -> u8) -> Self;
    fn fold(&self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self;

    fn invert(&self) -> Self {
        self.map(|c| 255 - c)
    }

    // TODO: Rewrite to use integer math
    // fn mix(&self, alpha: u8, other: Self) -> Self {
    //     // let this_alpha = 1.0 - alpha;
    //     let alpha = alpha as u16;
    //     let this_alpha = 256 - alpha;
    //     self.fold(other, |this, other| {
    //         ((this as u16 * this_alpha + other as u16 * alpha) >> 8) as u8
    //     })
    // }
    fn mix(&self, alpha: f32, other: Self) -> Self {
        let this_alpha = 1.0 - alpha;
        self.fold(other, |this, other| {
            (this as f32 * this_alpha + other as f32 * alpha) as u8
        })
    }
}

pub trait RgbExt:
    Sized + embedded_graphics::pixelcolor::RgbColor + Color
{
    fn rgb(r: u8, g: u8, b: u8) -> Self;

    // fn r(&self) -> u8;
    // fn g(&self) -> u8;
    // fn b(&self) -> u8;

    #[inline]
    fn hex(hex: u32) -> Self {
        Self::rgb(
            (hex & 0xff0000 >> 16) as u8,
            (hex & 0x00ff00 >> 8) as u8,
            (hex & 0x0000ff) as u8,
        )
    }

    // #[inline]
    // fn fold(&self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self {
    //     Self::rgb(
    //         f(self.r(), other.r()),
    //         f(self.g(), other.g()),
    //         f(self.b(), other.b()),
    //     )
    // }

    // fn mix(&self, alpha: f32, other: Self) -> Self {
    //     let this_alpha = 1.0 - alpha;
    //     self.fold(other, |this, other| {
    //         (this as f32 * this_alpha + other as f32 * alpha) as u8
    //     })
    // }
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

                fn map(&self, f: impl Fn(u8) -> u8) -> Self {
                    use embedded_graphics_core::pixelcolor::RgbColor;
                    Self::rgb(f(self.r()), f(self.g()), f(self.b()))
                }

                fn fold(&self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self {
                    use embedded_graphics_core::pixelcolor::RgbColor;
                    Self::rgb(
                        f(self.r(), other.r()),
                        f(self.g(), other.g()),
                        f(self.b(), other.b()),
                    )
                }
            }

            impl RgbExt for $color_ty {
                #[inline]
                fn rgb(r: u8, g: u8, b: u8) -> Self {
                    Self::new(r, g, b)
                }

                // fn r(&self) -> u8 {
                //     embedded_graphics_core::pixelcolor::RgbColor::r(self)
                // }
                // fn g(&self) -> u8 {
                //     embedded_graphics_core::pixelcolor::RgbColor::g(self)
                // }
                // fn b(&self) -> u8 {
                //     embedded_graphics_core::pixelcolor::RgbColor::b(self)
                // }
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
